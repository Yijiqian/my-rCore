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
}

impl TrapContext {
    /// 设置栈指针
    pub fn set_sp(&mut self, sp: usize) { self.x[2] = sp; }
    
    /// 初始化应用程序的上下文
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
        };
        cx.set_sp(sp);
        cx
    }
}