use buddy_system_allocator::LockedHeap;
use crate::config::KERNEL_HEAP_SIZE;
use core::ptr::addr_of_mut;

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();

/*
 * 通过下面这行代码，在内核的 .bss 段划分一块 static mut 且被初始化为0 的
 * 字节数组。即 HEAP_SPACE 这个数据位于 .bss 中，大小为 KERNEL_HEAP_SIZE
 */
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// 在使用任何 alloc crate 中提供的堆数据之前，需先调用这个函数来给全局分配器一块内存用于分配。
/// 也就是将 HEAP_SPACE 作为堆内存
pub fn init_heap() {
    unsafe {
        // addr_of_mut! 用来安全地获取可变裸指针，实际上是直接获取地址
        HEAP_ALLOCATOR
            .lock()
            .init(addr_of_mut!(HEAP_SPACE) as usize, KERNEL_HEAP_SIZE);
    }
}

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    // core::alloc::Layout 类型指出了分配的需求，分别是所需空间的大小、以及返回地址
    // 的对齐要求 align
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[allow(unused)]
pub fn heap_test() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    unsafe extern "C" {
        safe fn sbss();
        safe fn ebss();
    }
    let bss_range = sbss as usize..ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);   // 检查两个值是否相等
    println!("*a = {}", *a as usize);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));   // 检查是否为 true
    drop(a);
    let mut v: Vec<usize> = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    for i in 0..5 {
        assert_eq!(v[i], i);
        println!("v[i] = {}", v[i] as usize);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    println!("heap_test passed!");
}