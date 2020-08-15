use crate::sync::spinlock::RwLock;
use crate::tasking::file::{FileDescriptor, FileHandle};
use crate::tasking::scheme::{Scheme, SchemePtr};
use alloc::boxed::Box;
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use spin::Once;

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
}

impl SchemeContainer {
    /// Creates a new scheme container.
    fn new() -> Self {
        Self {
            name_scheme_map: BTreeMap::new(),
        }
    }

    /// Inserts a new scheme.
    pub fn insert(&mut self, name: Box<[u8]>, scheme: Scheme) -> Result<(), SchemeInsertionError> {
        match self.name_scheme_map.entry(name) {
            Entry::Occupied(_) => Err(SchemeInsertionError::NameAlreadyTaken),
            Entry::Vacant(v) => {
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

    pub fn open_self(&self, name: Box<[u8]>) -> Result<FileDescriptor, usize> {
        // TODO: errno nodevice ?
        let (_, w) = self.name_scheme_map.get(&name).ok_or(1usize)?;
        Ok(FileDescriptor::from(w.clone(), FileHandle::Own))
    }

    pub fn open(&self, name: Box<[u8]>) -> Result<FileDescriptor, usize> {
        // TODO: errno nodevice ?
        let (a, w) = self.name_scheme_map.get(&name).ok_or(1usize)?;
        // TODO: filename arg
        a.open()
            .map(|handle| FileDescriptor::from(w.clone(), handle))
    }
}

static SCHEMES: Once<RwLock<SchemeContainer>> = Once::new();

/// Gets the schemes.
pub fn schemes() -> &'static RwLock<SchemeContainer> {
    SCHEMES.call_once(|| {
        let mut container = SchemeContainer::new();

        container
            .insert(Box::new([]), Scheme::new())
            .expect("add self");

        RwLock::new(container)
    })
}
