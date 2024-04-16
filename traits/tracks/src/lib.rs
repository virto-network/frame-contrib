#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::DispatchResult;
use pallet_referenda::{TrackInfo, TracksInfo};

pub use crate::Mutate as MutateTracks;

pub trait Mutate<Balance, Moment>: TracksInfo<Balance, Moment>
where
    Balance: Clone + 'static,
    Moment: Clone + 'static,
{
    /// Inserts a new track into the tracks storage.
    fn insert(
        id: Self::Id,
        info: TrackInfo<Balance, Moment>,
        origin: Self::RuntimeOrigin,
    ) -> DispatchResult;

    /// Updates an existing track with the given Id.
    fn update(id: Self::Id, info: TrackInfo<Balance, Moment>) -> DispatchResult;

    /// Removes an existing track with the given Id,
    /// ensuring that the dispatch origin matches.
    fn remove(id: Self::Id, maybe_origin: Some<Self::RuntimeOrigin>) -> DispatchResult;
}
