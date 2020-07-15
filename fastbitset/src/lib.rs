#![no_std]

extern crate alloc;

use alloc::vec::Vec;

#[derive(Debug, Default)]
pub struct BitSet {
    level0: u64,
    level1: Vec<u64>,
    level2: Vec<u64>,
}

impl BitSet {
    pub const MAX_SIZE: usize = 64 * 64 * 64;

    pub fn new() -> Self {
        BitSet {
            level0: 0,
            level1: Vec::new(),
            level2: Vec::new(),
        }
    }

    /// Returns first set bit index.
    pub fn find_set(&self) -> Option<usize> {
        match self.level0.trailing_zeros() as usize {
            64 => None,
            i0 => {
                debug_assert!(
                        self.level1.len() > i0,
                        "Set bit guarantees that next level has non-zero value at that index"
                    );
                let i1 = unsafe {
                    // Bit was set in upper level.
                    // Thus there must be non zero u64.
                    self.level1.get_unchecked(i0)
                }
                .trailing_zeros() as usize;
                debug_assert_ne!(
                        i1, 64,
                        "Set bit in higher level means this level must has at least one bit set"
                    );
                let i1 = (i0 << 6) + i1;
                debug_assert!(
                        self.level2.len() > i1,
                        "Set bit guarantees that next level has non-zero value at that index"
                    );
                let i2 = unsafe {
                    // Bit was set in upper level.
                    // Thus there must be non zero u64.
                    self.level2.get_unchecked(i1)
                }
                .trailing_zeros() as usize;
                debug_assert_ne!(
                        i2, 64,
                        "Set bit in higher level means this level must has at least one bit set"
                    );
                Some((i1 << 6) + i2)
            }
        }
    }

    /// Adds new bit.
    /// Bits must be added in natural order.
    /// `index` must not exceed `64 ^ 3 - 1`
    pub unsafe fn add_unchecked(&mut self, index: usize) {
        let i0 = index >> 12;
        let i1 = (index >> 6) & 63;
        let i2 = index & 63;
        debug_assert_eq!(i0 & 63, i0, "`index` must not exceed `64 ^ 3 - 1`");
        if i2 == 0 {
            self.level2.push(1);
        } else {
            debug_assert!(self.level2.len() > i1);
            *self.level2.get_unchecked_mut(i1) |= 1 << i2;
        }
        if i2 == 0 && i1 == 0 {
            self.level1.push(1);
        } else {
            debug_assert!(self.level1.len() > i0);
            *self.level1.get_unchecked_mut(i0) |= 1 << i1;
        }
        self.level0 |= 1 << i0;
    }

    /// Sets previously added bit.
    pub unsafe fn set_unchecked(&mut self, index: usize) {
        let i0 = index >> 12;
        let i1 = (index >> 6) & 63;
        let i2 = index & 63;
        debug_assert_eq!(i2 & 63, i2);
        debug_assert!(self.level2.len() > i1);
        debug_assert!(self.level1.len() > i0);

        *self.level2.get_unchecked_mut(i1) |= 1 << i2;
        *self.level1.get_unchecked_mut(i0) |= 1 << i1;
        self.level0 |= 1 << i0;
    }

    /// Sets previously added bit.
    pub unsafe fn unset_unchecked(&mut self, index: usize) {
        let i0 = index >> 12;
        let i1 = (index >> 6) & 63;
        let i2 = index & 63;

        debug_assert_eq!(i0 & 63, i0, "`index` must not exceed `64 ^ 3 - 1`");
        debug_assert!(self.level2.len() > i1);
        debug_assert!(self.level1.len() > i0);

        *self.level2.get_unchecked_mut(i1) &= !(1 << i2);
        *self.level1.get_unchecked_mut(i0) &= !(1 << i1);
        self.level0 &= !(1 << i0);
    }

    /// Sets bit. Extenging bitset storage if necessary.
    pub fn set(&mut self, index: usize) {
        let i0 = index >> 12;
        let i1 = (index >> 6) & 63;
        let i2 = index & 63;

        assert_eq!(i2 & 63, i2);
        if self.level2.len() <= i1 {
            self.level2.resize(i1 + 1, 0);
        }
        if self.level1.len() <= i0 {
            self.level1.resize(i0 + 1, 0);
        }
        unsafe {
            *self.level2.get_unchecked_mut(i1) |= 1 << i2;
            *self.level1.get_unchecked_mut(i0) |= 1 << i1;
        }
        self.level0 |= 1 << i0;
    }

    /// Unsets bit.
    pub fn unset(&mut self, index: usize) {
        let i0 = index >> 12;
        let i1 = (index >> 6) & 63;
        let i2 = index & 63;

        assert_eq!(i0 & 63, i0, "`index` must not exceed `64 ^ 3 - 1`");
        if self.level2.len() <= i1 {
            return;
        }
        if self.level1.len() <= i0 {
            return;
        }
        unsafe {
            *self.level2.get_unchecked_mut(i1) &= !(1 << i2);
            *self.level1.get_unchecked_mut(i0) &= !(1 << i1);
        }
        self.level0 &= !(1 << i0);
    }
}
