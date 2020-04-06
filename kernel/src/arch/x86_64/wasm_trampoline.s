.global _switch_to_next
.type _switch_to_next, @function

.global wasm_trampoline
.type wasm_trampoline, @function

wasm_trampoline:
    // The stack is page aligned right now, so also 16 byte like it should for the System V ABI.
    movq %rbp, %rdi
    callq *%rbx
    // Applications should call exit
    ud2
