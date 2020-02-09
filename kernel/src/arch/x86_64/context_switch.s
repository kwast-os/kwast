// AMD64 ABI tells us that only rbx, rbp, r12 - r15 need to be preserved by the callee.
// switch_to(new_stack)
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

    // The `new_stack` argument is in %rdi
    movq %rdi, %rsp // TODO: need to call "save my thread" here.

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popfq

    ret
