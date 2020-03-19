//! Basic, read-only, in-memory tar support.

use core::marker::PhantomData;
use core::slice;

/// Tar standard Posix header.
#[repr(C, align(512))]
struct PosixHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 11],
    size_zero_byte: u8,
    mktime: [u8; 12],
    chksum: [u8; 8],
    typeflag: u8,
    linkname: [u8; 100],
    magic: [u8; 6],
    version: [u8; 2],
    uname: [u8; 32],
    gname: [u8; 32],
    devmajor: [u8; 8],
    devminor: [u8; 8],
    prefix: [u8; 155],
}

/// Representation of a tar archive.
pub struct Tar<'a> {
    contents: &'a [u8],
}

/// Representation of a file in a tar archive.
#[derive(Debug)]
pub struct TarFile<'a> {
    data: &'a [u8],
}

/// Iterator for the files in a tar archive.
pub struct TarIterator<'a> {
    ptr: *const PosixHeader,
    end: *const PosixHeader,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Tar<'a> {
    /// Creates a new in-memory tar.
    pub unsafe fn from_slice(contents: &'a [u8]) -> Option<Self> {
        if contents.len() % 512 != 0 {
            None
        } else {
            Some(Self { contents })
        }
    }
}

impl<'a> TarFile<'a> {
    /// Gets the file contents as a slice.
    pub fn as_slice(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> TarIterator<'a> {
    /// Converts an octal string to a number.
    fn octal_string_to_number(&self, str: &'a [u8]) -> Option<usize> {
        str.iter().try_fold(0, |sum, c| match *c {
            b'0'..=b'9' => Some(sum * 8 + (*c - b'0') as usize),
            _ => None,
        })
    }
}

impl<'a> IntoIterator for Tar<'a> {
    type Item = TarFile<'a>;
    type IntoIter = TarIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let ptr = self.contents.as_ptr() as *const PosixHeader;
        TarIterator {
            ptr,
            end: unsafe { ptr.offset((self.contents.len() / 512) as isize) },
            _phantom: PhantomData,
        }
    }
}

impl<'a> Iterator for TarIterator<'a> {
    type Item = TarFile<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr >= self.end {
            return None;
        }

        let header = unsafe { &*self.ptr };

        if header.name[0] == 0 {
            return None;
        }

        let size = self.octal_string_to_number(&header.size)?;
        let data_ptr = unsafe { self.ptr.offset(1) };

        self.ptr = unsafe { data_ptr.offset(((size + 512 - 1) / 512) as isize) };

        if self.ptr >= self.end {
            return None;
        }

        Some(TarFile {
            data: unsafe { slice::from_raw_parts(data_ptr as *const u8, size) },
        })
    }
}
