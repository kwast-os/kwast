use crate::sync::spinlock::RwLock;
use crate::tasking::file::{FileDescriptor, FileHandle};
use crate::tasking::scheme::{Scheme, SchemePtr};
use crate::wasm::wasi::Errno;
use alloc::boxed::Box;
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use spin::Once;

/// Scheme identifier.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct SchemeId(usize);

impl SchemeId {
    /// Sentinel.
    pub const fn sentinel() -> Self {
        Self(0)
    }
}

/// Error that can occur when inserting a new scheme.
#[derive(Debug)]
pub enum SchemeInsertionError {
    /// The scheme name is already taken.
    NameAlreadyTaken,
}

pub struct SchemeContainer {
    /// Maps a name to an id.
    /// It also stores a `SchemePtr` because creating it using `Arc::downgrade` is more expensive
    /// than cloning an already existing one.
    name_scheme_map: BTreeMap<Box<[u8]>, (Arc<Scheme>, SchemePtr)>,
    /// Next scheme id.
    next_scheme_id: usize,
}

impl SchemeContainer {
    /// Creates a new scheme container.
    fn new() -> Self {
        Self {
            name_scheme_map: BTreeMap::new(),
            next_scheme_id: 1,
        }
    }

    /// Inserts a new scheme.
    pub fn insert(&mut self, name: Box<[u8]>) -> Result<(), SchemeInsertionError> {
        match self.name_scheme_map.entry(name) {
            Entry::Occupied(_) => Err(SchemeInsertionError::NameAlreadyTaken),
            Entry::Vacant(v) => {
                let scheme = Scheme::new(SchemeId(self.next_scheme_id));
                self.next_scheme_id += 1;
                let scheme = Arc::new(scheme);
                let weak = Arc::downgrade(&scheme);
                v.insert((scheme, weak));
                Ok(())
            }
        }
    }

    /// Gets a scheme by name.
    //pub fn get(&self, name: Box<[u8]>) -> Option<Arc<Scheme>> {
    //    self.name_scheme_map.get(&name).map(|(a, _)| a).cloned()
    //}

    pub fn open_self(&self, name: Box<[u8]>) -> Result<FileDescriptor, Errno> {
        let (_, w) = self.name_scheme_map.get(&name).ok_or(Errno::NoDev)?;
        Ok(FileDescriptor::from(w.clone(), FileHandle::Own))
    }

    pub fn open(&self, name: Box<[u8]>, i: i32) -> Result<FileDescriptor, Errno> {
        let (a, w) = self.name_scheme_map.get(&name).ok_or(Errno::NoDev)?;
        // TODO: filename arg
        a.open(i)
            .map(|handle| FileDescriptor::from(w.clone(), handle))
    }
}

static SCHEMES: Once<RwLock<SchemeContainer>> = Once::new();

/// Gets the schemes.
pub fn schemes() -> &'static RwLock<SchemeContainer> {
    SCHEMES.call_once(|| {
        let mut container = SchemeContainer::new();

        container.insert(Box::new([])).expect("add self");

        RwLock::new(container)
    })
}
