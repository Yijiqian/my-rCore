//! Loading user applications into memory
//!
//! For chapter 3, user applications are simply part of the data included in the
//! kernel binary, so we only need to copy them to the space allocated for each
//! app to load them. We also allocate fixed spaces for each task's
//! [`KernelStack`] and [`UserStack`].


// #[repr(align(4096))]
// #[derive(Copy, Clone)]
// struct KernelStack {
//     data: [u8; KERNEL_STACK_SIZE],
// }

// #[repr(align(4096))]
// #[derive(Copy, Clone)]
// struct UserStack {
//     data: [u8; USER_STACK_SIZE],
// }

// static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [KernelStack { 
//     data: [0; KERNEL_STACK_SIZE],
// }; MAX_APP_NUM];

// static USER_STACK: [UserStack; MAX_APP_NUM] = [ UserStack { 
//     data: [0; USER_STACK_SIZE],
// }; MAX_APP_NUM];

// impl KernelStack {
//     fn get_sp(&self) -> usize {
//         self.data.as_ptr() as usize + KERNEL_STACK_SIZE
//     }

//     pub fn push_context(&self, trap_cx: TrapContext) -> usize {
//         let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
//         unsafe {
//             *trap_cx_ptr = trap_cx;
//         }
//         trap_cx_ptr as usize
//     }
// }

// impl UserStack {
//     fn get_sp(&self) -> usize {
//         self.data.as_ptr() as usize + USER_STACK_SIZE
//     }
// }

// /// 获取应用程序i在内存中的基地址
// fn get_base_i(app_id: usize) -> usize {
//     APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
// }



// /// 根据 app_id 加载对应的应用程序到不同内存
// pub fn load_app() {
//     unsafe extern "C" { safe fn _num_app(); }
//     let num_app_ptr = _num_app as usize as *const usize;
//     let num_app = get_num_app();
//     let app_start = unsafe {
//         core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1)
//     };

//     for i in 0..num_app {
//         let base_i = get_base_i(i);

//         // 清除对应内存
//         (base_i..base_i + APP_SIZE_LIMIT).for_each(|addr| unsafe {
//             (addr as *mut u8).write_volatile(0)
//         });

//         let src = unsafe {
//             core::slice::from_raw_parts(
//                 app_start[i] as *const u8,
//                 app_start[i+1] - app_start[i]
//             )
//         };
//         let dst = unsafe {
//             core::slice::from_raw_parts_mut(base_i as *mut u8, src.len())
//         };
//         dst.copy_from_slice(src);
//         println!("[kernel] load app_{}, base_address is {:#x}", i, base_i);
//     }
//     unsafe {
//         core::arch::asm!("fence.i");
//     }
// }

// pub fn init_app_cx(app_id: usize) -> usize {
//     KERNEL_STACK[app_id].push_context(TrapContext::app_init_context(
//             get_base_i(app_id), 
//             USER_STACK[app_id].get_sp(),
//         )
//     )
// }

/// 获取应用程序的数目
pub fn get_num_app() -> usize {
    unsafe extern "C" {
        safe fn _num_app();
    }
    unsafe {
        (_num_app as usize as *const usize).read_volatile()
    }
}

pub fn get_app_data(app_id: usize) -> &'static [u8] {
    unsafe extern "C" { safe fn _num_app(); }
    let num_app_ptr = _num_app as usize as *const usize;  
    let num_app = get_num_app();
    let app_start = unsafe {
        core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1)
    };
    assert!(app_id < num_app);
    unsafe {
        core::slice::from_raw_parts(
            app_start[app_id] as *const u8,
            app_start[app_id + 1] - app_start[app_id]
        )
    }
}