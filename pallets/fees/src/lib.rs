#![cfg_attr(not(feature = "std"), no_std)]

//! # Fees Pallet
//!
//! Allows protocol governance and communities to configure custom fees
//! that are applied on asset transfers. Protocol fees always apply;
//! community fees apply when the sender is detected as a community member.

extern crate alloc;

use alloc::vec::Vec;
use frame::prelude::*;
use frame::deps::frame_support::traits::fungibles;
use sp_runtime::traits::Zero;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod adapter;
mod extensions;
pub mod types;

pub use adapter::*;
pub use extensions::*;
pub use pallet::*;
pub use types::*;

#[frame::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Community identifier type.
        type CommunityId: Parameter + MaxEncodedLen + Copy;

        /// Maximum length of a fee name.
        #[pallet::constant]
        type MaxFeeNameLen: Get<u32>;

        /// Maximum number of protocol-level fees.
        #[pallet::constant]
        type MaxProtocolFees: Get<u32>;

        /// Maximum number of fees per community.
        #[pallet::constant]
        type MaxCommunityFees: Get<u32>;

        /// Origin for setting protocol-level fees.
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Origin for setting community-level fees, returns the community ID.
        type CommunityOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::CommunityId>;

        /// Maps accounts to their community.
        type CommunityDetector: AccountCommunity<Self::AccountId, Self::CommunityId>;

        /// The fungibles implementation providing asset transfers.
        type Assets: fungibles::Inspect<Self::AccountId, Balance: Ord>
            + fungibles::Unbalanced<Self::AccountId>
            + fungibles::Mutate<Self::AccountId>;

        /// Inspects runtime calls to detect asset transfer operations
        /// for the transaction extension.
        type CallInspector: CallInspector<Self::RuntimeCall, AssetIdOf<Self>, BalanceOf<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Protocol-level fees that apply to all asset transfers.
    #[pallet::storage]
    pub type ProtocolFees<T: Config> =
        StorageValue<_, BoundedVec<NamedFeeEntryOf<T>, T::MaxProtocolFees>, ValueQuery>;

    /// Per-community fees that apply when the sender belongs to a community.
    #[pallet::storage]
    pub type CommunityFees<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::CommunityId,
        BoundedVec<NamedFeeEntryOf<T>, T::MaxCommunityFees>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ProtocolFeeSet {
            name: FeeNameOf<T>,
        },
        ProtocolFeeRemoved {
            name: FeeNameOf<T>,
        },
        CommunityFeeSet {
            community: T::CommunityId,
            name: FeeNameOf<T>,
        },
        CommunityFeeRemoved {
            community: T::CommunityId,
            name: FeeNameOf<T>,
        },
        /// Fees were charged on a transfer.
        FeesCharged {
            who: T::AccountId,
            total_fees: BalanceOf<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Too many fees configured.
        TooManyFees,
        /// The specified fee was not found.
        FeeNotFound,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set or update a protocol-level fee. Requires `AdminOrigin`.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn set_protocol_fee(
            origin: OriginFor<T>,
            name: FeeNameOf<T>,
            config: FeeConfigOf<T>,
            beneficiary: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            ProtocolFees::<T>::try_mutate(|fees| {
                if let Some(entry) = fees.iter_mut().find(|e| e.name == name) {
                    entry.config = config;
                    entry.beneficiary = beneficiary;
                } else {
                    fees.try_push(NamedFeeEntry {
                        name: name.clone(),
                        config,
                        beneficiary,
                    })
                    .map_err(|_| Error::<T>::TooManyFees)?;
                }
                Ok::<_, DispatchError>(())
            })?;
            Self::deposit_event(Event::ProtocolFeeSet { name });
            Ok(())
        }

        /// Remove a protocol-level fee. Requires `AdminOrigin`.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn remove_protocol_fee(
            origin: OriginFor<T>,
            name: FeeNameOf<T>,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            ProtocolFees::<T>::try_mutate(|fees| {
                let pos = fees
                    .iter()
                    .position(|e| e.name == name)
                    .ok_or(Error::<T>::FeeNotFound)?;
                fees.remove(pos);
                Ok::<_, DispatchError>(())
            })?;
            Self::deposit_event(Event::ProtocolFeeRemoved { name });
            Ok(())
        }

        /// Set or update a community-level fee. Requires `CommunityOrigin`.
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn set_community_fee(
            origin: OriginFor<T>,
            name: FeeNameOf<T>,
            config: FeeConfigOf<T>,
            beneficiary: T::AccountId,
        ) -> DispatchResult {
            let community = T::CommunityOrigin::ensure_origin(origin)?;
            CommunityFees::<T>::try_mutate(&community, |fees| {
                if let Some(entry) = fees.iter_mut().find(|e| e.name == name) {
                    entry.config = config;
                    entry.beneficiary = beneficiary;
                } else {
                    fees.try_push(NamedFeeEntry {
                        name: name.clone(),
                        config,
                        beneficiary,
                    })
                    .map_err(|_| Error::<T>::TooManyFees)?;
                }
                Ok::<_, DispatchError>(())
            })?;
            Self::deposit_event(Event::CommunityFeeSet { community, name });
            Ok(())
        }

        /// Remove a community-level fee. Requires `CommunityOrigin`.
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn remove_community_fee(
            origin: OriginFor<T>,
            name: FeeNameOf<T>,
        ) -> DispatchResult {
            let community = T::CommunityOrigin::ensure_origin(origin)?;
            CommunityFees::<T>::try_mutate(&community, |fees| {
                let pos = fees
                    .iter()
                    .position(|e| e.name == name)
                    .ok_or(Error::<T>::FeeNotFound)?;
                fees.remove(pos);
                Ok::<_, DispatchError>(())
            })?;
            Self::deposit_event(Event::CommunityFeeRemoved { community, name });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Calculate all applicable fees for a transfer by `who`.
        /// Returns a list of (beneficiary, fee_amount) pairs.
        pub fn calculate_fees(
            who: &T::AccountId,
            amount: BalanceOf<T>,
        ) -> Vec<(T::AccountId, BalanceOf<T>)> {
            let mut fees = Vec::new();

            // Protocol fees always apply
            for entry in ProtocolFees::<T>::get().iter() {
                let fee = entry.config.calculate(amount);
                if !fee.is_zero() {
                    fees.push((entry.beneficiary.clone(), fee));
                }
            }

            // Community fees apply if the sender belongs to a community
            if let Some(community) = T::CommunityDetector::community_of(who) {
                for entry in CommunityFees::<T>::get(&community).iter() {
                    let fee = entry.config.calculate(amount);
                    if !fee.is_zero() {
                        fees.push((entry.beneficiary.clone(), fee));
                    }
                }
            }

            fees
        }
    }
}
