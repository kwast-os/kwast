use crate::tasking::scheme::Scheme;
use alloc::boxed::Box;
use alloc::sync::Weak;
use alloc::vec::Vec;

const MAX_FILES: usize = 32;

/// File index in file descriptor table.
pub type FileIdx = usize;

/// File handle used in a scheme (per-scheme).
pub type FileHandleInScheme = usize;

pub struct FileDescriptor {
    scheme: Weak<Scheme>,
    handle: FileHandleInScheme,
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
    /// Pre open path.
    pub fn pre_open_path(&self) -> Option<&[u8]> {
        self.pre_open_path.as_ref().map(|path| &path[..])
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
