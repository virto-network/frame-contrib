use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame::deps::frame_support::traits::fungibles;
use scale_info::TypeInfo;
use sp_runtime::Permill;

use super::pallet::Config;

// Type aliases
pub type BalanceOf<T> = <T as Config>::Balance;
pub type FeeNameOf<T> = sp_runtime::BoundedVec<u8, <T as Config>::MaxFeeNameLen>;
pub type FeeConfigOf<T> = FeeConfig<BalanceOf<T>>;
pub type NamedFeeEntryOf<T> = NamedFeeEntry<T>;
pub type AssetIdOf<T> = <<T as Config>::Assets as fungibles::Inspect<
    <T as frame::deps::frame_system::Config>::AccountId,
>>::AssetId;

/// Maps an account to the community it belongs to.
pub trait AccountCommunity<AccountId, CommunityId> {
    fn community_of(who: &AccountId) -> Option<CommunityId>;
}

impl<A, C> AccountCommunity<A, C> for () {
    fn community_of(_: &A) -> Option<C> {
        None
    }
}

/// Describes how a fee amount is calculated from a transfer amount.
#[derive(Clone, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug, PartialEq, Eq)]
pub enum FeeConfig<Balance> {
    /// A fixed fee amount regardless of transfer size.
    Fixed(Balance),
    /// A percentage of the transfer amount (parts per million).
    Percentage(Permill),
    /// A percentage with minimum and maximum bounds.
    PercentageClamped {
        rate: Permill,
        min: Balance,
        max: Balance,
    },
}

impl<Balance: sp_runtime::traits::AtLeast32BitUnsigned + Copy> FeeConfig<Balance> {
    pub fn calculate(&self, amount: Balance) -> Balance {
        match self {
            FeeConfig::Fixed(fee) => *fee,
            FeeConfig::Percentage(rate) => rate.mul_floor(amount),
            FeeConfig::PercentageClamped { rate, min, max } => {
                let fee = rate.mul_floor(amount);
                if fee < *min {
                    *min
                } else if fee > *max {
                    *max
                } else {
                    fee
                }
            }
        }
    }
}

/// A named fee entry stored on-chain.
#[derive(Clone, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug, PartialEq, Eq)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound(T: Config))]
pub struct NamedFeeEntry<T: Config> {
    pub name: FeeNameOf<T>,
    pub config: FeeConfigOf<T>,
    pub beneficiary: T::AccountId,
}
