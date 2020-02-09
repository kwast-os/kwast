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

    .extern next_thread_state
    .type next_thread_state, @function
    mov %rsp, %rdi
    call next_thread_state
    movq %rax, %rsp

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popfq

    ret
