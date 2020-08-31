.section .text

.extern next_thread_state
.type next_thread_state, @function

// AMD64 ABI tells us that only rbx, rbp, r12 - r15 need to be preserved by the callee.
// _switch_to_next()
.global _switch_to_next
.type _switch_to_next, @function
_switch_to_next:
    pushfq
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    // Protect the scheduler from nesting.
    // The interrupt flag will be restored because of the popfq later.
    cli

    movq %rsp, %rdi
    call next_thread_state
    movq %rax, %rsp
    testq %rdx, %rdx
    jz 1f
    movq %rdx, %cr3
1:
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popfq

    ret

.global irq0
.type irq0, @function
irq0:
    cmpl $0, %gs:8 // Check if preempt_count != 0
    jnz .flag
    pushq %rax
    pushq %rdi
    pushq %rsi
    pushq %rdx
    pushq %rcx
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11

    // EOI, do this here because we might not end up at the bottom part if the other didn't come from an irq0.
    movb $32, %al
    outb %al, $32

    call _switch_to_next

    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rcx
    popq %rdx
    popq %rsi
    popq %rdi
    popq %rax

    iretq
.flag:
    pushq %rax
    // EOI
    movb $32, %al
    outb %al, $32
    popq %rax
    iretq

.global _thread_exit
.type _thread_exit, @function
_thread_exit:
    // We want to free the memory areas of this thread. This includes the stack.
    // We can use the "interrupt stack" temporarily, because it's per-core and we are guaranteed to leave it alone
    // when the next thread is selected. An NMI does not use this IST.
    cli
    .extern INTERRUPT_STACK_TOP
    movq $INTERRUPT_STACK_TOP, %rsp

    call _switch_to_next

    // Should not get here
    ud2

.global _check_should_schedule
.type _check_should_schedule, @function
_check_should_schedule:
    cmpb $0, %gs:12
    jnz 1f
    ret
1:
    jmp _switch_to_next
