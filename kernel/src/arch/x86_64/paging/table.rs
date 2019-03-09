use core::marker::PhantomData;

use crate::arch::x86_64::paging::entry::*;
use crate::arch::x86_64::paging::MappingError;

// We use the clever solution for static type safety as described by [Philipp Oppermann's blog](https://os.phil-opp.com/)

pub trait Level {}

pub trait HierarchicalLevel: Level {
    type NextLevel: Level;
}

pub enum Level4 {}

pub enum Level3 {}

pub enum Level2 {}

pub enum Level1 {}

impl Level for Level4 {}

impl Level for Level3 {}

impl Level for Level2 {}

impl Level for Level1 {}

impl HierarchicalLevel for Level4 {
    type NextLevel = Level3;
}

impl HierarchicalLevel for Level3 {
    type NextLevel = Level2;
}

impl HierarchicalLevel for Level2 {
    type NextLevel = Level1;
}

#[repr(transparent)]
pub struct Table<L: Level> {
    pub entries: [Entry; 512],
    // Rust doesn't allow unused type parameters.
    _phantom: PhantomData<L>,
}

impl<L> Table<L> where L: HierarchicalLevel {
    /// Gets the next table address (unchecked).
    /// Internal use only!
    fn next_table_address_unchecked(&self, index: usize) -> usize {
        let addr = self as *const _ as usize;
        (addr << 9) | (index << 12)
    }

    /// Gets the next table address.
    fn next_table_address(&self, index: usize) -> Option<usize> {
        let entry = self.entries[index].flags();

        if entry.contains(EntryFlags::PRESENT) {
            Some(self.next_table_address_unchecked(index))
        } else {
            None
        }
    }

    /// Gets the next table level.
    pub fn next_table(&self, index: usize) -> Option<&Table<L::NextLevel>> {
        self.next_table_address(index).map(|x| unsafe { &*(x as *const _) })
    }

    /// Gets the next table (mutable), creates it if it doesn't exist yet.
    pub fn next_table_may_create(&mut self, index: usize) -> Result<&mut Table<L::NextLevel>, MappingError> {
        let entry = self.entries[index].flags();
        let addr = self.next_table_address_unchecked(index);

        if entry.contains(EntryFlags::PRESENT) {
            Ok(unsafe { &mut *(addr as *mut _) })
        } else {
            // TODO: allocate phys adress and put in table
            Err(MappingError::OOM)
        }
    }
}
