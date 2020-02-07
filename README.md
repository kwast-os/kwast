<p align="center">
<img alt="Kwast" src="https://github.com/nielsdos/kwast/raw/master/docs/small_logo.png">

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE) [![Build Status](https://travis-ci.org/nielsdos/kwast.svg?branch=master)](https://travis-ci.org/nielsdos/kwast)
</p>

**Kwast** (will be) an operating system, written in Rust, running WebAssembly. It uses a microkernel architecture for flexibility.

Since WebAssembly was designed to be a safe language, we can run it without having to use hardware usermode and multiple address spaces.
This enables low-cost context switches, low-cost syscalls, and a microkernel design without a big performance hit.
Another interesting thing is that it means the software is cross-platform and that the compiler could enable platform-specific optimisations.

For notes on Spectre, Meltdown and other related issues, see [#10](https://github.com/nielsdos/kwast/issues/10).

## Getting Started

These instructions help you get started with building the source and getting it to run.

### Requirements

* make
* grub-mkrescue (you might also need to install xorriso)
* qemu-system-x86_64
* Rust and Cargo

### Setting up a toolchain

You can setup your toolchain using the following steps:
```bash
# You'll need to get the rust nightly and install cargo-xbuild:
rustup override set nightly
rustup component add rust-src
cargo install cargo-xbuild

# You'll also need a cross-compile binutils, I wrote a bash script that builds this for you.
cd toolchain
./setup_cross_binutils.sh
```
Now you're ready to build and run the project!

### Building & Running

There's currently a Makefile in the `kernel` folder. The **Makefile** there provides some rules:

```bash
cd kernel # If not already there
make run # Builds iso and start a QEMU virtual machine
make iso # Only builds the iso

# You can make a release build using:
make iso BUILD=release # (or run)

# You can run tests using
./run_tests
```

### Contributing
Interested in contributing to the project? Check the issues.

## Goals

### Short-term goals
* Multitasking
* Simple PS/2 server & similar small servers
* SMP

### Personal goals
* Port my C++ kernel to Rust
* Improve my Rust skills
* Get a better understanding of WebAssembly

## Built With

* [Cranelift](https://github.com/bytecodealliance/cranelift) - Code generator used to parse & run WebAssembly. Kwast uses a fork of Cranelift to let it work in a no_std environment.

* To integrate Cranelift, [wasmtime](https://github.com/bytecodealliance/wasmtime/) has been used as a reference implementation, which is licensed under the [Apache License 2.0](https://github.com/bytecodealliance/wasmtime/blob/master/LICENSE).

## Similar projects
* [Nebulet](https://github.com/nebulet/nebulet) - A microkernel that implements a WebAssembly "usermode" that runs in Ring 0
* [wasmjit](https://github.com/kenny-ngo/wasmjit) - Small Embeddable WebAssembly Runtime
* [cervus](https://github.com/cervus-v/cervus) - A WebAssembly subsystem for Linux
