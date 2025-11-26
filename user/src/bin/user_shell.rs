#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

const LF: u8 = 0x0au8;   // 换行符 - \n
const CR: u8 = 0x0du8;   // 回车符 - \r
const DL: u8 = 0x7fu8;   // 删除符 - DEL
const BS: u8 = 0x08u8;   // 退格符 - \b

use alloc::string::String;
use user_lib::{fork, exec, waitpid};
use user_lib::console::getchar;

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line: String = String::new();
    print!(">> ");
    loop {
        let c = getchar();
        match c {
            LF | CR => {   // 换行符 \n 或 回车符 \r
                println!("");
                if !line.is_empty() {
                    line.push('\0');
                    let pid = fork();
                    if pid == 0 {
                        // 子进程
                        if exec(line.as_str()) == -1 {
                            println!("Error when executing!");
                            return -4;
                        }
                        unreachable!();
                    } else {
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!(
                            "Shell: Process {} exited with code {}",
                            pid, exit_code
                        );
                    }
                    line.clear();
                }
                print!(">> ");
            }
            BS | DL => {   // 删除符 或 退格符
                if !line.is_empty() {
                    print!("{}", BS as char);  // 将屏幕上当前行的最后一个字符用空格替换掉
                    print!(" ");
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}