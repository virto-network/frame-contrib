// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
mod impls;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use core::iter::Map;
use frame_support::{storage::PrefixIterator, traits::OriginTrait};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_referenda::{BalanceOf, PalletsOriginOf, Track};
use sp_core::Get;
use sp_std::{borrow::Cow, vec::Vec};

pub use pallet::*;
pub use weights::WeightInfo;

pub type TrackIdOf<T, I = ()> = <T as Config<I>>::TrackId;
pub type TrackInfoOf<T, I = ()> = pallet_referenda::TrackInfoOf<T, I>;
pub type TrackOf<T, I> = Track<<T as Config<I>>::TrackId, BalanceOf<T, I>, BlockNumberFor<T>>;

type TracksIter<T, I> = Map<
    PrefixIterator<(<T as Config<I>>::TrackId, TrackInfoOf<T, I>)>,
    fn((<T as Config<I>>::TrackId, TrackInfoOf<T, I>)) -> Cow<'static, TrackOf<T, I>>,
>;

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
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_referenda::Config<I> {
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        type UpdateOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, TrackIdOf<Self, I>>;

        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type TrackId: Parameter + Member + Copy + MaxEncodedLen + Ord;

        type MaxTracks: Get<u32>;
        ///
        type WeightInfo: WeightInfo;

        #[cfg(feature = "runtime-benchmarks")]
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
            <Self as fc_traits_tracks::MutateTracks<BalanceOf<T, I>, BlockNumberFor<T>>>::insert(
                id,
                info,
                pallet_origin,
            )
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
            <Self as fc_traits_tracks::MutateTracks<BalanceOf<T, I>, BlockNumberFor<T>>>::update(
                id, info,
            )
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
            <Self as fc_traits_tracks::MutateTracks<BalanceOf<T, I>, BlockNumberFor<T>>>::remove(
                id,
                pallet_origin,
            )
        }
    }
}
