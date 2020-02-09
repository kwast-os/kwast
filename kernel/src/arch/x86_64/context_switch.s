// AMD64 ABI tells us that only rbx, rbp, r12 - r15 need to be preserved by the callee.
// switch_to(new_stack, old_thread_id)
.global switch_to
.type switch_to, @function
switch_to:
    pushfq
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    // The `new_stack` argument is in %rdi.
    // We switch stacks so our rdi will contain the old stack and the rsp the new stack.
    // That means the call to `save_thread_state` will have the right arguments.
    xchg %rdi, %rsp
    .extern save_thread_state
    call save_thread_state

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popfq

    ret
