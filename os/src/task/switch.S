

# 下面的这段代码是 RISC-V 汇编中的宏定义
# s\n - 字符串链接，s 是寄存器名前缀，\n将参数 n 的值插入到字符串中。例如当 n=0 时，s\n 展开为 s0
# (\n+2)*8(a0) - 计算内存偏移。a0 基址寄存器，(\n+2)*8 计算偏移地址，最终地址为 = a0+(n+2)*8
# sd - 存储双字指令（Store Doubleword）。将原寄存器的值存储到内存地址
.altmacro                               # 启用高级宏功能
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
    .section .text
    .globl __switch
__switch:
    # 阶段【1】
    # __switch(
    #    current_task_cx_ptr: *mut TaskContext,
    #    next_task_cx_ptr: *const TaskContext
    # )
    # 阶段【2】
    # 保存当前任务的内核栈
    sd sp, 8(a0)
    # 保存当前任务的上下文
    sd ra, 0(a0)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n+1
    .endr
    # 阶段【3】
    # 恢复下个任务的上下文
    ld ra, 0(a1)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # 恢复下个任务的内核栈
    ld sp, 8(a1)
    # 阶段【4】
    ret