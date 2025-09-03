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
mod split_id;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use codec::DecodeWithMemTracking;
use frame_support::traits::OriginTrait;
use pallet_referenda::{BalanceOf, BlockNumberFor, Curve, PalletsOriginOf, Track, TrackInfoOf};

pub use pallet::UpdateType;
pub use pallet::*;
pub use split_id::SplitId;
pub use weights::WeightInfo;

pub type TrackIdOf<T, I = ()> = <T as Config<I>>::TrackId;
pub type GroupIdOf<T, I = ()> = <TrackIdOf<T, I> as SplitId>::Half;
pub type SubTrackIdOf<T, I = ()> = <TrackIdOf<T, I> as SplitId>::Half;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        dispatch::DispatchResult,
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
        type CreateOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, PalletsOriginOf<Self>>;
        /// An origin that is authorized to mutate an existing origin.
        type GroupManagerCreateOrigin: EnsureOriginWithArg<
            Self::RuntimeOrigin,
            PalletsOriginOf<Self>,
            Success = GroupIdOf<Self, I>,
        >;
        type GroupManagerOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, Self::TrackId>;

        // Types: A set of parameter types that the pallet uses to handle information.

        /// The ID of a single track.
        type TrackId: SplitId + Parameter + Member + Copy + MaxEncodedLen + Ord;

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
    pub type NextGroupId<T: Config<I>, I: 'static = ()> =
        StorageValue<_, GroupIdOf<T, I>, ValueQuery>;

    #[pallet::storage]
    pub type TracksIds<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BoundedBTreeSet<TrackIdOf<T, I>, T::MaxTracks>, ValueQuery>;

    #[pallet::storage]
    pub type OriginToTrackId<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, PalletsOriginOf<T>, TrackIdOf<T, I>>;

    #[pallet::storage]
    pub type Tracks<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        GroupIdOf<T, I>,
        Blake2_128Concat,
        SubTrackIdOf<T, I>,
        TrackInfoOf<T, I>,
    >;

    #[derive(
        Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, DecodeWithMemTracking, TypeInfo,
    )]
    pub enum UpdateType {
        /// Decision deposit was updated
        DecisionDeposit,
        /// One or more periods were updated
        Periods,
        /// One or more curves were updated
        Curves,
        /// Full track configuration was updated
        Full,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// A new track has been inserted
        Created { id: TrackIdOf<T, I> },
        /// A track has been updated
        Updated {
            id: TrackIdOf<T, I>,
            update_type: UpdateType,
        },
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
        #[pallet::call_index(0)]
        pub fn new_group_with_track(
            origin: OriginFor<T>,
            group_origin: PalletsOriginOf<T>,
            info: TrackInfoOf<T, I>,
        ) -> DispatchResult {
            T::CreateOrigin::ensure_origin(origin, &group_origin)?;
            let id = Self::next_group_track_id().ok_or_else(|| Error::<T, I>::MaxTracksExceeded)?;
            Self::do_insert(id, info, group_origin)
        }

        #[pallet::call_index(1)]
        pub fn add_sub_track(
            origin: OriginFor<T>,
            sub_track_id: SubTrackIdOf<T, I>,
            sub_origin: PalletsOriginOf<T>,
            info: TrackInfoOf<T, I>,
        ) -> DispatchResult {
            let group = T::GroupManagerCreateOrigin::ensure_origin(origin, &sub_origin)?;
            let id = T::TrackId::combine(group, sub_track_id);
            Self::do_insert(id, info, sub_origin)
        }

        // #[pallet::call_index(0)]
        // pub fn insert(
        //     origin: OriginFor<T>,
        //     id: TrackIdOf<T, I>,
        //     info: TrackInfoOf<T, I>,
        //     pallet_origin: PalletsOriginOf<T>,
        // ) -> DispatchResult {
        //     T::AdminOrigin::ensure_origin(origin)?;
        //     Self::do_insert(id, info, pallet_origin)
        // }

        /// Update the configuration of an existing referenda Track.
        ///
        /// **DEPRECATED**: Use granular methods instead (`set_decision_deposit`, `set_periods`, `set_curves`).
        ///
        /// Parameters:
        /// - `id`: The Id of the track to be updated.
        /// - `info`: The new configuration of the track.
        ///
        /// Emits `Updated` event when successful.
        ///
        /// Weight: `O(1)`
        // #[pallet::call_index(1)]
        // #[deprecated(
        //     note = "Use granular methods instead: set_decision_deposit, set_periods, set_curves"
        // )]
        // pub fn update(
        //     origin: OriginFor<T>,
        //     id: TrackIdOf<T, I>,
        //     info: TrackInfoOf<T, I>,
        // ) -> DispatchResult {
        //     T::GroupManagerOrigin::ensure_origin(origin, &id)?;
        //     Self::do_update(id, info)?;
        //     Self::deposit_event(Event::Updated {
        //         id,
        //         update_type: UpdateType::Full,
        //     });
        //     Ok(())
        // }

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
            T::GroupManagerOrigin::ensure_origin(origin, &id)?;
            Self::do_remove(id, pallet_origin)
        }

        /// Set the decision deposit for an existing track.
        ///
        /// Parameters:
        /// - `id`: The Id of the track to be updated.
        /// - `deposit`: The new decision deposit amount.
        ///
        /// Emits `Updated` event when successful.
        ///
        /// Weight: `O(1)`
        #[pallet::call_index(3)]
        pub fn set_decision_deposit(
            origin: OriginFor<T>,
            id: T::TrackId,
            deposit: BalanceOf<T, I>,
        ) -> DispatchResult {
            T::GroupManagerOrigin::ensure_origin(origin, &id)?;
            Self::do_set_decision_deposit(id, deposit)?;
            Self::deposit_event(Event::Updated {
                id,
                update_type: UpdateType::DecisionDeposit,
            });
            Ok(())
        }

        /// Set periods for an existing track.
        ///
        /// Parameters:
        /// - `id`: The Id of the track to be updated.
        /// - `prepare`: Optional new prepare period.
        /// - `decision`: Optional new decision period.
        /// - `confirm`: Optional new confirm period.
        /// - `min_enactment`: Optional new minimum enactment period.
        ///
        /// Emits `Updated` event when successful.
        ///
        /// Weight: `O(1)`
        #[pallet::call_index(4)]
        pub fn set_periods(
            origin: OriginFor<T>,
            id: TrackIdOf<T, I>,
            prepare: Option<pallet_referenda::BlockNumberFor<T, I>>,
            decision: Option<pallet_referenda::BlockNumberFor<T, I>>,
            confirm: Option<pallet_referenda::BlockNumberFor<T, I>>,
            min_enactment: Option<pallet_referenda::BlockNumberFor<T, I>>,
        ) -> DispatchResult {
            T::GroupManagerOrigin::ensure_origin(origin, &id)?;
            Self::do_set_periods(id, prepare, decision, confirm, min_enactment)?;
            Self::deposit_event(Event::Updated {
                id,
                update_type: UpdateType::Periods,
            });
            Ok(())
        }

        /// Set curves for an existing track.
        ///
        /// Parameters:
        /// - `id`: The Id of the track to be updated.
        /// - `min_approval`: Optional new minimum approval curve.
        /// - `min_support`: Optional new minimum support curve.
        ///
        /// Emits `Updated` event when successful.
        ///
        /// Weight: `O(1)`
        #[pallet::call_index(5)]
        pub fn set_curves(
            origin: OriginFor<T>,
            id: TrackIdOf<T, I>,
            min_approval: Option<Curve>,
            min_support: Option<Curve>,
        ) -> DispatchResult {
            T::GroupManagerOrigin::ensure_origin(origin, &id)?;
            Self::do_set_curves(id, min_approval, min_support)?;
            Self::deposit_event(Event::Updated {
                id,
                update_type: UpdateType::Curves,
            });
            Ok(())
        }
    }
}
