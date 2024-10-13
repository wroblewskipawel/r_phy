use std::error::Error;
use std::fmt::{Display, Formatter};

#[cfg(test)]
mod tests {
    use super::*;

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

#[derive(Debug, Clone, Copy)]
pub struct GenIndex<T> {
    index: usize,
    generation: usize,
    _phantom: PhantomData<T>,
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

#[derive(Debug, Default)]
pub struct GenCollection<T> {
    items: Vec<T>,
    indices: Vec<LockedCell>,
    mapping: Vec<usize>,
    next_free: Option<usize>,
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
