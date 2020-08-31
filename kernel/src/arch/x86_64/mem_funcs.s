.section .text

.global page_clear
.type page_clear, @function
// page_clear(destination)
page_clear:
    sub $16, %rsp
    movdqu %xmm0, (%rsp)
    pxor %xmm0, %xmm0
    mov $4096, %ecx
1:
    movdqa %xmm0, 0(%rdi)
    movdqa %xmm0, 16(%rdi)
    movdqa %xmm0, 32(%rdi)
    movdqa %xmm0, 48(%rdi)
    add $64, %rdi
    sub $64, %ecx
    jnz 1b
    movdqu (%rsp), %xmm0
    add $16, %rsp
    ret
