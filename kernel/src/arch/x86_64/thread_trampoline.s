.section .text

.global thread_trampoline
.type thread_trampoline, @function

thread_trampoline:
    // The stack is page aligned right now, so also 16 byte like it should for the System V ABI.
    movq %rbp, %rdi
    callq *%rbx

    // Should not get here. Applications should call exit.
    ud2
