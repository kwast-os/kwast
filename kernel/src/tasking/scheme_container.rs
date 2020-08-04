use crate::sync::spinlock::RwLock;
use crate::tasking::scheme::Scheme;
use alloc::boxed::Box;
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use spin::Once;

/// Error that can occur when inserting a new scheme.
#[derive(Debug)]
pub enum SchemeInsertionError {
    /// The scheme name is already taken.
    NameAlreadyTaken,
}

pub struct SchemeContainer {
    /// Maps a name to an id.
    name_scheme_map: BTreeMap<Box<[u8]>, Arc<Scheme>>,
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
            Entry::Occupied(_) => return Err(SchemeInsertionError::NameAlreadyTaken),
            Entry::Vacant(v) => v.insert(Arc::new(scheme)),
        };

        Ok(())
    }

    /// Gets a scheme by name.
    pub fn get(&mut self, name: Box<[u8]>) -> Option<Weak<Scheme>> {
        self.name_scheme_map
            .get(&name)
            .map(|scheme| Arc::downgrade(scheme))
    }
}

static SCHEMES: Once<RwLock<SchemeContainer>> = Once::new();

/// Gets the schemes.
pub fn schemes() -> &'static RwLock<SchemeContainer> {
    SCHEMES.call_once(|| {
        // TODO
        let mut container = SchemeContainer::new();
        container
            .insert(Box::new(*b"test"), Scheme::new())
            .expect("add test");

        RwLock::new(container)
    })
}
