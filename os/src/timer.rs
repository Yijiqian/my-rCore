use crate::config::CLOCK_FREQ;
const TICKS_PER_SEC: usize = 100;
const MICRO_PER_SEC: usize = 1_000_000;

use riscv::register::time;
use crate::sbi::set_timer;

/// 获取处理器内置时钟周期的计数器的值，这个计数器保存在 mtime 这个 CSR 中
pub fn get_time() -> usize {
    time::read()
}

/// 实现 10ms 后触发一个 S 特权级的时钟中断
pub fn set_next_trigger() {
    // CLOCK_FREQ 是一个预先获取到的各平台不同的时钟频率，单位为赫兹，也就是一秒内计数器的增量
    // TICKS_PER_SEC 表示每秒触发多少次时钟中断
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}

/// 以微秒为单位返回当前计数器的值
pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / MICRO_PER_SEC)  //    time::read() / 12.5
}