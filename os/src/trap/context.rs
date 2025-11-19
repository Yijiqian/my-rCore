use riscv::register::sstatus::{self, SPP, Sstatus};

/// 上下文
#[repr(C)]
pub struct TrapContext {
    /// 32个通用寄存器
    pub x: [usize; 32],  

    /// CSR中存储当前 CPU 执行环境的特权级
    pub sstatus: Sstatus,

    /// 存储PC值的寄存器  
    pub sepc: usize,    

    /// 下面三个字段在应用初始化的时候，由内核写入应用地址空间中的 TrapContext 的相应位置，此后就不再被修改
    /// 内核地址空间的 token，即内核页表的起始物理地址；
    pub kernel_satp: usize,
    /// 当前应用在内核地址空间中的内核栈栈顶的虚拟地址；
    pub kernel_sp: usize,
    /// 内核中 trap handler 入口点的虚拟地址
    pub trap_handler: usize,
}

impl TrapContext {
    /// 设置栈指针
    pub fn set_sp(&mut self, sp: usize) { self.x[2] = sp; }
    
    /// 初始化应用程序的上下文
    pub fn app_init_context(
        entry: usize, 
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        cx.set_sp(sp);
        cx
    }
}