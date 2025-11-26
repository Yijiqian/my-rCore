use lazy_static::lazy_static;
use super::{VPNRange, VirtPageNum, FrameTracker, VirtAddr, PageTable, StepByOne, PhysPageNum, PhysAddr,
            PTEFlags, frame_alloc, PageTableEntry};
use crate::config::{PAGE_SIZE, TRAMPOLINE, USER_STACK_SIZE, MEMORY_END, TRAP_CONTEXT};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::sync::Arc;
use riscv::register::satp;
use core::arch::asm;
use crate::sync::UPSafeCell;


#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,   // 恒等映射
    Framed,    // 页面映射
}

bitflags! {
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

unsafe extern "C" {
    safe fn stext();
    safe fn etext();
    safe fn srodata();
    safe fn erodata();
    safe fn sdata();
    safe fn edata();
    safe fn sbss_with_stack();
    safe fn ebss();
    safe fn ekernel();
    safe fn strampoline();
}

pub struct MapArea {
    // 一段虚拟页号的连续区间，表示该逻辑段在地址区间中的位置和长度。
    vpn_range: VPNRange,
    // Frame_Tracker 中的物理页帧存放实际数据，而不是一个页表
    data_frames: BTreeMap<VirtPageNum, FrameTracker>, 
    // 描述该逻辑段内的所有虚拟页面映射到物理页帧的同一种方式，是一个枚举类型
    map_type: MapType,
    // 表示控制该逻辑段的访问方式，是页表项标志位 PTEFlags 的一个子集，仅保留 U/R/W/X 四个标志位
    map_perm: MapPermission,
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }

    pub fn from_another(another: &MapArea) -> Self {
        Self {
            vpn_range: VPNRange::new(
                another.vpn_range.get_start(),
                another.vpn_range.get_end()
            ),
            data_frames: BTreeMap::new(),
            map_type: another.map_type,
            map_perm: another.map_perm,
        }
    }

    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);  // 建立一个 vpn 在页表中的映射
        }
    }

    #[allow(unused)]
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }

    pub fn copy_data(&mut self, page_table: &PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                    .translate(current_vpn)
                    .unwrap()
                    .ppn()
                    .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }

    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }

    #[allow(unused)]
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        match self.map_type {
            MapType::Framed => {
                self.data_frames.remove(&vpn);  // 清除 (vpn, ppn) 这个键值对，对应的物理页帧会自动回收
            }
            _ => {}
        }
        page_table.unmap(vpn);  // 再取消页表中 vpn -> ppn 的映射。
    }

    #[allow(unused)]
    pub fn shrink_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(new_end, self.vpn_range.get_end()) {
            self.unmap_one(page_table, vpn);
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }

    #[allow(unused)]
    pub fn append_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(self.vpn_range.get_end(), new_end) {
            self.map_one(page_table, vpn);
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
}

pub struct MemorySet {
    /* 注意：
            PageTable 下挂着所有多级页表的节点所在的物理页帧，而每个 MapArea 下则挂着对应逻辑段
            中的数据所在的物理页帧，这两部分合在一起构成了一个地址空间所需的所有物理页帧。
            这同样是一种 RAII 风格，当一个地址空间 MemorySet 生命周期结束后，这些物理页帧都会被回收
     */
    page_table: PageTable,  // 该地址空间的多级页表
    areas: Vec<MapArea>,    // 逻辑段向量
}

impl MemorySet {
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }

    pub fn from_existed_user(user_space: &MemorySet) -> MemorySet {
        let mut memory_set = Self::new_bare();

        memory_set.map_trampoline();

        for area in user_space.areas.iter() {
            let new_area = MapArea::from_another(area);
            memory_set.push(new_area, None);
            
            // 从另一个地址空间复制数据
            for vpn in area.vpn_range {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                let dst_ppn = memory_set.translate(vpn).unwrap().ppn();
                dst_ppn.get_bytes_array().copy_from_slice(src_ppn.get_bytes_array());
            }
        }

        memory_set
    }

    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        // 创建待插入的逻辑段 的 vpn -> ppn 映射，以及创建对应存储数据的物理页帧
        map_area.map(&mut self.page_table);  
        if let Some(data) = data {
            // 初始化物理页帧中的数据
            map_area.copy_data(&self.page_table, data);
        }
        self.areas.push(map_area);
    }

    pub fn insert_framed_area (
        &mut self,
        start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission
    ) {
        self.push(MapArea::new(
            start_va,
            end_va,
            MapType::Framed,
            permission,
        ), None);
    }

    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),     // TRAMPOLINE：跳板的虚拟地址。 into()实现将虚拟地址转换为虚拟页号，核心实现为 floor()
            PhysAddr::from(strampoline as usize).into(),    // strampoline: 跳板的物理地址，这个符号在 linker-qemu.ld 中声明
            PTEFlags::R | PTEFlags::X,
        );
    }

    /// 生成内核的地址空间
    pub fn new_kernel() -> Self{
        let mut memory_set = Self::new_bare();

        // 映射跳板
        memory_set.map_trampoline();

        // 映射内核段
        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(".bss [{:#x}, {:#x})", sbss_with_stack as usize, ebss as usize);

        println!("mapping .text section");
        memory_set.push(MapArea::new(
            (stext as usize).into(),
            (etext as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::X,
        ), None);

        println!("mapping .rodata section");
        memory_set.push(MapArea::new(
            (srodata as usize).into(),
            (erodata as usize).into(),
            MapType::Identical,
            MapPermission::R,
        ), None);

        println!("mapping .data section");
        memory_set.push(MapArea::new(
            (sdata as usize).into(),
            (edata as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
        ), None);

        println!("mapping .bss section");
        memory_set.push(MapArea::new(
            (sbss_with_stack as usize).into(),
            (ebss as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
        ), None);

        println!("mapping physical memory");
        println!("[ekernel, MEMORY_END] = [{:#x}, {:#x}]", ekernel as usize, MEMORY_END);
        memory_set.push(MapArea::new(
            (ekernel as usize).into(),
            MEMORY_END.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
        ), None);

        memory_set
    }

    /// 分析应用的 ELF 文件格式内容，解析出各数据段并生成对应的地址空间
    /// 包括 elf 中的段、跳板、trap 上下文、以及用户栈
    /// 同时返回用户栈指针和入口点
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = MemorySet::new_bare();

        // 映射跳板
        memory_set.map_trampoline();  // 跳板的地址范围为 [TRAMPOLINE + PAGE_SIZE, TRAMPOLINE]

        // 映射 ELF 的程序头
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();  // 从原始 ELF文件数据 创建 ELF文件解析器
        let elf_header = elf.header;   // 获取ELF文件头，文件头包含文件的基本信息和元数据
        let magic = elf_header.pt1.magic;   // 获取 ELF 文件的魔术字节，以验证文件确实是ELF格式
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();  // 获取程序头表中条目的数量。程序头表描述如何将程序加载到内存中的段信息

        // 记录目前涉及到的最大虚拟页号
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            // 程序头的类型为 Load ，表示它有被内核加载的必要
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                // println!(" ph = {}", i);
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                // println!("start_va = {:#x}, end_va = {:#x}", (ph.virtual_addr() as usize), ((ph.virtual_addr() + ph.mem_size()) as usize));
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read()  { map_perm |= MapPermission::R; }
                if ph_flags.is_write()  { map_perm |= MapPermission::W; }
                if ph_flags.is_execute()  { map_perm |= MapPermission::X; }
                let map_area = MapArea::new(
                    start_va,
                    end_va,
                    MapType::Framed,
                    map_perm,
                );
                max_end_vpn = map_area.vpn_range.get_end();
                // println!("max_end_vpn = {}", max_end_vpn.0);
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize])
                );
            }
        }

        // 映射用户栈
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        user_stack_bottom += PAGE_SIZE;   // 加一个 PAGE_SZIE，表示放置一个保护页面
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(MapArea::new(
            user_stack_bottom.into(),
            user_stack_top.into(),
            MapType::Framed,
            MapPermission::R | MapPermission::W | MapPermission::U,
        ), None);

        memory_set.push(MapArea::new(
            TRAP_CONTEXT.into(),
            TRAMPOLINE.into(),
            MapType::Framed,
            MapPermission::R | MapPermission::W,
        ), None);

        (memory_set, user_stack_top, elf.header.pt2.entry_point() as usize)
    }

    pub fn activate(&self) {
        let satp = self.page_table.token();  // 将根页表的物理页帧号 逻辑或上 8usize<<60，而这个8正式启动页表的关键
        unsafe {
            println!("start activate pagetable!");
            satp::write(satp);
            println!("activate pagetable success!");
            asm!("sfence.vma");  // 清除快表
        }
    }

    pub fn token(&self) -> usize {
        self.page_table.token()
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }

    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some((idx, area)) = self.areas
                                       .iter_mut()
                                       .enumerate()
                                       .find(|(_, area)| area.vpn_range.get_start() == start_vpn)
        {
            area.unmap(&mut self.page_table);
            self.areas.remove(idx);
        }
    }

    #[allow(unused)]
    pub fn shrink_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self.areas 
                                .iter_mut()
                                .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.shrink_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }

    #[allow(unused)]
    pub fn append_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self.areas
                                .iter_mut()
                                .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.append_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }

    pub fn recycle_data_pages(&mut self) {
        self.areas.clear();
    }
}

lazy_static! {
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> = Arc::new(unsafe {
        UPSafeCell::new(MemorySet::new_kernel())
    });
}

pub fn remap_test() {
    let kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert_eq!(
        kernel_space.page_table.translate(mid_text.floor()).unwrap().writable(),
        false
    );
    assert_eq!(
        kernel_space.page_table.translate(mid_rodata.floor()).unwrap().writable(),
        false
    );
    assert_eq!(
        kernel_space.page_table.translate(mid_data.floor()).unwrap().executable(),
        false
    );
    println!("remap_test passed!");
}
