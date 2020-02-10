#!/usr/bin/env python3

import struct
import os
import tempfile

list = [0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x48, 0x83, 0xec, 0x8, 0x89, 0xbc, 0x24, 0x4, 0x0, 0x0, 0x0, 0x41, 0x89, 0xff, 0x44, 0x89, 0xff, 0xe8, 0xe4, 0xff, 0xff, 0xff, 0x44, 0x8b, 0xbc, 0x24, 0x4, 0x0, 0x0, 0x0, 0x44, 0x1, 0xf8, 0x48, 0x83, 0xc4, 0x8, 0x41, 0x5f, 0x5d, 0xc3]
list = [0x55, 0x48, 0x89, 0xe5, 0xb8, 0xff, 0xff, 0xff, 0xff, 0xb9, 0xad, 0xde, 0xad, 0xde, 0x89, 0xc0, 0x48, 0x8b, 0x17, 0x89, 0xc, 0x2, 0x5d, 0xc3]

out = tempfile.NamedTemporaryFile(mode = 'wb')
out.write(struct.pack(f'{len(list)}B', *list))
out.flush()
os.system(f'objdump -b binary -D -m i386:x86-64 {out.name}')
out.close()
