.global _switch_to_next
.type _switch_to_next, @function

.global wasm_trampoline
.type wasm_trampoline, @function

wasm_trampoline:
    // TODO: stack alignment?
    movq %rbp, %rdi
    callq *%rbx

    // Exit
    movl $1, %edi
    call _switch_to_next
    ud2
