//! Loading user applications into memory
//!
//! For chapter 3, user applications are simply part of the data included in the
//! kernel binary, so we only need to copy them to the space allocated for each
//! app to load them. We also allocate fixed spaces for each task's
//! [`KernelStack`] and [`UserStack`].

use lazy_static::lazy_static;
use alloc::vec::Vec;

/// 获取应用程序的数目
pub fn get_num_app() -> usize {
    unsafe extern "C" {
        safe fn _num_app();
    }
    unsafe {
        (_num_app as usize as *const usize).read_volatile()
    }
}

lazy_static! {
    static ref APP_NAMES: Vec<&'static str> = {
        let num_app = get_num_app();
        unsafe extern "C" { safe fn _app_names(); }
        let mut start = _app_names as usize as *const u8;
        let mut v = Vec::new();
        unsafe {
            for _ in 0..num_app {
                let mut end = start;
                while end.read_volatile() != '\0' as u8 {
                    end = end.add(1);
                }
                let slice = core::slice::from_raw_parts(start, end as usize - start as usize);
                let str = core::str::from_utf8(slice).unwrap();
                v.push(str);
                start = end.add(1);
            }
        }
        v
    };
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

pub fn get_app_data_by_name(name: &str) -> Option<&'static [u8]> {
    let num_app = get_num_app();
    (0..num_app).find(|&i| APP_NAMES[i] == name)
                .map(|i| get_app_data(i))
}
