/// Nibble array.
// TODO: Once https://github.com/rust-lang/rust/issues/68567 is fixed, we can do the ugly
//       calculation in here.
struct NibbleArray<const N: usize> {
    /// Entries, stored as nibbles.
    entries: [u8; N],
}

impl<const N: usize> NibbleArray<N> {
    /// Gets the shift for the nibble.
    #[inline]
    fn get_shift(index: usize) -> u8 {
        ((index & 1) << 2) as u8
    }

    /// Gets a nibble at a logical index.
    fn get_nibble_at(&self, index: usize) -> u8 {
        (self.entries[index >> 1] >> NibbleArray::<N>::get_shift(index)) & 15
    }

    /// Sets a nibble at a logical index to a value.
    fn set_nibble_at(&mut self, index: usize, value: u8) {
        debug_assert!(value < 16);
        let shift = NibbleArray::<N>::get_shift(index);
        let masked = self.entries[index >> 1] & (0b11110000 >> shift);
        self.entries[index >> 1] = masked | (value << shift);
    }
}
