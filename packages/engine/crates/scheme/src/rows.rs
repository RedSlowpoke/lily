//! A list whose copies share what neither has changed (D259): N8's snapshots
//! and the translation they are of hold the same records, and most of a
//! translation's records never change after the bar that made them.
//!
//! The items are kept in rows of [`ROW`], each behind an [`Rc`]. A copy
//! copies the pointers. A write to a row that a copy holds too copies that
//! row first ([`Rc::make_mut`]), so a run pays for what it changes since its
//! last copy, not for its length. A list never moves its items, so it needs
//! no one allocation as long as itself: a record's table in a long score
//! doubles no block, which is what fills wasm's memory first.
//!
//! The collector reaches a row once however many copies hold it
//! ([`Rows::trace`]): a row's values are the same in all of them.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::Cell;
use core::ops::{Index, IndexMut};

use crate::heap::Tracer;

/// Items in a row. A write to a shared row copies this many, so it is small
/// next to what a snapshot would copy, and the pointers are small next to it.
pub const ROW: usize = 64;

struct Row<T> {
    items: Vec<T>,
    /// The last collection that traced it.
    traced: Cell<u32>,
}

impl<T: Clone> Clone for Row<T> {
    fn clone(&self) -> Row<T> {
        Row {
            items: self.items.clone(),
            traced: Cell::new(0),
        }
    }
}

/// See the module.
pub struct Rows<T> {
    rows: Vec<Rc<Row<T>>>,
    len: usize,
}

impl<T> Default for Rows<T> {
    fn default() -> Rows<T> {
        Rows {
            rows: Vec::new(),
            len: 0,
        }
    }
}

impl<T> Clone for Rows<T> {
    fn clone(&self) -> Rows<T> {
        Rows {
            rows: self.rows.clone(),
            len: self.len,
        }
    }
}

impl<T> Rows<T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        (index < self.len).then(|| &self.rows[index / ROW].items[index % ROW])
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> {
        self.rows.iter().flat_map(|row| row.items.iter())
    }

    /// The items from `index` on.
    pub fn iter_from(&self, index: usize) -> impl Iterator<Item = &T> {
        let from = self.rows.get(index / ROW..).unwrap_or_default();
        from.iter()
            .flat_map(|row| row.items.iter())
            .skip(index % ROW)
    }

    /// The first of the first `end` items for which `before` is false, those
    /// it is true of all before it, as `slice::partition_point`.
    pub fn partition_point(&self, end: usize, before: impl Fn(&T) -> bool) -> usize {
        let (mut low, mut high) = (0, end.min(self.len));
        while low < high {
            let middle = low + (high - low) / 2;
            if before(&self[middle]) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low
    }

    /// Traces each item of each row no copy has shown this collection.
    pub fn trace(&self, tracer: &mut Tracer<'_>, mut each: impl FnMut(&T, &mut Tracer<'_>)) {
        for row in &self.rows {
            if tracer.first_time(&row.traced) {
                for item in &row.items {
                    each(item, tracer);
                }
            }
        }
    }

    /// Whether the row of `index` is the one `other` has there, so that the
    /// two lists hold the same item.
    pub fn shares(&self, other: &Rows<T>, index: usize) -> bool {
        match (self.rows.get(index / ROW), other.rows.get(index / ROW)) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl<T: Clone> Rows<T> {
    pub fn push(&mut self, item: T) {
        if self.len.is_multiple_of(ROW) {
            self.rows.push(Rc::new(Row {
                items: Vec::with_capacity(ROW),
                traced: Cell::new(0),
            }));
        }
        let row = self.rows.last_mut().expect("a row was just made");
        Rc::make_mut(row).items.push(item);
        self.len += 1;
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        (index < self.len)
            .then(|| &mut Rc::make_mut(&mut self.rows[index / ROW]).items[index % ROW])
    }
}

impl<T> Index<usize> for Rows<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        assert!(index < self.len, "row {index} of {}", self.len);
        &self.rows[index / ROW].items[index % ROW]
    }
}

impl<T: Clone> IndexMut<usize> for Rows<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        assert!(index < self.len, "row {index} of {}", self.len);
        &mut Rc::make_mut(&mut self.rows[index / ROW]).items[index % ROW]
    }
}

impl<T> FromIterator<T> for Rows<T>
where
    T: Clone,
{
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Rows<T> {
        let mut rows = Rows::default();
        for item in items {
            rows.push(item);
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(n: u32) -> Rows<u32> {
        (0..n).collect()
    }

    #[test]
    fn it_reads_as_a_list() {
        let rows = numbers(ROW as u32 * 3 + 5);
        assert_eq!(rows.len(), ROW * 3 + 5);
        assert_eq!(rows[ROW + 1], ROW as u32 + 1);
        assert_eq!(rows.get(rows.len()), None);
        assert_eq!(rows.iter().next_back(), Some(&(ROW as u32 * 3 + 4)));
        assert!(rows.iter().copied().eq(0..ROW as u32 * 3 + 5));
        assert!(
            rows.iter_from(ROW + 2)
                .copied()
                .eq(ROW as u32 + 2..ROW as u32 * 3 + 5)
        );
        assert_eq!(rows.iter_from(rows.len()).count(), 0);
        assert_eq!(rows.partition_point(rows.len(), |&x| x < 70), 70);
        assert_eq!(rows.partition_point(5, |&x| x < 70), 5);
    }

    #[test]
    fn a_copy_keeps_what_it_held_and_shares_what_is_not_written() {
        let mut rows = numbers(ROW as u32 * 2);
        let copy = rows.clone();
        assert!(rows.shares(&copy, 0) && rows.shares(&copy, ROW));
        rows[3] = 1000;
        rows.push(7);
        assert_eq!((rows[3], copy[3]), (1000, 3));
        assert_eq!((rows.len(), copy.len()), (ROW * 2 + 1, ROW * 2));
        assert!(!rows.shares(&copy, 0), "the row written is its own");
        assert!(rows.shares(&copy, ROW), "the other is not");
    }

    #[test]
    fn a_full_row_is_left_as_it_is_by_a_push() {
        let mut rows = numbers(ROW as u32);
        let copy = rows.clone();
        rows.push(1);
        assert!(rows.shares(&copy, 0));
        assert_eq!(copy.len(), ROW);
    }
}
