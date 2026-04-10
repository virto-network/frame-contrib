use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::Permill;

use super::pallet::Config;

// Type aliases — derived directly from pallet_assets::Config
pub type BalanceOf<T> = <T as pallet_assets::Config>::Balance;
pub type AssetIdOf<T> = <T as pallet_assets::Config>::AssetId;
pub type FeeNameOf<T> = sp_runtime::BoundedVec<u8, <T as Config>::MaxFeeNameLen>;
pub type FeeConfigOf<T> = FeeConfig<BalanceOf<T>>;
pub type NamedFeeEntryOf<T> = NamedFeeEntry<T>;

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
#[derive(
    Clone, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug, PartialEq, Eq,
)]
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
    /// Returns `true` if the fee configuration is valid.
    pub fn is_valid(&self) -> bool {
        if let FeeConfig::PercentageClamped { min, max, .. } = self {
            return *min <= *max;
        }
        true
    }

    /// Calculate the fee for a given transfer amount.
    /// If `min_balance` is provided and the raw fee is non-zero but below it,
    /// the fee is rounded up to `min_balance` to avoid dust.
    pub fn calculate(&self, amount: Balance, min_balance: Option<Balance>) -> Balance {
        let raw = match self {
            FeeConfig::Fixed(fee) => *fee,
            FeeConfig::Percentage(rate) => rate.mul_ceil(amount),
            FeeConfig::PercentageClamped { rate, min, max } => {
                let fee = rate.mul_ceil(amount);
                if fee < *min {
                    *min
                } else if fee > *max {
                    *max
                } else {
                    fee
                }
            }
        };
        // Round up to min_balance if the fee is non-zero but below it
        match min_balance {
            Some(mb) if !raw.is_zero() && raw < mb => mb,
            _ => raw,
        }
    }
}

/// A named fee entry stored on-chain.
#[derive(
    Clone, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug, PartialEq, Eq,
)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound(T: Config))]
pub struct NamedFeeEntry<T: Config> {
    pub name: FeeNameOf<T>,
    pub config: FeeConfigOf<T>,
    pub beneficiary: T::AccountId,
}
