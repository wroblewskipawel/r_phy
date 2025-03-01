use std::any::type_name;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_guard::test_types::{A, B};

    #[test]
    fn test_push_and_get() {
        let mut collection = GenCollection::default();
        let index1 = collection.push("Item 1").unwrap();
        let index2 = collection.push("Item 2").unwrap();

        assert_eq!(collection.get(index1).unwrap(), &"Item 1");
        assert_eq!(collection.get(index2).unwrap(), &"Item 2");
    }

    #[test]
    fn test_get_mut() {
        let mut collection = GenCollection::default();
        let index = collection.push("Item 1").unwrap();

        {
            let item = collection.get_mut(index).unwrap();
            *item = "Updated Item 1";
        }

        assert_eq!(collection.get(index).unwrap(), &"Updated Item 1");
    }

    #[test]
    fn test_pop() {
        let mut collection = GenCollection::default();
        let index1 = collection.push("Item 1").unwrap();
        let index2 = collection.push("Item 2").unwrap();

        let removed_item = collection.pop(index1).unwrap();
        assert_eq!(removed_item, "Item 1");

        // Verify that the second item is still accessible
        assert_eq!(collection.get(index2).unwrap(), &"Item 2");

        // Attempting to get the removed item should fail
        assert!(collection.get(index1).is_err());
    }

    #[test]
    fn test_pop_last() {
        let mut collection = GenCollection::default();
        let index = collection.push("Last Item").unwrap();

        let removed_item = collection.pop(index).unwrap();
        assert_eq!(removed_item, "Last Item");

        // Verify that the collection is now empty
        assert!(collection.get(index).is_err());
    }

    #[test]
    fn test_invalid_index() {
        let collection: GenCollection<&str> = GenCollection::default();
        let invalid_index = GenIndex::wrap(0, 999); // Invalid index

        assert!(collection.get(invalid_index).is_err());
    }

    #[test]
    fn test_generation_mismatch() {
        let mut collection = GenCollection::default();
        let index = collection.push("Item 1").unwrap();

        // Manually create an index with an incorrect generation
        let invalid_index = GenIndex::wrap(index.generation + 1, index.index);

        // Attempting to get or pop with the invalid index should fail
        assert!(collection.get(invalid_index).is_err());
        assert!(collection.pop(invalid_index).is_err());
    }

    #[test]
    fn test_iter() {
        let mut collection = GenCollection::default();
        collection.push("Item 1").unwrap();
        collection.push("Item 2").unwrap();

        let items: Vec<_> = (&collection).into_iter().cloned().collect();
        assert_eq!(items, vec!["Item 1", "Item 2"]);
    }

    #[test]
    fn test_iter_mut() {
        let mut collection = GenCollection::default();
        collection.push("Item 1").unwrap();
        collection.push("Item 2").unwrap();

        for item in &mut collection {
            *item = "Updated";
        }

        let items: Vec<_> = (&collection).into_iter().cloned().collect();
        assert_eq!(items, vec!["Updated", "Updated"]);
    }

    #[test]
    fn test_into_iter() {
        let mut collection = GenCollection::default();
        collection.push("Item 1").unwrap();
        collection.push("Item 2").unwrap();

        let items: Vec<_> = collection.into_iter().collect();
        assert_eq!(items, vec!["Item 1", "Item 2"]);
    }

    #[test]
    fn test_reuse_freed_cells() {
        let mut collection = GenCollection::default();
        let index1 = collection.push("Item 1").unwrap();
        let _index2 = collection.push("Item 2").unwrap();

        // Pop the first item, freeing its cell
        collection.pop(index1).unwrap();

        // Push a new item and check if it reuses the freed cell
        let index3 = collection.push("Item 3").unwrap();

        // The new index should reuse the old index1 position
        assert_eq!(index3.index, index1.index);
        assert_eq!(collection.get(index3).unwrap(), &"Item 3");
    }

    #[test]
    fn test_guard_collection_entry_valid_index() {
        let mut collection = TypeGuardCollection::<u32>::default();
        let index_a = collection.push(A(42).into_guard()).unwrap();
        let index_b = collection.push(B(31).into_guard()).unwrap();

        let entry: ScopedEntry<'_, A> = collection.entry(index_a).unwrap();
        assert_eq!(entry.0, 42);
        let entry: ScopedEntry<'_, B> = collection.entry(index_b).unwrap();
        assert_eq!(entry.0, 31);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_guard_collection_entry_invalid_index_type_checked_in_debug() {
        let mut collection = TypeGuardCollection::<u32>::default();
        let index_a = collection.push(A(42).into_guard()).unwrap();
        let index_b = collection.push(B(31).into_guard()).unwrap();

        let entry: ScopedEntryResult<B> = collection.entry(index_a);
        assert!(entry.is_err());
        let entry: ScopedEntryResult<A> = collection.entry(index_b);
        assert!(entry.is_err());
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn test_guard_collection_entry_invalid_index_type_check_skip_in_release() {
        let mut collection = TypeGuardCollection::<u32>::default();
        let index_a = collection.push(A(42).into_guard()).unwrap();
        let index_b = collection.push(B(31).into_guard()).unwrap();

        let entry_b_invalid: ScopedEntry<'_, B> = collection.entry(index_a).unwrap();
        assert_eq!(entry_b_invalid.0, 42);
        let entry_a_invalid: ScopedEntry<'_, A> = collection.entry(index_b).unwrap();
        assert_eq!(entry_a_invalid.0, 31);
    }

    #[test]
    fn test_guard_collection_mut_entry_update_on_drop() {
        let mut collection = TypeGuardCollection::<u32>::default();
        let index = collection.push(A(42).into_guard()).unwrap();

        {
            let mut entry: ScopedEntryMut<'_, A> = collection.entry_mut(index).unwrap();
            assert_eq!(entry.0, 42);
            entry.0 = 31;
        }

        {
            let entry: ScopedEntryMut<'_, A> = collection.entry_mut(index).unwrap();
            assert_eq!(entry.0, 31);
        }
    }

    #[test]
    fn test_gen_index_as_hash_map_key() {
        let mut collection = GenCollection::<u32>::default();
        let index1 = collection.push(42).unwrap();
        let index2 = collection.push(32).unwrap();
        let mut map = std::collections::HashMap::new();
        map.insert(index1, 42);
        map.insert(index2, 32);
        assert_eq!(map.get(&index1), Some(&42));
        assert_eq!(map.get(&index2), Some(&32));
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GenCollectionError {
    InvalidGeneration { expected: usize, actual: usize },
    InvalidIndex { index: usize, len: usize },
    InvalidItemIndex { index: usize, len: usize },
    CellEmpty,
    CellOccupied,
    CellBorrowed,
    ItemBorrowed,
    ItemOccupied,
}

impl Display for GenCollectionError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            GenCollectionError::InvalidGeneration { expected, actual } => {
                write!(
                    f,
                    "Invalid generation: expected {}, actual {}",
                    expected, actual
                )
            }
            GenCollectionError::InvalidIndex { index, len } => {
                write!(f, "Invalid index: index {}, len {}", index, len)
            }
            GenCollectionError::InvalidItemIndex { index, len } => {
                write!(f, "Invalid item index: index {}, len {}", index, len)
            }
            GenCollectionError::CellEmpty => {
                write!(f, "Cell is empty")
            }
            GenCollectionError::CellOccupied => {
                write!(f, "Cell is occupied")
            }
            GenCollectionError::CellBorrowed => {
                write!(f, "Cell is borrowed")
            }
            GenCollectionError::ItemBorrowed => {
                write!(f, "Item is borrowed")
            }
            GenCollectionError::ItemOccupied => {
                write!(f, "Item is occupied")
            }
        }
    }
}

impl Error for GenCollectionError {}

pub type GenCollectionResult<T> = Result<T, GenCollectionError>;

mod cell {
    use super::{GenCollectionError, GenCollectionResult};

    #[derive(Debug, Clone, Copy)]
    struct Occupied {
        item_index: usize,
    }

    #[derive(Debug, Clone, Copy)]
    struct Empty {
        next_free: Option<usize>,
    }

    #[derive(Debug)]
    pub(super) struct LockedCell {
        cell: GenCell,
        generation: usize,
    }

    impl LockedCell {
        #[inline]
        pub(super) fn new(item_index: usize) -> Self {
            Self {
                cell: GenCell::Occupied(Occupied { item_index }),
                generation: 0,
            }
        }

        #[inline]
        pub(super) fn generation(&self) -> GenCollectionResult<usize> {
            match self.cell {
                GenCell::Occupied(_) => Ok(self.generation),
                GenCell::Borrowed(_) => Ok(self.generation),
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
            }
        }

        #[inline]
        pub(super) fn unlock(&self, generation: usize) -> GenCollectionResult<&GenCell> {
            let cell_generation = self.generation()?;
            if cell_generation == generation {
                Ok(&self.cell)
            } else {
                Err(GenCollectionError::InvalidGeneration {
                    expected: generation,
                    actual: cell_generation,
                })
            }
        }

        #[inline]
        pub(super) fn unlock_mut(
            &mut self,
            generation: usize,
        ) -> GenCollectionResult<&mut GenCell> {
            let cell_generation = self.generation()?;
            if cell_generation == generation {
                Ok(&mut self.cell)
            } else {
                Err(GenCollectionError::InvalidGeneration {
                    expected: generation,
                    actual: cell_generation,
                })
            }
        }

        #[inline]
        pub(super) fn insert(
            &mut self,
            item_index: usize,
        ) -> GenCollectionResult<(usize, Option<usize>)> {
            match self.cell {
                GenCell::Empty(Empty { next_free }) => {
                    self.generation += 1;
                    self.cell = GenCell::Occupied(Occupied { item_index });
                    Ok((self.generation, next_free))
                }
                GenCell::Occupied(..) => Err(GenCollectionError::CellOccupied),
                GenCell::Borrowed(..) => Err(GenCollectionError::CellBorrowed),
            }
        }

        #[inline]
        pub(super) fn update_item_index(&mut self, item_index: usize) -> GenCollectionResult<()> {
            match &mut self.cell {
                GenCell::Occupied(cell) => {
                    cell.item_index = item_index;
                    Ok(())
                }
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
                GenCell::Borrowed(..) => Err(GenCollectionError::CellBorrowed),
            }
        }
    }

    #[allow(private_interfaces)]
    #[derive(Debug, Clone, Copy)]
    pub(super) enum GenCell {
        Occupied(Occupied),
        Borrowed(Occupied),
        Empty(Empty),
    }

    impl GenCell {
        #[inline]
        pub(super) fn pop(&mut self, next_free: Option<usize>) -> GenCollectionResult<usize> {
            match *self {
                GenCell::Occupied(cell) => {
                    *self = GenCell::Empty(Empty { next_free });
                    Ok(cell.item_index)
                }
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
                GenCell::Borrowed(..) => Err(GenCollectionError::CellBorrowed),
            }
        }

        #[inline]
        pub(super) fn borrow(&mut self) -> GenCollectionResult<usize> {
            match *self {
                GenCell::Occupied(cell) => {
                    *self = GenCell::Borrowed(cell);
                    Ok(cell.item_index)
                }
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
                GenCell::Borrowed(..) => Err(GenCollectionError::CellBorrowed),
            }
        }

        #[inline]
        pub(super) fn put_back(&mut self) -> GenCollectionResult<usize> {
            match *self {
                GenCell::Borrowed(cell) => {
                    *self = GenCell::Occupied(cell);
                    Ok(cell.item_index)
                }
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
                GenCell::Occupied(..) => Err(GenCollectionError::CellOccupied),
            }
        }

        #[inline]
        pub(super) fn item_index(&self) -> GenCollectionResult<usize> {
            match self {
                GenCell::Occupied(cell) => Ok(cell.item_index),
                GenCell::Borrowed(cell) => Ok(cell.item_index),
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
            }
        }
    }
}

use cell::{GenCell, LockedCell};
use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use crate::{
    Cons, Contains, Destroy, DestroyResult, DropGuard, FromGuard, Guard, Marked, Marker, Nil,
    TypeGuard, TypeGuardConversionError, TypeList, Valid, ValidMut, ValidRef,
};

pub struct GenIndex<T> {
    index: usize,
    generation: usize,
    _phantom: PhantomData<T>,
}

impl<T> Clone for GenIndex<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for GenIndex<T> {}

impl<T> PartialEq for GenIndex<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for GenIndex<T> {}

impl<T> Hash for GenIndex<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> Debug for GenIndex<T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(
            f,
            "GenIndex<{}> {{ index: {}, generation: {} }}",
            type_name::<T>(),
            self.index,
            self.generation
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenIndexRaw {
    index: usize,
    generation: usize,
}

impl<T: 'static> FromGuard for GenIndex<T> {
    type Inner = GenIndexRaw;

    #[inline]
    fn into_inner(self) -> GenIndexRaw {
        GenIndexRaw {
            index: self.index,
            generation: self.generation,
        }
    }
}

impl<T> From<Valid<GenIndex<T>>> for GenIndex<T> {
    #[inline]
    fn from(value: Valid<GenIndex<T>>) -> Self {
        let GenIndexRaw { index, generation } = value.into_inner();
        GenIndex::wrap(generation, index)
    }
}

impl<T> GenIndex<T> {
    #[inline]
    pub fn wrap(generation: usize, index: usize) -> Self {
        Self {
            index,
            generation,
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn mark<C, M: Marker>(self) -> Marked<Self, M>
    where
        C: Contains<GenCollection<T>, M>,
    {
        Marked::new(self)
    }
}

#[derive(Debug)]
pub struct GenCollection<T> {
    items: Vec<T>,
    indices: Vec<LockedCell>,
    mapping: Vec<usize>,
    next_free: Option<usize>,
}

impl<T> Default for GenCollection<T> {
    #[inline]
    fn default() -> Self {
        Self {
            items: Vec::new(),
            indices: Vec::new(),
            mapping: Vec::new(),
            next_free: None,
        }
    }
}

impl<T> GenCollection<T> {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[inline]
    pub fn push(&mut self, item: T) -> GenCollectionResult<GenIndex<T>> {
        let item_index = self.items.len();
        self.items.push(item);

        let (generation, cell_index) = if let Some(index) = self.next_free {
            let cell = &mut self.indices[index];
            let (generation, next_free) = cell.insert(item_index)?;
            self.next_free = next_free;
            (generation, index)
        } else {
            let index = self.indices.len();
            self.indices.push(LockedCell::new(item_index));
            (0, index)
        };

        self.mapping.push(cell_index);
        Ok(GenIndex::wrap(generation, cell_index))
    }

    #[inline]
    pub fn pop(&mut self, index: GenIndex<T>) -> GenCollectionResult<T> {
        let GenIndex {
            index: next_free, ..
        } = index;
        let next_free = self.next_free.replace(next_free);
        let item_index = self.get_cell_mut_unlocked(index)?.pop(next_free)?;
        self.swap_remove(item_index)
    }

    #[inline]
    pub fn get(&self, index: GenIndex<T>) -> GenCollectionResult<&T> {
        let item_index = self.get_cell_item_index(index)?;
        Ok(&self.items[item_index])
    }

    #[inline]
    pub fn get_mut(&mut self, index: GenIndex<T>) -> GenCollectionResult<&mut T> {
        let item_index = self.get_cell_item_index(index)?;
        Ok(&mut self.items[item_index])
    }

    #[inline]
    pub fn drain(&mut self) -> Vec<T> {
        let items = self.items.drain(..).collect();
        self.indices.clear();
        self.mapping.clear();
        self.next_free = None;
        items
    }

    #[inline]
    pub fn filter_drain<P>(&mut self, predicate: P) -> Vec<T>
    where
        P: Fn(&T) -> bool,
    {
        let mut removed = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            if predicate(&self.items[i]) {
                removed.push(self.swap_remove(i).unwrap());
            } else {
                i += 1;
            }
        }
        removed
    }

    #[inline]
    fn get_cell_unlocked(&self, index: GenIndex<T>) -> GenCollectionResult<&GenCell> {
        let len = self.indices.len();
        let GenIndex {
            index, generation, ..
        } = index;
        self.indices
            .get(index)
            .ok_or(GenCollectionError::InvalidIndex { index, len })
            .and_then(|cell| cell.unlock(generation))
    }

    #[inline]
    fn get_cell_mut_unlocked(&mut self, index: GenIndex<T>) -> GenCollectionResult<&mut GenCell> {
        let len = self.indices.len();
        let GenIndex {
            index, generation, ..
        } = index;
        self.indices
            .get_mut(index)
            .ok_or(GenCollectionError::InvalidIndex { index, len })
            .and_then(|cell| cell.unlock_mut(generation))
    }

    #[inline]
    fn validate_item_index(&self, index: usize) -> GenCollectionResult<usize> {
        let len = self.items.len();
        if index < self.items.len() {
            Ok(index)
        } else {
            Err(GenCollectionError::InvalidItemIndex { index, len })
        }
    }

    #[inline]
    fn get_cell_item_index(&self, index: GenIndex<T>) -> GenCollectionResult<usize> {
        self.get_cell_unlocked(index)?
            .item_index()
            .and_then(|item_index| self.validate_item_index(item_index))
    }

    #[inline]
    fn pop_last(&mut self) -> T {
        self.mapping
            .pop()
            .expect("Pop called on empty GenCollection mapping member!");
        self.items
            .pop()
            .expect("Pop called on empty GenCollection items member!")
    }

    #[inline]
    fn swap_remove(&mut self, item_index: usize) -> GenCollectionResult<T> {
        let item_index = self.validate_item_index(item_index)?;
        let last_index = self.items.len() - 1;
        if item_index < last_index {
            let cell_index = self.mapping[last_index];
            self.indices[cell_index].update_item_index(item_index)?;
            self.mapping.swap(item_index, last_index);
            self.items.swap(item_index, last_index);
        }
        Ok(self.pop_last())
    }
}

pub struct Borrowed<T: BorrowItem> {
    item: T::Item,
    index: GenIndex<T>,
}

impl<T: BorrowItem> Deref for Borrowed<T> {
    type Target = T::Item;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T: BorrowItem> DerefMut for Borrowed<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

pub trait BorrowItem: Sized {
    type Item;

    fn borrow(&mut self) -> GenCollectionResult<Self::Item>;
    fn put_back(&mut self, item: Self::Item) -> GenCollectionResult<()>;
}

impl<T> BorrowItem for Option<T> {
    type Item = T;

    #[inline]
    fn borrow(&mut self) -> GenCollectionResult<Self::Item> {
        match self.take() {
            Some(item) => Ok(item),
            None => Err(GenCollectionError::ItemBorrowed),
        }
    }

    #[inline]
    fn put_back(&mut self, item: Self::Item) -> GenCollectionResult<()> {
        if self.is_none() {
            *self = Some(item);
            Ok(())
        } else {
            Err(GenCollectionError::ItemOccupied)
        }
    }
}

pub type OptionCollection<T> = GenCollection<Option<T>>;

#[derive(Debug, Clone, Copy)]
pub struct CopyEntry<T: Clone + Copy> {
    item: T,
}

impl<T: Clone + Copy> CopyEntry<T> {
    #[inline]
    pub fn new(item: T) -> Self {
        Self { item }
    }
}

impl<T: Clone + Copy> Deref for CopyEntry<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T: Clone + Copy> DerefMut for CopyEntry<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

impl<T: Clone + Copy> BorrowItem for CopyEntry<T> {
    type Item = T;

    #[inline]
    fn borrow(&mut self) -> GenCollectionResult<Self::Item> {
        Ok(self.item)
    }

    #[inline]
    fn put_back(&mut self, item: Self::Item) -> GenCollectionResult<()> {
        self.item = item;
        Ok(())
    }
}

impl<T: Clone + Copy> From<T> for CopyEntry<T> {
    #[inline]
    fn from(item: T) -> Self {
        Self { item }
    }
}

pub type CopyCollection<T> = GenCollection<CopyEntry<T>>;

impl<T: BorrowItem> GenCollection<T> {
    #[inline]
    fn borrow(&mut self, index: GenIndex<T>) -> GenCollectionResult<Borrowed<T>> {
        let item_index = self.get_cell_mut_unlocked(index.clone())?.borrow()?;
        let item = self.items[item_index].borrow()?;
        Ok(Borrowed { item, index })
    }

    #[inline]
    fn put_back(&mut self, borrow: Borrowed<T>) -> GenCollectionResult<()> {
        let Borrowed { item, index } = borrow;
        let item_index = self.get_cell_mut_unlocked(index)?.put_back()?;
        self.items[item_index].put_back(item)?;
        Ok(())
    }
}

impl<T> Index<GenIndex<T>> for GenCollection<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: GenIndex<T>) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<GenIndex<T>> for GenCollection<T> {
    #[inline]
    fn index_mut(&mut self, index: GenIndex<T>) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

impl<'a, T> IntoIterator for &'a GenCollection<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut GenCollection<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter_mut()
    }
}

impl<T> IntoIterator for GenCollection<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScopedEntry<'a, T: FromGuard> {
    resource: T,
    _raw: &'a T::Inner,
}

impl<'a, T: FromGuard> Deref for ScopedEntry<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

pub struct ScopedEntryMut<'a, T: FromGuard> {
    resource: Option<T>,
    raw: &'a mut T::Inner,
}

impl<'a, T: FromGuard> Drop for ScopedEntryMut<'a, T> {
    #[inline]
    fn drop(&mut self) {
        *self.raw = self.resource.take().unwrap().into_inner();
    }
}

impl<'a, T: FromGuard> Deref for ScopedEntryMut<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().unwrap()
    }
}

impl<'a, T: FromGuard> DerefMut for ScopedEntryMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resource.as_mut().unwrap()
    }
}

pub struct ScopedInnerRef<'a, T: FromGuard> {
    inner: &'a T::Inner,
    _phantom: PhantomData<T>,
}

impl<'a, T: FromGuard> From<ValidRef<'a, T>> for ScopedInnerRef<'a, T> {
    #[inline]
    fn from(value: ValidRef<'a, T>) -> Self {
        Self {
            inner: value.inner_ref(),
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: FromGuard> Deref for ScopedInnerRef<'a, T> {
    type Target = T::Inner;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ScopedInnerMut<'a, T: FromGuard> {
    inner: &'a mut T::Inner,
    _phantom: PhantomData<T>,
}

impl<'a, T: FromGuard> From<ValidMut<'a, T>> for ScopedInnerMut<'a, T> {
    #[inline]
    fn from(value: ValidMut<'a, T>) -> Self {
        Self {
            inner: value.inner_mut(),
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: FromGuard> Deref for ScopedInnerMut<'a, T> {
    type Target = T::Inner;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T: FromGuard> DerefMut for ScopedInnerMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub type GuardIndex<T> = GenIndex<Guard<T>>;
pub type TypeGuardCollection<T> = GenCollection<TypeGuard<T>>;

#[derive(Debug, Clone, Copy)]
pub enum GuardCollectionError {
    GenCollection(GenCollectionError),
    TypeGuardConversion(TypeGuardConversionError),
}

impl Display for GuardCollectionError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            GuardCollectionError::GenCollection(error) => {
                write!(f, "GenCollection error: {}", error)
            }
            GuardCollectionError::TypeGuardConversion(error) => {
                write!(f, "TypeGuard conversion error: {}", error)
            }
        }
    }
}

impl From<GenCollectionError> for GuardCollectionError {
    #[inline]
    fn from(error: GenCollectionError) -> Self {
        GuardCollectionError::GenCollection(error)
    }
}

impl From<TypeGuardConversionError> for GuardCollectionError {
    #[inline]
    fn from(error: TypeGuardConversionError) -> Self {
        GuardCollectionError::TypeGuardConversion(error)
    }
}

impl Error for GuardCollectionError {}

pub type ScopedEntryResult<'a, T> = Result<ScopedEntry<'a, T>, GuardCollectionError>;
pub type ScopedEntryMutResult<'a, T> = Result<ScopedEntryMut<'a, T>, GuardCollectionError>;

impl<I: Clone + Copy> TypeGuardCollection<I> {
    #[inline]
    pub fn entry<'a, T: FromGuard<Inner = I>>(
        &'a self,
        index: GuardIndex<T>,
    ) -> ScopedEntryResult<T> {
        let guard = self.get(index)?;
        Ok(ScopedEntry {
            resource: T::try_from_guard(*guard).map_err(|(_, err)| err)?,
            _raw: guard.inner(),
        })
    }

    #[inline]
    pub fn entry_mut<'a, T: FromGuard<Inner = I>>(
        &'a mut self,
        index: GuardIndex<T>,
    ) -> ScopedEntryMutResult<'a, T> {
        let guard = self.get_mut(index)?;
        Ok(ScopedEntryMut {
            resource: Some(T::try_from_guard(*guard).map_err(|(_, err)| err)?),
            raw: guard.inner_mut(),
        })
    }
}

pub type ScopedInnerResult<'a, T> = Result<ScopedInnerRef<'a, T>, GuardCollectionError>;
pub type ScopedInnerMutResult<'a, T> = Result<ScopedInnerMut<'a, T>, GuardCollectionError>;

impl<I> TypeGuardCollection<I> {
    #[inline]
    pub fn inner_ref<'a, T: FromGuard<Inner = I>>(
        &'a self,
        index: GuardIndex<T>,
    ) -> ScopedInnerResult<T> {
        let inner: ValidRef<T> = self.get(index)?.try_into()?;
        Ok(inner.into())
    }

    #[inline]
    pub fn inner_mut<'a, T: FromGuard<Inner = I>>(
        &'a mut self,
        index: GuardIndex<T>,
    ) -> ScopedInnerMutResult<'a, T> {
        let inner: ValidMut<T> = self.get_mut(index)?.try_into()?;
        Ok(inner.into())
    }
}

impl<I: Destroy> Destroy for GenCollection<I>
where
    for<'a> I::Context<'a>: Clone + Copy,
{
    type Context<'a> = I::Context<'a>;
    type DestroyError = I::DestroyError;

    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        self.into_iter().try_for_each(|item| item.destroy(context))
    }
}

pub type DropGuardCollection<T> = DropGuard<GenCollection<T>>;
pub type GuardCollection<T> = DropGuard<TypeGuardCollection<T>>;

pub trait IndexList<C: 'static> {
    type Owned;
    type Ref<'a>;

    fn get_ref(self, collection: &C) -> GenCollectionResult<Self::Ref<'_>>;
    fn get_owned(self, collection: &mut C) -> GenCollectionResult<Self::Owned>;
}

impl<C: 'static> IndexList<C> for Nil {
    type Owned = Nil;
    type Ref<'a> = Nil;

    #[inline]
    fn get_ref(self, _: &C) -> GenCollectionResult<Self::Ref<'_>> {
        Ok(Nil::new())
    }

    #[inline]
    fn get_owned(self, _: &mut C) -> GenCollectionResult<Self::Owned> {
        Ok(Nil::new())
    }
}

impl<C: 'static, H: 'static, M: Marker, T: IndexList<C>> IndexList<C>
    for Cons<Marked<GenIndex<H>, M>, T>
where
    C: Contains<GenCollection<H>, M>,
{
    type Owned = Cons<H, T::Owned>;
    type Ref<'a> = Cons<&'a H, T::Ref<'a>>;

    #[inline]
    fn get_ref(self, collection: &C) -> GenCollectionResult<Self::Ref<'_>> {
        let Cons {
            head: Marked { value: index, .. },
            tail,
        } = self;
        let head = <C as Contains<_, _>>::get(collection).get(index)?;
        let tail = tail.get_ref(collection)?;
        Ok(Cons::new(head, tail))
    }

    #[inline]
    fn get_owned(self, collection: &mut C) -> GenCollectionResult<Self::Owned> {
        let Cons {
            head: Marked { value: index, .. },
            tail,
        } = self;
        let head = <C as Contains<_, _>>::get_mut(collection).pop(index)?;
        let tail = tail.get_owned(collection)?;
        Ok(Cons::new(head, tail))
    }
}

pub trait BorrowList<C: 'static> {
    fn put_back(self, collection: &mut C) -> GenCollectionResult<()>;
}

impl<C: 'static> BorrowList<C> for Nil {
    #[inline]
    fn put_back(self, _: &mut C) -> GenCollectionResult<()> {
        Ok(())
    }
}

impl<C: 'static, H: BorrowItem, M: Marker, T: BorrowList<C>> BorrowList<C>
    for Cons<Marked<Borrowed<H>, M>, T>
where
    C: Contains<GenCollection<H>, M>,
{
    #[inline]
    fn put_back(self, collection: &mut C) -> GenCollectionResult<()> {
        let Cons {
            head: Marked { value: borrow, .. },
            tail,
        } = self;
        collection.get_mut().put_back(borrow)?;
        tail.put_back(collection)
    }
}

pub trait IndexListBorrow<C: 'static> {
    type Borrowed: BorrowList<C>;

    fn get_borrowed(self, collection: &mut C) -> GenCollectionResult<Self::Borrowed>;
}

impl<C: 'static> IndexListBorrow<C> for Nil {
    type Borrowed = Nil;

    #[inline]
    fn get_borrowed(self, _: &mut C) -> GenCollectionResult<Self::Borrowed> {
        Ok(Nil::new())
    }
}

impl<C: 'static, H: BorrowItem, M: Marker, T: IndexListBorrow<C>> IndexListBorrow<C>
    for Cons<Marked<GenIndex<H>, M>, T>
where
    C: Contains<GenCollection<H>, M>,
{
    type Borrowed = Cons<Marked<Borrowed<H>, M>, T::Borrowed>;
    #[inline]
    fn get_borrowed(self, collection: &mut C) -> GenCollectionResult<Self::Borrowed> {
        let Cons {
            head: Marked { value: index, .. },
            tail,
        } = self;
        let head = <C as Contains<_, _>>::get_mut(collection).borrow(index)?;
        let tail = tail.get_borrowed(collection)?;
        Ok(Cons::new(Marked::new(head), tail))
    }
}

#[macro_export]
macro_rules! mark {
    [$collection:ty] => { Nil::new() };
    [$collection:ty, $index:expr $(, $indices:expr)*] => {
        Cons::new($index.mark::<$collection, _>(), mark![$collection $(, $indices)*])
    };
}

#[derive(Debug)]
pub struct GenCollectionList<T: TypeList + 'static> {
    collection: T,
}

impl<T: TypeList> Deref for GenCollectionList<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.collection
    }
}

impl<T: TypeList> DerefMut for GenCollectionList<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.collection
    }
}

impl<T: TypeList + Default> Default for GenCollectionList<T> {
    #[inline]
    fn default() -> Self {
        Self {
            collection: T::default(),
        }
    }
}

impl<T: TypeList + Default> GenCollectionList<T> {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug)]
pub struct BorrowedContext<C: 'static, B: BorrowList<C>> {
    borrow: Option<B>,
    _phantom: PhantomData<C>,
}

impl<C: 'static, B: BorrowList<C>> BorrowedContext<C, B> {
    #[inline]
    pub fn operate_ref<R, E, F: FnOnce(&B) -> Result<R, E>>(&self, operation: F) -> Result<R, E> {
        operation(self.borrow.as_ref().unwrap())
    }

    #[inline]
    pub fn operate_mut<R, E, F: FnOnce(&mut B) -> Result<R, E>>(
        &mut self,
        operation: F,
    ) -> Result<R, E> {
        operation(self.borrow.as_mut().unwrap())
    }
}

impl<C, B: BorrowList<C>> Destroy for BorrowedContext<C, B> {
    type Context<'a> = &'a mut C;
    type DestroyError = GenCollectionError;

    #[inline]
    fn destroy<'a>(&mut self, context: Self::Context<'a>) -> DestroyResult<Self> {
        if let Some(borrow) = self.borrow.take() {
            borrow.put_back(context)?;
            self.borrow = None;
        }
        Ok(())
    }
}

impl<T: TypeList> GenCollectionList<T> {
    #[inline]
    pub fn len<I, M: Marker>(&self) -> usize
    where
        T: Contains<GenCollection<I>, M>,
    {
        self.collection.get().len()
    }

    #[inline]
    pub fn push<I, M: Marker>(&mut self, item: I) -> GenCollectionResult<GenIndex<I>>
    where
        T: Contains<GenCollection<I>, M>,
    {
        self.collection.get_mut().push(item)
    }

    #[inline]
    pub fn pop<I, M: Marker>(&mut self, index: GenIndex<I>) -> GenCollectionResult<I>
    where
        T: Contains<GenCollection<I>, M>,
    {
        self.collection.get_mut().pop(index)
    }

    #[inline]
    pub fn get_ref<'a, I: IndexList<T>>(&'a self, index: I) -> GenCollectionResult<I::Ref<'a>> {
        index.get_ref(&self.collection)
    }

    #[inline]
    pub fn get_owned<I: IndexList<T>>(&mut self, index: I) -> GenCollectionResult<I::Owned> {
        index.get_owned(&mut self.collection)
    }

    #[inline]
    pub fn get_borrow<I: IndexListBorrow<T>>(
        &mut self,
        index: I,
    ) -> GenCollectionResult<DropGuard<BorrowedContext<T, I::Borrowed>>> {
        let borrow = index.get_borrowed(&mut self.collection)?;
        let context = BorrowedContext {
            borrow: Some(borrow),
            _phantom: PhantomData,
        };
        Ok(DropGuard::new(context))
    }
}

#[cfg(test)]
mod test_list_index {
    use std::convert::Infallible;

    use super::*;
    use crate::{list_type, list_value, unpack_list, Cons, GenIndex, IndexList, Nil};

    type TestCollection = list_type![
        GenCollection<u8>,
        GenCollection<u16>,
        GenCollection<u32>,
        Nil
    ];
    type TestCopyCollection = list_type![
        CopyCollection<u8>,
        CopyCollection<u16>,
        CopyCollection<u32>,
        Nil
    ];
    type TestOptionCollection =
        list_type![OptionCollection<Vec<u8>>, OptionCollection<Vec<u16>>, Nil];
    type TestCollectionList = GenCollectionList<TestCopyCollection>;

    #[test]
    fn test_collection_list_index_get_owned() {
        let mut collection = TestCollection::default();

        let collection_u8: &mut GenCollection<u8> = collection.get_mut();
        let index_u8: GenIndex<u8> = collection_u8.push(8).unwrap();

        let collection_u16: &mut GenCollection<u16> = collection.get_mut();
        let index_u16: GenIndex<u16> = collection_u16.push(16).unwrap();

        let collection_u32: &mut GenCollection<u32> = collection.get_mut();
        let index_u32: GenIndex<u32> = collection_u32.push(32).unwrap();

        let index_list = mark![TestCollection, index_u8, index_u16, index_u32];
        let unpack_list![b_u8, b_u16, b_u32, _rest] =
            index_list.get_owned(&mut collection).unwrap();

        assert_eq!(b_u8, 8);
        assert_eq!(b_u16, 16);
        assert_eq!(b_u32, 32);

        let collection_u8: &GenCollection<u8> = collection.get();
        let collection_u16: &GenCollection<u16> = collection.get();
        let collection_u32: &GenCollection<u32> = collection.get();

        assert_eq!(collection_u8.len(), 0);
        assert_eq!(collection_u16.len(), 0);
        assert_eq!(collection_u32.len(), 0);
    }

    #[test]
    fn test_collection_list_index_get_ref() {
        let mut collection = TestCollection::default();

        let collection_u8: &mut GenCollection<u8> = collection.get_mut();
        let index_u8: GenIndex<u8> = collection_u8.push(8).unwrap();

        let collection_u16: &mut GenCollection<u16> = collection.get_mut();
        let index_u16: GenIndex<u16> = collection_u16.push(16).unwrap();

        let collection_u32: &mut GenCollection<u32> = collection.get_mut();
        let index_u32: GenIndex<u32> = collection_u32.push(32).unwrap();

        let index_list = mark![TestCollection, index_u8, index_u16, index_u32];
        let unpack_list![b_u8, b_u16, b_u32, _rest] = index_list.get_ref(&collection).unwrap();

        assert_eq!(*b_u8, 8);
        assert_eq!(*b_u16, 16);
        assert_eq!(*b_u32, 32);

        let collection_u8: &GenCollection<u8> = collection.get();
        let collection_u16: &GenCollection<u16> = collection.get();
        let collection_u32: &GenCollection<u32> = collection.get();

        assert_eq!(collection_u8.len(), 1);
        assert_eq!(collection_u16.len(), 1);
        assert_eq!(collection_u32.len(), 1);
    }

    #[test]
    fn test_collection_list_index_get_borrow_copy_type() {
        let mut collection = TestCopyCollection::default();

        let collection_u8: &mut CopyCollection<u8> = collection.get_mut();
        let index_u8: GenIndex<CopyEntry<u8>> = collection_u8.push(8.into()).unwrap();

        let collection_u16: &mut CopyCollection<u16> = collection.get_mut();
        let index_u16: GenIndex<CopyEntry<u16>> = collection_u16.push(16.into()).unwrap();

        let collection_u32: &mut CopyCollection<u32> = collection.get_mut();
        let index_u32: GenIndex<CopyEntry<u32>> = collection_u32.push(32.into()).unwrap();

        let index_list = mark![TestCopyCollection, index_u8, index_u16, index_u32];
        let unpack_list![b_u8, b_u16, b_u32, _rest] =
            index_list.get_borrowed(&mut collection).unwrap();

        assert_eq!(**b_u8, 8);
        assert_eq!(**b_u16, 16);
        assert_eq!(**b_u32, 32);

        let collection_u8: &CopyCollection<u8> = collection.get();
        let collection_u16: &CopyCollection<u16> = collection.get();
        let collection_u32: &CopyCollection<u32> = collection.get();

        assert_eq!(collection_u8.len(), 1);
        assert_eq!(collection_u16.len(), 1);
        assert_eq!(collection_u32.len(), 1);

        let collection_u8: &mut CopyCollection<u8> = collection.get_mut();
        matches!(
            collection_u8.pop(index_u8),
            Err(GenCollectionError::ItemBorrowed)
        );

        let collection_u16: &mut CopyCollection<u16> = collection.get_mut();
        matches!(
            collection_u16.pop(index_u16),
            Err(GenCollectionError::ItemBorrowed)
        );

        let collection_u32: &mut CopyCollection<u32> = collection.get_mut();
        matches!(
            collection_u32.pop(index_u32),
            Err(GenCollectionError::ItemBorrowed)
        );

        let borrowed = list_value![b_u8, b_u16, b_u32, Nil::new()];
        matches!(borrowed.put_back(&mut collection), Ok(..));

        let collection_u8: &mut CopyCollection<u8> = collection.get_mut();
        matches!(collection_u8.pop(index_u8), Ok(CopyEntry { item: 8 }));

        let collection_u16: &mut CopyCollection<u16> = collection.get_mut();
        matches!(collection_u16.pop(index_u16), Ok(CopyEntry { item: 16 }));

        let collection_u32: &mut CopyCollection<u32> = collection.get_mut();
        matches!(collection_u32.pop(index_u32), Ok(CopyEntry { item: 32 }));
    }

    #[test]
    fn test_collection_list_index_get_borrow_non_copy_type() {
        let mut collection = TestOptionCollection::default();

        let collection_vec_u8: &mut OptionCollection<Vec<u8>> = collection.get_mut();
        let index_vec_u8: GenIndex<Option<Vec<u8>>> =
            collection_vec_u8.push(Some(vec![8])).unwrap();

        let collection_vec_u16: &mut OptionCollection<Vec<u16>> = collection.get_mut();
        let index_vec_u16: GenIndex<Option<Vec<u16>>> =
            collection_vec_u16.push(Some(vec![16])).unwrap();

        let index_list = mark![TestOptionCollection, index_vec_u8, index_vec_u16];
        let unpack_list![b_vec_u8, b_vec_u16, _rest] =
            index_list.get_borrowed(&mut collection).unwrap();

        assert_eq!(**b_vec_u8, vec![8]);
        assert_eq!(**b_vec_u16, vec![16]);

        let collection_vec_u8: &OptionCollection<Vec<u8>> = collection.get();
        let collection_vec_u16: &OptionCollection<Vec<u16>> = collection.get();

        assert_eq!(collection_vec_u8.len(), 1);
        assert_eq!(collection_vec_u16.len(), 1);

        let collection_vec_u8: &mut OptionCollection<Vec<u8>> = collection.get_mut();
        matches!(
            collection_vec_u8.pop(index_vec_u8),
            Err(GenCollectionError::ItemBorrowed)
        );

        let collection_vec_u16: &mut OptionCollection<Vec<u16>> = collection.get_mut();
        matches!(
            collection_vec_u16.pop(index_vec_u16),
            Err(GenCollectionError::ItemBorrowed)
        );

        let borrowed = list_value![b_vec_u8, b_vec_u16, Nil::new()];
        matches!(borrowed.put_back(&mut collection), Ok(..));

        let collection_vec_u8: &mut OptionCollection<Vec<u8>> = collection.get_mut();
        matches!(collection_vec_u8.pop(index_vec_u8), Ok(..));

        let collection_vec_u16: &mut OptionCollection<Vec<u16>> = collection.get_mut();
        matches!(collection_vec_u16.pop(index_vec_u16), Ok(..));
    }

    #[test]
    fn test_gen_collection_list() {
        let mut collection = TestCollectionList::new();
        let index_u8: GenIndex<CopyEntry<u8>> = collection.push(8u8.into()).unwrap();
        let index_u16: GenIndex<CopyEntry<u16>> = collection.push(16u16.into()).unwrap();
        let index_u32: GenIndex<CopyEntry<u32>> = collection.push(32u32.into()).unwrap();

        let index_list = mark![TestCopyCollection, index_u8, index_u16, index_u32];
        {
            let mut context = collection.get_borrow(index_list).unwrap();
            let _ = context.operate_ref::<_, Infallible, _>(|borrow| {
                let unpack_list![b_u8, b_u16, b_u32] = borrow;
                assert_eq!(b_u8.item, 8);
                assert_eq!(b_u16.item, 16);
                assert_eq!(b_u32.item, 32);
                Ok(())
            });
            let _ = context.operate_mut::<_, bool, _>(|borrow| {
                let unpack_list![b_u8, b_u16, b_u32] = borrow;
                b_u8.item = 7;
                b_u16.item = 15;
                b_u32.item = 31;
                Ok(())
            });
            assert!(context.destroy(&mut collection.deref_mut()).is_ok());
        }
        {
            let mut context = collection.get_borrow(index_list).unwrap();
            let _ = context.operate_ref::<_, bool, _>(|borrow| {
                let unpack_list![b_u8, b_u16, b_u32] = borrow;
                assert_eq!(b_u8.item, 7);
                assert_eq!(b_u16.item, 15);
                assert_eq!(b_u32.item, 31);
                Ok(())
            });
            assert!(context.destroy(&mut collection.deref_mut()).is_ok());
        }
    }
}
