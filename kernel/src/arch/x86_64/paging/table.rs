use core::marker::PhantomData;

use crate::arch::x86_64::address::VirtAddr;

use super::entry::*;
use super::MemoryError;
use crate::mm::pmm::with_pmm;

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

impl<L> Table<L>
where
    L: Level,
{
    /// Clears the table entries. (internal use only)
    pub fn clear(&mut self) {
        for e in self.entries.iter_mut() {
            e.clear();
        }
    }

    /// Gets the used count.
    pub fn used_count(&self) -> u64 {
        self.entries[0].used_count()
    }

    /// Sets the used count.
    pub fn set_used_count(&mut self, count: u64) {
        self.entries[0].set_used_count(count);
    }

    /// Increases the used count.
    pub fn increase_used_count(&mut self) {
        self.set_used_count(self.used_count() + 1);
    }

    /// Decreases the used count.
    pub fn decrease_used_count(&mut self) {
        debug_assert!(self.used_count() > 0);
        self.set_used_count(self.used_count() - 1);
    }
}

impl<L> Table<L>
where
    L: HierarchicalLevel,
{
    /// Gets the next table address (unchecked). (internal use only).
    fn next_table_address_unchecked(&self, index: usize) -> usize {
        let addr = self as *const _ as usize;
        (addr << 9) | (index << 12)
    }

    /// Gets the next table address.
    fn next_table_address(&self, index: usize) -> Option<usize> {
        let flags = self.entries[index].flags();

        // Would be invalid if we refer to a huge page
        debug_assert!(!flags.contains(EntryFlags::HUGE_PAGE));

        if flags.contains(EntryFlags::PRESENT) {
            Some(self.next_table_address_unchecked(index))
        } else {
            None
        }
    }

    /// Gets the next table level.
    pub fn next_table(&self, index: usize) -> Option<&Table<L::NextLevel>> {
        self.next_table_address(index)
            .map(|x| unsafe { &*(x as *const _) })
    }

    /// Gets the next table level (mutable).
    pub fn next_table_mut(&self, index: usize) -> Option<&mut Table<L::NextLevel>> {
        self.next_table_address(index)
            .map(|x| unsafe { &mut *(x as *mut _) })
    }

    /// Gets the next table (mutable), creates it if it doesn't exist yet.
    pub fn next_table_may_create(
        &mut self,
        index: usize,
    ) -> Result<&mut Table<L::NextLevel>, MemoryError> {
        let flags = self.entries[index].flags();
        debug_assert!(!flags.contains(EntryFlags::HUGE_PAGE));

        let addr = self.next_table_address_unchecked(index);
        let table = unsafe { &mut *(addr as *mut Table<L::NextLevel>) };

        // Need to create a table.
        if !flags.contains(EntryFlags::PRESENT) {
            // We could call the page mapping functions here, but it would be slower than
            // manipulating the pmm ourselves.
            with_pmm(|pmm| {
                pmm.pop_top(|top| {
                    // We don't need to invalidate because it wasn't present.
                    self.entries[index].set(top, EntryFlags::PRESENT | EntryFlags::WRITABLE);

                    VirtAddr::new(addr)
                })
            })?;

            self.increase_used_count();

            table.clear();
        }

        Ok(table)
    }
}
