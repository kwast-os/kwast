use crate::tasking::scheme::{Scheme, SchemePtr};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Maximum amount of files a single table can have opened.
const MAX_FILES: usize = 32;

/// File index in file descriptor table.
pub type FileIdx = usize;

/// File handle used in a scheme (per-scheme).
pub enum FileHandle {
    /// A handle to a file in the scheme.
    /// This should be handled by the service.
    Inner(u64),
    /// A handle to the scheme itself.
    Own,
}

pub struct FileDescriptor {
    scheme: SchemePtr,
    handle: FileHandle,
    /// Files can be pre-opened and even mapped to a different name.
    /// Keep track of this because WASI needs it.
    pre_open_path: Option<Box<[u8]>>,
}

pub struct FileDescriptorTable {
    /// File descriptor table.
    /// Note: there can be holes, which is why we need Option.
    files: Vec<Option<FileDescriptor>>,
}

impl FileDescriptor {
    /// Creates a file descriptor from scheme data.
    pub fn from(scheme: SchemePtr, handle: FileHandle) -> Self {
        Self {
            scheme,
            handle,
            pre_open_path: None,
        }
    }

    /// Pre open path.
    pub fn pre_open_path(&self) -> Option<&[u8]> {
        self.pre_open_path.as_ref().map(|path| &path[..])
    }

    /// Sets the pre open path.
    pub fn set_pre_open_path(&mut self, path: Box<[u8]>) {
        self.pre_open_path = Some(path);
    }
}

impl FileDescriptorTable {
    /// Creates a new file descriptor table.
    pub fn new() -> Self {
        //Self { files: Vec::new() }
        // TODO
        Self {
            files: vec![None, None, None],
        }
    }

    /// Insert file into lowest available index.
    pub fn insert_lowest(&mut self, fd: FileDescriptor) -> Option<FileIdx> {
        for (idx, file) in self.files.iter_mut().enumerate() {
            // TODO: debug
            if idx < 3 {
                continue;
            }

            if file.is_none() {
                *file = Some(fd);
                return Some(idx);
            }
        }

        if self.files.len() < MAX_FILES {
            self.files.push(Some(fd));
            Some(self.files.len() - 1)
        } else {
            None
        }
    }

    /// Gets a file descriptor.
    pub fn get(&self, idx: FileIdx) -> Option<&FileDescriptor> {
        self.files.get(idx).unwrap_or(&None).as_ref()
    }
}

impl Default for FileDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}
