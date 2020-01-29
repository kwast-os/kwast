#!/bin/sh

cookie='built-2020-01-29.cookie'
cross_rel_path=./opt/cross

if [ ! -d $cross_rel_path ]; then
    mkdir -p $cross_rel_path
fi

cross_path=$(realpath $cross_rel_path)

binutils_ver=2.33.1
binutils="https://ftp.gnu.org/gnu/binutils/binutils-$binutils_ver.tar.xz"
binutils_file=$(basename $binutils)

# This will check if a toolchain was already built.
# If it wasn't, then it'll build. Otherwise it won't.

if [ -f "$cookie" ]; then
    echo Toolchain was already built.
    exit 0
else
    # Cleanup old toolchain
    rm -rf opt build-binutils binutils-*
fi

if [ ! -f $binutils_file ]; then
    if hash curl 2>/dev/null; then
        downloader="curl --output"
    elif hash wget 2>/dev/null; then
        downloader="wget -O"
    else
        echo "Neither curl or wget is available on your system. Download binutils manually from $binutils" >&2
        exit 1
    fi

    $downloader $binutils_file $binutils
fi

sha256sum -c SHA256SUMS
if [ $? -ne 0 ]; then
    echo "SHA256 does not match" >&2
    exit 2
fi

tar xf $binutils_file

mkdir build-binutils
cd build-binutils
../binutils-$binutils_ver/configure --target=x86_64-elf --prefix="$cross_path" \
    --disable-nls --disable-werror \
    --disable-gdb --disable-libdecnumber --disable-readline --disable-sim

make -j2 || exit 1
make install || exit 1
cd ..

touch "$cookie"
