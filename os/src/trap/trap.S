.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .align 2
__alltraps:
    csrrw sp, sscratch, sp
    # now sp->*TrapContext in user space, sscratch->user stack
    # save general-purpose registers
    sd x1, 1*8(sp)
    # skip sp(x2), we will save it later
    sd x3, 3*8(sp)
    # skip tp(x4), application does not use it
    # save x5~x31
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    # we can use t0/t1/t2 freely, because they were saved on kernel stack
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    # 从 sscratch 中读取用户栈，并将其保存到 TrapContext 中
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # 加载 kernel_satp 到t0
    ld t0, 34*8(sp)
    # 加载 trap_handler 到 t1
    ld t1, 36*8(sp)
    # 加载内核栈到sp，即让sp 指向内核栈
    ld sp, 35*8(sp)
    # 往 satp CSR中写入 kernel_satp，以完成 用户地址空间 -> 内核地址空间 的切换
    csrw satp, t0
    sfence.vma
    # 到这里为止，sscratch 寄存器中存储用户栈的地址，sp的指向由 用户空间的 TrapContext 转变为 内核栈
    jr t1

__restore:
    # a0: 用户空间的 TrapContext；a1: 用户空间 token
    # 切换到用户空间
    csrw satp, a1
    sfence.vma
    # 将 trap 上下文位置保存到 sscratch 寄存器中，这样 __alltraps 中才能基于它将 Trap 上下文保存到正确位置
    csrw sscratch, a0
    mv sp, a0
    # 现在 sp 指向用户空间的 TrapContext，可以开始恢复trap上下文了
    # 恢复 sstatus 和 sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # restore general-purpuse registers except x0/sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # 回到用户栈
    ld sp, 2*8(sp)
    sret