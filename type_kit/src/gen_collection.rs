use std::any::type_name;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_types::{A, B};

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
        let mut collection = GuardCollection::<u32>::default();
        let index_a = collection.push(A(42).into_guard()).unwrap();
        let index_b = collection.push(B(31).into_guard()).unwrap();

        let entry: ScopedEntry<'_, A> = collection.entry(index_a).unwrap();
        assert_eq!(entry.0, 42);
        let entry: ScopedEntry<'_, B> = collection.entry(index_b).unwrap();
        assert_eq!(entry.0, 31);
    }

    #[test]
    fn test_guard_collection_entry_invalid_index() {
        let mut collection = GuardCollection::<u32>::default();
        let index_a = collection.push(A(42).into_guard()).unwrap();
        let index_b = collection.push(B(31).into_guard()).unwrap();

        let entry: ScopedResult<B> = collection.entry(index_a);
        assert!(entry.is_err());
        let entry: ScopedResult<A> = collection.entry(index_b);
        assert!(entry.is_err());
    }

    #[test]
    fn test_guard_collection_mut_entry_update_on_drop() {
        let mut collection = GuardCollection::<u32>::default();
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
}

#[derive(Debug, Clone, Copy)]
pub enum GenCollectionError {
    InvalidGeneration { expected: usize, actual: usize },
    InvalidIndex { index: usize, len: usize },
    InvalidItemIndex { index: usize, len: usize },
    CellEmpty,
    CellOccupied,
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
        }
    }
}

impl Error for GenCollectionError {}

pub type GenCollectionResult<T> = Result<T, GenCollectionError>;

mod cell {
    use super::{GenCollectionError, GenCollectionResult};

    #[derive(Debug, Clone, Copy)]
    struct Occupied {
        generation: usize,
        item_index: usize,
    }

    #[derive(Debug, Clone, Copy)]
    struct Empty {
        prev_generation: usize,
        next_free: Option<usize>,
    }

    #[derive(Debug)]
    pub(super) struct LockedCell {
        cell: GenCell,
    }

    impl LockedCell {
        #[inline]
        pub(super) fn new(item_index: usize) -> Self {
            Self {
                cell: GenCell::Occupied(Occupied {
                    generation: 0,
                    item_index,
                }),
            }
        }

        #[inline]
        pub(super) fn unlock(&self, generation: usize) -> GenCollectionResult<&GenCell> {
            let cell_generation = self.cell.generation()?;
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
            let cell_generation = self.cell.generation()?;
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
                GenCell::Empty(Empty {
                    prev_generation,
                    next_free,
                }) => {
                    let generation = prev_generation + 1;
                    self.cell = GenCell::Occupied(Occupied {
                        generation,
                        item_index,
                    });
                    Ok((generation, next_free))
                }
                GenCell::Occupied(..) => Err(GenCollectionError::CellOccupied),
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
            }
        }
    }

    #[allow(private_interfaces)]
    #[derive(Debug, Clone, Copy)]
    pub(super) enum GenCell {
        Occupied(Occupied),
        Empty(Empty),
    }

    impl GenCell {
        #[inline]
        pub(super) fn pop(&mut self, next_free: Option<usize>) -> GenCollectionResult<usize> {
            match *self {
                GenCell::Occupied(cell) => {
                    *self = GenCell::Empty(Empty {
                        next_free,
                        prev_generation: cell.generation,
                    });
                    Ok(cell.item_index)
                }
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
            }
        }

        #[inline]
        pub(super) fn generation(&self) -> GenCollectionResult<usize> {
            match self {
                GenCell::Occupied(cell) => Ok(cell.generation),
                GenCell::Empty(..) => Err(GenCollectionError::CellEmpty),
            }
        }

        #[inline]
        pub(super) fn item_index(&self) -> GenCollectionResult<usize> {
            match self {
                GenCell::Occupied(cell) => Ok(cell.item_index),
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

use crate::{FromGuard, Guard, TypeGuard, TypeGuardConversionError, Valid};

#[derive(Clone, Copy)]
pub struct GenIndex<T> {
    index: usize,
    generation: usize,
    _phantom: PhantomData<T>,
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

#[derive(Debug, Clone, Copy)]
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

pub struct ScopedEntry<'a, T: FromGuard> {
    resource: T,
    _raw: &'a T::Inner,
}

impl<'a, T: FromGuard> Deref for ScopedEntry<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

pub struct ScopedEntryMut<'a, T: FromGuard> {
    resource: Option<T>,
    raw: &'a mut T::Inner,
}

impl<'a, T: FromGuard> Drop for ScopedEntryMut<'a, T> {
    fn drop(&mut self) {
        *self.raw = self.resource.take().unwrap().into_inner();
    }
}

impl<'a, T: FromGuard> Deref for ScopedEntryMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().unwrap()
    }
}

impl<'a, T: FromGuard> DerefMut for ScopedEntryMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resource.as_mut().unwrap()
    }
}

pub type GuardIndex<T> = GenIndex<Guard<T>>;
pub type GuardCollection<T> = GenCollection<TypeGuard<T>>;

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

pub type ScopedResult<'a, T> = Result<ScopedEntry<'a, T>, GuardCollectionError>;
pub type ScopedMutResult<'a, T> = Result<ScopedEntryMut<'a, T>, GuardCollectionError>;

impl<I: Clone + Copy> GuardCollection<I> {
    #[inline]
    pub fn entry<'a, T: FromGuard<Inner = I>>(&'a self, index: GuardIndex<T>) -> ScopedResult<T> {
        let guard = self.get(index)?;
        Ok(ScopedEntry {
            resource: T::try_from_guard(*guard)?,
            _raw: guard.inner(),
        })
    }

    #[inline]
    pub fn entry_mut<'a, T: FromGuard<Inner = I>>(
        &'a mut self,
        index: GuardIndex<T>,
    ) -> ScopedMutResult<'a, T> {
        let guard = self.get_mut(index)?;
        Ok(ScopedEntryMut {
            resource: Some(T::try_from_guard(*guard)?),
            raw: guard.inner_mut(),
        })
    }
}
