#![no_std]

/// Defines a predicate to determine whether to select an item or not within a
/// filter operation.
pub trait SelectNonFungibleItem<CollectionId, ItemId> {
    /// Returns `true` if the item should be selected in a filter operation
    fn select(&self, collection_id: CollectionId, item_id: ItemId) -> bool;
}

impl<C, I> SelectNonFungibleItem<C, I> for () {
    fn select(&self, _: C, _: I) -> bool {
        true
    }
}

impl<CollectionId, ItemId, T> SelectNonFungibleItem<CollectionId, ItemId> for T
where
    T: Fn(CollectionId, ItemId) -> bool,
    CollectionId: Clone,
    ItemId: Clone,
{
    fn select(&self, collection_id: CollectionId, item_id: ItemId) -> bool {
        self(collection_id, item_id)
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::boxed::Box;

    #[test]
    fn it_works() {
        // No-op works
        assert!(SelectNonFungibleItem::<u32, u32>::select(&(), 1, 1));
        assert!(SelectNonFungibleItem::<u32, u32>::select(&(), 1, 2));

        // Specific function works
        let select: Box<dyn SelectNonFungibleItem<u32, u32>> = Box::new(|a, b| (a + b) < 3u32);
        assert!(select.select(1, 1));
        assert!(!select.select(1, 2));
    }
}
