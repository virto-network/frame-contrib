#![cfg_attr(not(feature = "std"), no_std)]

//! # Referenda Tracks Pallet
//!
//! - [`Config`][Config]
//! - [`Call`][Call]
//!
//! ## Overview
//!
//! Manage referenda voting tracks.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - [`insert`][`crate::Pallet::insert`] - Insert a new referenda Track.
//! - [`update`][`crate::Pallet::update`] - Update the configuration of an existing referenda Track.
//! - [`remove`][`crate::Pallet::remove`] - Remove an existing track

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
mod impls;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use frame_support::traits::OriginTrait;
use pallet_referenda::{BalanceOf, BlockNumberFor, PalletsOriginOf, Track, TrackInfoOf};
use sp_core::Get;

pub use pallet::*;
pub use weights::WeightInfo;

pub type TrackIdOf<T, I = ()> = <T as Config<I>>::TrackId;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{EnsureOrigin, EnsureOriginWithArg},
    };
    use frame_system::pallet_prelude::*;

    #[cfg(feature = "runtime-benchmarks")]
    pub trait BenchmarkHelper<T: Config<I>, I: 'static = ()> {
        fn track_id(id: u32) -> TrackIdOf<T, I>;
    }

    #[pallet::config]
    pub trait Config<I: 'static = ()>:
        frame_system::Config<RuntimeEvent: From<Event<Self, I>>> + pallet_referenda::Config<I>
    {
        // Primitives: Some overarching types that come from the system (or the system depends on).

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        // Origins: Types that manage authorization rules to allow or deny some caller origins to
        // execute a method.

        /// An origin that is authorized to mutate the list of origins.
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// An origin that is authorized to mutate an existing origin.
        type UpdateOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, TrackIdOf<Self, I>>;

        // Types: A set of parameter types that the pallet uses to handle information.

        /// The ID of a single track.
        type TrackId: Parameter + Member + Copy + MaxEncodedLen + Ord;

        // Parameters: A set of constant parameters to configure limits.

        /// The maximum amount of tracks which can be configured in this module
        type MaxTracks: Get<u32>;

        // Benchmarking: Types to handle benchmarks.
        #[cfg(feature = "runtime-benchmarks")]
        /// A helper trait to set up benchmark tests.
        type BenchmarkHelper: BenchmarkHelper<Self, I>;
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::storage]
    pub type TracksIds<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BoundedVec<TrackIdOf<T, I>, <T as Config<I>>::MaxTracks>, ValueQuery>;

    #[pallet::storage]
    pub type OriginToTrackId<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, PalletsOriginOf<T>, TrackIdOf<T, I>>;

    #[pallet::storage]
    pub type Tracks<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, TrackIdOf<T, I>, TrackInfoOf<T, I>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// A new track has been inserted
        Created { id: TrackIdOf<T, I> },
        /// The information for a track has been updated
        Updated { id: TrackIdOf<T, I> },
        /// A track has been removed
        Removed { id: TrackIdOf<T, I> },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// The maxmimum amount of track IDs was exceeded
        MaxTracksExceeded,
        /// The specified ID for this track was not found
        TrackIdNotFound,
        /// The specified ID for this track was already existing
        TrackIdAlreadyExisting,
        /// The track cannot be removed
        CannotRemove,
    }

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Insert a new referenda Track.
        ///
        /// Parameters:
        /// - `id`: The Id of the track to be inserted.
        /// - `info`: The configuration of the track.
        /// - `pallet_origin`: A generic origin that will be matched to the track.
        ///
        /// Emits `Created` event when successful.
        ///
        /// Weight: `O(1)`
        #[pallet::call_index(0)]
        pub fn insert(
            origin: OriginFor<T>,
            id: TrackIdOf<T, I>,
            info: TrackInfoOf<T, I>,
            pallet_origin: PalletsOriginOf<T>,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::do_insert(id, info, pallet_origin)
        }

        /// Update the configuration of an existing referenda Track.
        ///
        /// Parameters:
        /// - `id`: The Id of the track to be updated.
        /// - `info`: The new configuration of the track.
        ///
        /// Emits `Updated` event when successful.
        ///
        /// Weight: `O(1)`
        #[pallet::call_index(1)]
        pub fn update(
            origin: OriginFor<T>,
            id: TrackIdOf<T, I>,
            info: TrackInfoOf<T, I>,
        ) -> DispatchResult {
            T::UpdateOrigin::ensure_origin(origin, &id)?;
            Self::do_update(id, info)
        }

        /// Remove an existing track
        ///
        /// Parameters:
        /// - `id`: The Id of the track to be deleted.
        ///
        /// Emits `Removed` event when successful.
        ///
        /// Weight: `O(MaxTracks)`
        #[pallet::call_index(2)]
        pub fn remove(
            origin: OriginFor<T>,
            id: TrackIdOf<T, I>,
            pallet_origin: PalletsOriginOf<T>,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::do_remove(id, pallet_origin)
        }
    }
}
