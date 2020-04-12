#!/usr/bin/env python3

import struct
import os
import tempfile

list = [0x55, 0x48, 0x89, 0xe5, 0xb8, 0x1, 0x0, 0x0, 0x0, 0x5d, 0xc3, 0x55, 0x48, 0x89, 0xe5, 0xb8, 0x2, 0x0, 0x0, 0x0, 0x5d, 0xc3, 0x55, 0x48, 0x89, 0xe5, 0xb8, 0x3, 0x0, 0x0, 0x0, 0x5d, 0xc3, 0x55, 0x48, 0x89, 0xe5, 0xb8, 0x4, 0x0, 0x0, 0x0, 0x5d, 0xc3, 0x55, 0x48, 0x89, 0xe5, 0xb8, 0x5, 0x0, 0x0, 0x0, 0x5d, 0xc3, 0x55, 0x48, 0x89, 0xe5, 0x40, 0x89, 0xf0, 0x83, 0xc0, 0xfd, 0x83, 0xf8, 0x7, 0x77, 0x1e, 0x83, 0xf8, 0x8, 0x73, 0x12, 0x89, 0xc0, 0x48, 0x8d, 0xd, 0x2c, 0x0, 0x0, 0x0, 0x48, 0x63, 0x4, 0x81, 0x48, 0x1, 0xc1, 0xff, 0xe1, 0xe8, 0x9e, 0xff, 0xff, 0xff, 0x5d, 0xc3, 0xe8, 0xc3, 0xff, 0xff, 0xff, 0x5d, 0xc3, 0xe8, 0x9b, 0xff, 0xff, 0xff, 0x5d, 0xc3, 0xe8, 0x9f, 0xff, 0xff, 0xff, 0x5d, 0xc3, 0xe8, 0xa3, 0xff, 0xff, 0xff, 0x5d, 0xc3, 0xdd, 0xff, 0xff, 0xff, 0xe4, 0xff, 0xff, 0xff, 0xe4, 0xff, 0xff, 0xff, 0xe4, 0xff, 0xff, 0xff, 0xe4, 0xff, 0xff, 0xff, 0xeb, 0xff, 0xff, 0xff, 0xf2, 0xff, 0xff, 0xff, 0xf9, 0xff, 0xff, 0xff, 0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x48, 0x83, 0xec, 0x8, 0x48, 0x89, 0xbc, 0x24, 0x0, 0x0, 0x0, 0x0, 0xb8, 0x3, 0x0, 0x0, 0x0, 0x49, 0x89, 0xff, 0x4c, 0x89, 0xff, 0x40, 0x89, 0xc6, 0xe8, 0x72, 0xff, 0xff, 0xff, 0x4c, 0x8b, 0xbc, 0x24, 0x0, 0x0, 0x0, 0x0, 0x49, 0x8b, 0x4f, 0x8, 0x4c, 0x89, 0xff, 0x40, 0x89, 0xc6, 0xff, 0xd1, 0x48, 0x83, 0xc4, 0x8, 0x41, 0x5f, 0x5d, 0xc3]

out = tempfile.NamedTemporaryFile(mode = 'wb')
out.write(struct.pack(f'{len(list)}B', *list))
out.flush()
os.system(f'objdump -b binary -D -m i386:x86-64 {out.name}')
out.close()
