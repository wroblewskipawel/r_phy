use std::any::type_name;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

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
    fn test_pop_while_last_borrowed() {
        let mut collection = GenCollection::<u8>::default();
        let first_index = collection.push(42u8.into()).unwrap();
        let second_index = collection.push(37u8.into()).unwrap();

        let borrowed_item = collection.borrow(second_index).unwrap();
        assert_eq!(*borrowed_item, 37u8);

        let removed_item = collection.pop(first_index).unwrap();
        assert_eq!(removed_item, 42u8);

        collection.put_back(borrowed_item).unwrap();
    }

    #[test]
    fn test_invalid_index() {
        let collection: GenCollection<&str> = GenCollection::default();
        let invalid_index = GenIndex::wrap(0, 999); // Invalid index

        assert!(matches!(
            collection.get(invalid_index),
            Err(GenCollectionError::InvalidIndex { .. })
        ));
    }

    #[test]
    fn test_generation_mismatch() {
        let mut collection = GenCollection::default();
        let index = collection.push("Item 1").unwrap();

        // Manually create an index with an incorrect generation
        let invalid_index = GenIndex::wrap(index.generation + 1, index.index);

        // Attempting to get or pop with the invalid index should fail
        assert!(matches!(
            collection.get(invalid_index),
            Err(GenCollectionError::InvalidGeneration { .. })
        ));
        assert!(matches!(
            collection.pop(invalid_index),
            Err(GenCollectionError::InvalidGeneration { .. })
        ));
    }

    #[test]
    fn test_generation_item_borrowed() {
        let mut collection = GenCollection::default();
        let index = collection.push("Item 1").unwrap();

        // Manually create an index with an incorrect generation
        let _borrowed_item = collection.borrow(index);

        // Attempting to get or pop with the invalid index should fail
        assert!(matches!(
            collection.get(index),
            Err(GenCollectionError::CellBorrowed)
        ));
        assert!(matches!(
            collection.pop(index),
            Err(GenCollectionError::CellBorrowed)
        ));
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
    fn test_drain() {
        let mut collection = GenCollection::default();
        collection.push("Item 1").unwrap();
        collection.push("Item 2").unwrap();

        let items: Vec<_> = collection.drain();
        assert_eq!(items, vec!["Item 1", "Item 2"]);
        assert_eq!(collection.len(), 0);
    }

    #[test]
    fn test_filter_drain() {
        let mut collection = GenCollection::default();
        let index_1 = collection.push(11).unwrap();
        let index_2 = collection.push(42).unwrap();
        let index_3 = collection.push(31).unwrap();

        let items: Vec<_> = collection.filter_drain(|item| item % 2 == 0);
        assert_eq!(items, vec![42]);
        assert_eq!(collection.len(), 2);
        assert_eq!(collection.get(index_1).unwrap(), &11);
        assert_eq!(collection.get(index_3).unwrap(), &31);

        assert!(matches!(
            collection.get(index_2),
            Err(GenCollectionError::CellEmpty)
        ));

        collection.push(42).unwrap();
        assert!(matches!(
            collection.get(index_2),
            Err(GenCollectionError::InvalidGeneration {
                actual: 1,
                expected: 0
            })
        ));
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

        let entry: ScopedEntry<'_, A> = collection.entry(TypedIndex::<A>::new(index_a)).unwrap();
        assert_eq!(entry.0, 42);
        let entry: ScopedEntry<'_, B> = collection.entry(TypedIndex::<B>::new(index_b)).unwrap();
        assert_eq!(entry.0, 31);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_guard_collection_entry_invalid_index_type_checked_in_debug() {
        let mut collection = TypeGuardCollection::<u32>::default();
        let index_a = collection.push(A(42).into_guard()).unwrap();
        let index_b = collection.push(B(31).into_guard()).unwrap();

        let entry: ScopedEntryResult<B> = collection.entry(TypedIndex::<B>::new(index_a));
        assert!(entry.is_err());
        let entry: ScopedEntryResult<A> = collection.entry(TypedIndex::<A>::new(index_b));
        assert!(entry.is_err());
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn test_guard_collection_entry_invalid_index_type_check_skip_in_release() {
        let mut collection = TypeGuardCollection::<u32>::default();
        let index_a = collection.push(A(42).into_guard()).unwrap();
        let index_b = collection.push(B(31).into_guard()).unwrap();

        let entry_b_invalid: ScopedEntry<'_, B> =
            collection.entry(TypedIndex::<B>::new(index_a)).unwrap();
        assert_eq!(entry_b_invalid.0, 42);
        let entry_a_invalid: ScopedEntry<'_, A> =
            collection.entry(TypedIndex::<A>::new(index_b)).unwrap();
        assert_eq!(entry_a_invalid.0, 31);
    }

    #[test]
    fn test_guard_collection_mut_entry_update_on_drop() {
        let mut collection = TypeGuardCollection::<u32>::default();
        let index = collection.push(A(42).into_guard()).unwrap();

        {
            let mut entry: ScopedEntryMut<'_, A> =
                collection.entry_mut(TypedIndex::<A>::new(index)).unwrap();
            assert_eq!(entry.0, 42);
            entry.0 = 31;
        }

        {
            let entry: ScopedEntryMut<'_, A> =
                collection.entry_mut(TypedIndex::<A>::new(index)).unwrap();
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

    struct DropCounter {
        count: Rc<Cell<usize>>,
    }

    impl DropCounter {
        fn new() -> Self {
            Self {
                count: Rc::new(Cell::new(1)),
            }
        }

        fn count(&self) -> usize {
            self.count.get()
        }
    }

    impl Clone for DropCounter {
        fn clone(&self) -> Self {
            let count = self.count.clone();
            count.set(count.get() + 1);
            Self { count }
        }
    }

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.count.set(self.count.get() - 1);
        }
    }

    #[test]
    fn test_items_dropped_on_collection_drop() {
        let drop_counter = DropCounter::new();
        let mut collection = GenCollection::default();
        collection.push(drop_counter.clone()).unwrap();
        collection.push(drop_counter.clone()).unwrap();
        collection.push(drop_counter.clone()).unwrap();
        assert_eq!(drop_counter.count(), 4);
        drop(collection);
        assert_eq!(drop_counter.count(), 1);
    }

    #[test]
    fn test_items_dropped_on_collection_drop_skip_borrowed() {
        let drop_counter = DropCounter::new();
        let mut collection = GenCollection::default();

        let index_1 = collection.push(drop_counter.clone()).unwrap();
        let index_2 = collection.push(drop_counter.clone()).unwrap();
        collection.push(drop_counter.clone()).unwrap();
        assert_eq!(drop_counter.count(), 4);

        let borrowed_item = collection.borrow(index_2).unwrap();
        let popped_item = collection.pop(index_1).unwrap();

        drop(collection);
        assert_eq!(drop_counter.count(), 3);

        drop(popped_item);
        drop(borrowed_item);
        assert_eq!(drop_counter.count(), 1);
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
    // TODO: Temporary until separate TypeGuardCollection type is implemented
    TypeGuardConversion(TypeGuardConversionError),
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
            GenCollectionError::TypeGuardConversion(err) => write!(f, "{}", err),
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
        pub(super) fn unlock_unchecked(&mut self) -> &mut GenCell {
            &mut self.cell
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
                GenCell::Borrowed(cell) => {
                    cell.item_index = item_index;
                    Ok(())
                }
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
            }
        }

        #[inline]
        pub(super) fn is_occupied(&self) -> bool {
            match &self.cell {
                GenCell::Occupied(..) => true,
                _ => false,
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
                GenCell::Borrowed(..) => Err(GenCollectionError::CellBorrowed),
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
    Cons, Contains, Destroy, DestroyResult, DropGuard, FromGuard, Guard, IntoOuter, Marked, Marker,
    Nil, TypeGuard, TypeGuardConversionError, TypeList, Valid, ValidMut, ValidRef,
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
    items: Vec<MaybeUninit<T>>,
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

impl<T> Drop for GenCollection<T> {
    #[inline]
    fn drop(&mut self) {
        self.items
            .iter_mut()
            .zip(self.mapping.iter())
            .for_each(|(item, &cell_index)| {
                if self.indices[cell_index].is_occupied() {
                    unsafe {
                        item.assume_init_drop();
                    }
                }
            });
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
        self.items.push(MaybeUninit::new(item));

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
        let next_free = self.next_free;
        let item_index = self.get_cell_mut_unlocked(index)?.pop(next_free)?;
        self.next_free.replace(index.index);
        unsafe { Ok(self.swap_remove(item_index)) }
    }

    #[inline]
    pub fn get(&self, index: GenIndex<T>) -> GenCollectionResult<&T> {
        let item_index = self.get_cell_unlocked(index)?.item_index()?;
        Ok(unsafe { self.items[item_index].assume_init_ref() })
    }

    #[inline]
    pub fn get_mut(&mut self, index: GenIndex<T>) -> GenCollectionResult<&mut T> {
        let item_index = self.get_cell_unlocked(index)?.item_index()?;
        Ok(unsafe { self.items[item_index].assume_init_mut() })
    }

    #[inline]
    pub fn drain(&mut self) -> Vec<T> {
        self.filter_drain(|_| true)
    }

    #[inline]
    pub fn filter_drain<P>(&mut self, predicate: P) -> Vec<T>
    where
        P: Fn(&T) -> bool,
    {
        let mut removed = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            let cell_index = self.mapping[i];
            let cell = &mut self.indices[cell_index];
            if cell.is_occupied() && predicate(unsafe { self.items[i].assume_init_ref() }) {
                let next_free = self.next_free.replace(cell_index);
                let _ = cell.unlock_unchecked().pop(next_free);
                removed.push(unsafe { self.swap_remove(i) });
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

    // Safety: The caller must ensure that the item at the given index is occupied
    #[inline]
    unsafe fn swap_remove(&mut self, item_index: usize) -> T {
        let last_index = self.items.len() - 1;
        if item_index < last_index {
            let cell_index = self.mapping[last_index];
            self.indices[cell_index]
                .update_item_index(item_index)
                .unwrap();
            self.mapping.swap(item_index, last_index);
            self.items.swap(item_index, last_index);
        }
        self.mapping.pop().unwrap();
        unsafe { self.items.pop().unwrap().assume_init() }
    }
}

pub struct Borrowed<T> {
    item: T,
    index: GenIndex<T>,
}

impl<T> Deref for Borrowed<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T> DerefMut for Borrowed<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

impl<T> GenCollection<T> {
    #[inline]
    fn borrow(&mut self, index: GenIndex<T>) -> GenCollectionResult<Borrowed<T>> {
        let item_index = self.get_cell_mut_unlocked(index.clone())?.borrow()?;
        let item = unsafe { self.items[item_index].assume_init_read() };
        Ok(Borrowed { item, index })
    }

    #[inline]
    fn put_back(&mut self, borrow: Borrowed<T>) -> GenCollectionResult<()> {
        let Borrowed { item, index } = borrow;
        let item_index = self.get_cell_mut_unlocked(index)?.put_back()?;
        self.items[item_index] = MaybeUninit::new(item);
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

#[derive(Debug, Clone, Copy)]
pub struct GenCollectionRefIter<'a, T> {
    collection: &'a GenCollection<T>,
    next: usize,
}

impl<'a, T> Iterator for GenCollectionRefIter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let indices = &self.collection.indices;
        let mapping = &self.collection.mapping;
        let items = &self.collection.items;

        while self.next < items.len() {
            let item_index = self.next;
            self.next += 1;
            if indices[mapping[item_index]].is_occupied() {
                return Some(unsafe { items[item_index].assume_init_ref() });
            }
        }
        None
    }
}

impl<'a, T> IntoIterator for &'a GenCollection<T> {
    type Item = &'a T;
    type IntoIter = GenCollectionRefIter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        GenCollectionRefIter {
            collection: self,
            next: 0,
        }
    }
}

#[derive(Debug)]
pub struct GenCollectionMutIter<'a, T> {
    collection: &'a mut GenCollection<T>,
    next: usize,
}

impl<'a, T> Iterator for GenCollectionMutIter<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let indices = &self.collection.indices;
        let mapping = &self.collection.mapping;
        let items = &mut self.collection.items;

        while self.next < items.len() {
            let item_index = self.next;
            self.next += 1;
            if indices[mapping[item_index]].is_occupied() {
                return Some(unsafe { &mut *items[item_index].as_mut_ptr() });
            }
        }
        None
    }
}

impl<'a, T> IntoIterator for &'a mut GenCollection<T> {
    type Item = &'a mut T;
    type IntoIter = GenCollectionMutIter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        GenCollectionMutIter {
            collection: self,
            next: 0,
        }
    }
}

#[derive(Debug)]
pub struct GenCollectionIntoIter<T> {
    items: Vec<MaybeUninit<T>>,
    indices: Vec<LockedCell>,
    mapping: Vec<usize>,
    next: usize,
}

impl<T> Iterator for GenCollectionIntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.next < self.items.len() {
            let item_indx = self.next;
            self.next += 1;
            if self.indices[self.mapping[item_indx]].is_occupied() {
                return Some(unsafe { self.items[item_indx].assume_init_read() });
            }
        }
        None
    }
}

impl<T: 'static> IntoIterator for GenCollection<T> {
    type Item = T;
    type IntoIter = GenCollectionIntoIter<T>;

    #[inline]
    fn into_iter(mut self) -> Self::IntoIter {
        GenCollectionIntoIter {
            items: std::mem::replace(&mut self.items, vec![]),
            indices: std::mem::replace(&mut self.indices, vec![]),
            mapping: std::mem::replace(&mut self.mapping, vec![]),
            next: 0,
        }
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

#[derive(Debug)]
pub struct TypedIndex<T: FromGuard> {
    index: GuardIndex<T>,
}

impl<T: FromGuard> Clone for TypedIndex<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: FromGuard> Copy for TypedIndex<T> {}

impl<T: FromGuard> TypedIndex<T> {
    #[inline]
    pub fn new(index: GuardIndex<T>) -> Self {
        Self { index }
    }

    #[inline]
    pub fn mark<C, M: Marker>(self) -> Marked<Self, M>
    where
        C: Contains<TypeGuardCollection<T::Inner>, M>,
    {
        Marked::new(self)
    }
}

impl<I: Clone + Copy> TypeGuardCollection<I> {
    #[inline]
    pub fn entry<'a, T: FromGuard<Inner = I>>(
        &'a self,
        index: TypedIndex<T>,
    ) -> ScopedEntryResult<'a, T> {
        let TypedIndex { index } = index;
        let guard = self.get(index)?;
        Ok(ScopedEntry {
            resource: T::try_from_guard(*guard).map_err(|(_, err)| err)?,
            _raw: guard.inner(),
        })
    }

    #[inline]
    pub fn entry_mut<'a, T: FromGuard<Inner = I>>(
        &'a mut self,
        index: TypedIndex<T>,
    ) -> ScopedEntryMutResult<'a, T> {
        let TypedIndex { index } = index;
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
    ) -> ScopedInnerResult<'a, T> {
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
    type Borrowed: BorrowList<C>;
    type Ref<'a>;

    fn get_ref(self, collection: &C) -> GenCollectionResult<Self::Ref<'_>>;
    fn get_owned(self, collection: &mut C) -> GenCollectionResult<Self::Owned>;
    fn get_borrowed(self, collection: &mut C) -> GenCollectionResult<Self::Borrowed>;
}

impl<C: 'static> IndexList<C> for Nil {
    type Owned = Nil;
    type Borrowed = Nil;
    type Ref<'a> = Nil;

    #[inline]
    fn get_ref(self, _: &C) -> GenCollectionResult<Self::Ref<'_>> {
        Ok(Nil::new())
    }

    #[inline]
    fn get_owned(self, _: &mut C) -> GenCollectionResult<Self::Owned> {
        Ok(Nil::new())
    }

    fn get_borrowed(self, _: &mut C) -> GenCollectionResult<Self::Borrowed> {
        Ok(Nil::new())
    }
}

impl<C: 'static, H: 'static, M: Marker, T: IndexList<C>> IndexList<C>
    for Cons<Marked<GenIndex<H>, M>, T>
where
    C: Contains<GenCollection<H>, M>,
{
    type Owned = Cons<H, T::Owned>;
    type Borrowed = Cons<Marked<Borrowed<H>, M>, T::Borrowed>;
    type Ref<'a> = Cons<&'a H, T::Ref<'a>>;

    #[inline]
    fn get_ref(self, collection: &C) -> GenCollectionResult<Self::Ref<'_>> {
        let Cons {
            head: Marked { value: index, .. },
            tail,
        } = self;
        let head = collection.get().get(index)?;
        let tail = tail.get_ref(collection)?;
        Ok(Cons::new(head, tail))
    }

    #[inline]
    fn get_owned(self, collection: &mut C) -> GenCollectionResult<Self::Owned> {
        let Cons {
            head: Marked { value: index, .. },
            tail,
        } = self;
        let head = collection.get_mut().pop(index)?;
        let tail = tail.get_owned(collection)?;
        Ok(Cons::new(head, tail))
    }

    #[inline]
    fn get_borrowed(self, collection: &mut C) -> GenCollectionResult<Self::Borrowed> {
        let Cons {
            head: Marked { value: index, .. },
            tail,
        } = self;
        let tail = tail.get_borrowed(collection)?;
        match collection.get_mut().borrow(index) {
            Ok(item) => Ok(Cons::new(Marked::new(item), tail)),
            Err(err) => {
                tail.put_back(collection).unwrap();
                Err(err)
            }
        }
    }
}

pub trait BorrowList<C: 'static> {
    // Consider if here failure to put back the borrowed item should be considered a fatal error, resulting in pacnic
    // This is because if any single item on the list fails to be put back, the entire list must be considered invalid.
    // It is not possible to defined static error type for this case,
    // as the type varies depend on which entries failed to be put back
    // Hence current implementation leaves a possibility for a partial put back, leaving the collection with a borrowed cells
    // which may not be ever returned. This could cause errors in the future access to the collection,
    // or could be handled by allowing to 'prune' the collection from borrowed cells
    fn put_back(self, collection: &mut C) -> GenCollectionResult<()>;
}

impl<C: 'static> BorrowList<C> for Nil {
    #[inline]
    fn put_back(self, _: &mut C) -> GenCollectionResult<()> {
        Ok(())
    }
}

impl<C: 'static, H: 'static, M: Marker, T: BorrowList<C>> BorrowList<C>
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
    pub fn get_borrow<I: IndexList<T>>(
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

    type TestCopyCollection = list_type![
        GenCollection<u8>,
        GenCollection<u16>,
        GenCollection<u32>,
        Nil
    ];

    type TestNonCopyCollection = list_type![GenCollection<Vec<u8>>, GenCollection<Vec<u16>>, Nil];

    type TestCollectionList = GenCollectionList<TestCopyCollection>;

    #[test]
    fn test_collection_list_index_get_owned() {
        let mut collection = TestCopyCollection::default();

        let collection_u8: &mut GenCollection<u8> = collection.get_mut();
        let index_u8: GenIndex<u8> = collection_u8.push(8).unwrap();

        let collection_u16: &mut GenCollection<u16> = collection.get_mut();
        let index_u16: GenIndex<u16> = collection_u16.push(16).unwrap();

        let collection_u32: &mut GenCollection<u32> = collection.get_mut();
        let index_u32: GenIndex<u32> = collection_u32.push(32).unwrap();

        let index_list = mark![TestCopyCollection, index_u8, index_u16, index_u32];
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
        let mut collection = TestCopyCollection::default();

        let collection_u8: &mut GenCollection<u8> = collection.get_mut();
        let index_u8: GenIndex<u8> = collection_u8.push(8).unwrap();

        let collection_u16: &mut GenCollection<u16> = collection.get_mut();
        let index_u16: GenIndex<u16> = collection_u16.push(16).unwrap();

        let collection_u32: &mut GenCollection<u32> = collection.get_mut();
        let index_u32: GenIndex<u32> = collection_u32.push(32).unwrap();

        let index_list = mark![TestCopyCollection, index_u8, index_u16, index_u32];
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

        let collection_u8: &mut GenCollection<u8> = collection.get_mut();
        let index_u8: GenIndex<u8> = collection_u8.push(8).unwrap();

        let collection_u16: &mut GenCollection<u16> = collection.get_mut();
        let index_u16: GenIndex<u16> = collection_u16.push(16).unwrap();

        let collection_u32: &mut GenCollection<u32> = collection.get_mut();
        let index_u32: GenIndex<u32> = collection_u32.push(32).unwrap();

        let index_list = mark![TestCopyCollection, index_u8, index_u16, index_u32];
        let unpack_list![b_u8, b_u16, b_u32, _rest] =
            index_list.get_borrowed(&mut collection).unwrap();

        assert_eq!(**b_u8, 8);
        assert_eq!(**b_u16, 16);
        assert_eq!(**b_u32, 32);

        let collection_u8: &GenCollection<u8> = collection.get();
        let collection_u16: &GenCollection<u16> = collection.get();
        let collection_u32: &GenCollection<u32> = collection.get();

        assert_eq!(collection_u8.len(), 1);
        assert_eq!(collection_u16.len(), 1);
        assert_eq!(collection_u32.len(), 1);

        let collection_u8: &mut GenCollection<u8> = collection.get_mut();
        assert!(matches!(
            collection_u8.pop(index_u8),
            Err(GenCollectionError::CellBorrowed)
        ));

        let collection_u16: &mut GenCollection<u16> = collection.get_mut();
        assert!(matches!(
            collection_u16.pop(index_u16),
            Err(GenCollectionError::CellBorrowed)
        ));

        let collection_u32: &mut GenCollection<u32> = collection.get_mut();
        assert!(matches!(
            collection_u32.pop(index_u32),
            Err(GenCollectionError::CellBorrowed)
        ));

        let borrowed = list_value![b_u8, b_u16, b_u32, Nil::new()];
        assert!(matches!(borrowed.put_back(&mut collection), Ok(..)));

        let collection_u8: &mut GenCollection<u8> = collection.get_mut();
        assert!(matches!(collection_u8.pop(index_u8), Ok(8)));

        let collection_u16: &mut GenCollection<u16> = collection.get_mut();
        assert!(matches!(collection_u16.pop(index_u16), Ok(16)));

        let collection_u32: &mut GenCollection<u32> = collection.get_mut();
        assert!(matches!(collection_u32.pop(index_u32), Ok(32)));
    }

    #[test]
    fn test_collection_list_index_get_borrow_non_copy_type() {
        let mut collection = TestNonCopyCollection::default();

        let collection_vec_u8: &mut GenCollection<Vec<u8>> = collection.get_mut();
        let index_vec_u8: GenIndex<Vec<u8>> = collection_vec_u8.push(vec![8]).unwrap();

        let collection_vec_u16: &mut GenCollection<Vec<u16>> = collection.get_mut();
        let index_vec_u16: GenIndex<Vec<u16>> = collection_vec_u16.push(vec![16]).unwrap();

        let index_list = mark![TestNonCopyCollection, index_vec_u8, index_vec_u16];
        let unpack_list![b_vec_u8, b_vec_u16, _rest] =
            index_list.get_borrowed(&mut collection).unwrap();

        assert_eq!(**b_vec_u8, vec![8]);
        assert_eq!(**b_vec_u16, vec![16]);

        let collection_vec_u8: &GenCollection<Vec<u8>> = collection.get();
        let collection_vec_u16: &GenCollection<Vec<u16>> = collection.get();

        assert_eq!(collection_vec_u8.len(), 1);
        assert_eq!(collection_vec_u16.len(), 1);

        let collection_vec_u8: &mut GenCollection<Vec<u8>> = collection.get_mut();
        assert!(matches!(
            collection_vec_u8.pop(index_vec_u8),
            Err(GenCollectionError::CellBorrowed)
        ));

        let collection_vec_u16: &mut GenCollection<Vec<u16>> = collection.get_mut();
        assert!(matches!(
            collection_vec_u16.pop(index_vec_u16),
            Err(GenCollectionError::CellBorrowed)
        ));

        let borrowed = list_value![b_vec_u8, b_vec_u16, Nil::new()];
        assert!(matches!(borrowed.put_back(&mut collection), Ok(..)));

        let collection_vec_u8: &mut GenCollection<Vec<u8>> = collection.get_mut();
        assert!(matches!(collection_vec_u8.pop(index_vec_u8), Ok(..)));

        let collection_vec_u16: &mut GenCollection<Vec<u16>> = collection.get_mut();
        assert!(matches!(collection_vec_u16.pop(index_vec_u16), Ok(..)));
    }

    #[test]
    fn test_gen_collection_list() {
        let mut collection = TestCollectionList::new();
        let index_u8: GenIndex<u8> = collection.push(8u8.into()).unwrap();
        let index_u16: GenIndex<u16> = collection.push(16u16.into()).unwrap();
        let index_u32: GenIndex<u32> = collection.push(32u32.into()).unwrap();

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
            assert!(context.destroy(&mut collection).is_ok());
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
            assert!(context.destroy(&mut collection).is_ok());
        }
    }
}

#[derive(Debug)]
pub struct BorrowedGuard<T: FromGuard> {
    item: T,
    index: TypedIndex<T>,
}

impl<T: FromGuard> Deref for BorrowedGuard<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T: FromGuard> DerefMut for BorrowedGuard<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}

impl<T: FromGuard> From<BorrowedGuard<T>> for Borrowed<Guard<T>> {
    #[inline]
    fn from(value: BorrowedGuard<T>) -> Self {
        let BorrowedGuard {
            item,
            index: TypedIndex { index },
        } = value;
        Borrowed {
            item: item.into_guard(),
            index,
        }
    }
}

impl<T: FromGuard> TryFrom<Borrowed<Guard<T>>> for BorrowedGuard<T> {
    type Error = (Borrowed<Guard<T>>, TypeGuardConversionError);

    #[inline]
    fn try_from(value: Borrowed<Guard<T>>) -> Result<Self, Self::Error> {
        let Borrowed { item, index } = value;
        Ok(Self {
            item: T::try_from_guard(item)
                .map_err(|(guard, err)| (Borrowed { item: guard, index }, err))?,
            index: TypedIndex { index },
        })
    }
}

impl<C: 'static, H: FromGuard, M: Marker, T: BorrowList<C>> BorrowList<C>
    for Cons<Marked<BorrowedGuard<H>, M>, T>
where
    C: Contains<TypeGuardCollection<H::Inner>, M>,
{
    #[inline]
    fn put_back(self, collection: &mut C) -> GenCollectionResult<()> {
        let Cons {
            head: Marked { value: borrow, .. },
            tail,
        } = self;
        collection.get_mut().put_back(borrow.into())?;
        tail.put_back(collection)
    }
}

impl<C: 'static, H: FromGuard, M: Marker, T: IndexList<C>> IndexList<C>
    for Cons<Marked<TypedIndex<H>, M>, T>
where
    C: Contains<TypeGuardCollection<H::Inner>, M>,
{
    type Borrowed = Cons<Marked<BorrowedGuard<H>, M>, T::Borrowed>;
    type Owned = Cons<H, T::Owned>;
    type Ref<'a> = Cons<&'a TypeGuard<H::Inner>, T::Ref<'a>>;

    #[inline]
    fn get_ref(self, collection: &C) -> GenCollectionResult<Self::Ref<'_>> {
        let Cons {
            head:
                Marked {
                    value: TypedIndex { index },
                    ..
                },
            tail,
        } = self;
        let head = collection.get().get(index)?;
        let tail = tail.get_ref(collection)?;
        Ok(Cons::new(head, tail))
    }

    // Consider error handling for when the some, other than last, index on the list is invalid
    // This could lead to resources beein leaked, as the collection would not be able to put back the borrowed resources,
    // which in case of resources that should be manually released, e.g. implementing Destroy trait, could lead to memory leaks
    // Hence, for these 'index lists' the error handling should be performed for all indices,
    // before any state is modified, and only then the items should be pulled out of the collections,
    // this way we would always end with the correct state of the collection, either the items properly removed and handed to the user,
    // so the user is responsible for their destruction, or the items are put back to the collection, so the collection can handle their destruction on drop
    #[inline]
    fn get_owned(self, collection: &mut C) -> GenCollectionResult<Self::Owned> {
        let Cons {
            head:
                Marked {
                    value: TypedIndex { index },
                    ..
                },
            tail,
        } = self;
        let tail = tail.get_owned(collection)?;
        let head = collection
            .get_mut()
            .pop(index)?
            .try_into_outer()
            .map_err(|(_, err)| GenCollectionError::TypeGuardConversion(err))?;

        Ok(Cons::new(head, tail))
    }

    #[inline]
    fn get_borrowed(self, collection: &mut C) -> GenCollectionResult<Self::Borrowed> {
        let Cons {
            head:
                Marked {
                    value: TypedIndex { index },
                    ..
                },
            tail,
        } = self;
        let tail = tail.get_borrowed(collection)?;
        let result = match collection.get_mut().borrow(index) {
            Ok(borrow) => match borrow.try_into() {
                Ok(borrow) => Ok(borrow),
                Err((borrow, err)) => {
                    collection.get_mut().put_back(borrow).unwrap();
                    Err(GenCollectionError::TypeGuardConversion(err))
                }
            },
            Err(err) => Err(err),
        };
        match result {
            Ok(borrow) => Ok(Cons::new(Marked::new(borrow), tail)),
            Err(err) => {
                tail.put_back(collection).unwrap();
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod test_type_guard_borrow_list {
    use std::convert::Infallible;

    use super::*;

    use crate::{
        list_type,
        type_guard::test_types::{A, B},
        unpack_list, Cons, Nil,
    };

    type TestTypeGuardCollection = list_type![TypeGuardCollection<u32>, Nil];
    type TestTypeGuardCollectionList = GenCollectionList<TestTypeGuardCollection>;

    #[test]
    fn test_type_guard_borrow() {
        let mut collection = TestTypeGuardCollection::default();
        let index_a = TypedIndex::<A>::new(collection.push(A(42).into_guard()).unwrap());
        let index_b = TypedIndex::<B>::new(collection.push(B(42).into_guard()).unwrap());

        let index_list = mark![TestTypeGuardCollection, index_a, index_b];
        let borrow = index_list.get_borrowed(&mut collection).unwrap();
        borrow.put_back(&mut collection).unwrap();
    }

    #[test]
    fn test_invalid_type_cast_does_not_invalidate_collection() {
        let mut collection = TestTypeGuardCollection::default();
        let index_inner_a = collection.push(A(42).into_guard()).unwrap();
        let index_inner_b = collection.push(B(31).into_guard()).unwrap();

        let index_a_invalid = TypedIndex::<A>::new(index_inner_b);
        let index_b_invalid = TypedIndex::<B>::new(index_inner_a);

        let index_list = mark![TestTypeGuardCollection, index_a_invalid, index_b_invalid];
        let borrow = index_list.get_borrowed(&mut collection);
        assert!(matches!(
            borrow,
            Err(GenCollectionError::TypeGuardConversion(..))
        ));

        let index_a_valid = TypedIndex::<A>::new(index_inner_a);
        let index_b_valid = TypedIndex::<B>::new(index_inner_b);

        let index_list = mark![TestTypeGuardCollection, index_a_valid, index_b_valid];
        let borrow = index_list.get_borrowed(&mut collection);
        assert!(borrow.is_ok());
        borrow.unwrap().put_back(&mut collection).unwrap();
    }

    #[test]
    fn test_type_guard_context_works_with_borrow_context() {
        let mut collection = TestTypeGuardCollectionList::default();
        let index_inner_a = collection.push(A(42).into_guard()).unwrap();
        let index_inner_b = collection.push(B(31).into_guard()).unwrap();

        let index_a = TypedIndex::<A>::new(index_inner_a);
        let index_b = TypedIndex::<B>::new(index_inner_b);

        let index_list = mark![TestTypeGuardCollection, index_a, index_b];
        {
            let mut borrow = collection.get_borrow(index_list).unwrap();
            let _ = borrow.operate_ref::<_, Infallible, _>(|borrow| {
                let unpack_list![b_a, b_b, _rest] = borrow;
                assert_eq!(b_a.0, 42);
                assert_eq!(b_b.0, 31);
                Ok(())
            });
            assert!(borrow.destroy(&mut collection).is_ok());
        }
        {
            let mut borrow = collection.get_borrow(index_list).unwrap();
            let _ = borrow.operate_mut::<_, Infallible, _>(|borrow| {
                let unpack_list![b_a, b_b, _rest] = borrow;
                b_a.0 = 41;
                b_b.0 = 30;
                Ok(())
            });
            assert!(borrow.destroy(&mut collection).is_ok());
        }
        {
            let mut borrow = collection.get_borrow(index_list).unwrap();
            let _ = borrow.operate_ref::<_, Infallible, _>(|borrow| {
                let unpack_list![b_a, b_b, _rest] = borrow;
                assert_eq!(b_a.0, 41);
                assert_eq!(b_b.0, 30);
                Ok(())
            });
            assert!(borrow.destroy(&mut collection).is_ok());
        }
        let collection_u32: &TypeGuardCollection<u32> = collection.get();
        assert_eq!(collection_u32.len(), 2);
    }
}
