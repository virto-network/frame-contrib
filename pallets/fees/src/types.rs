use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame::deps::frame_support::traits::fungibles;
use scale_info::TypeInfo;
use sp_runtime::Permill;

use super::pallet::Config;

// Type aliases
pub type BalanceOf<T> = <<T as Config>::Assets as fungibles::Inspect<
    <T as frame::deps::frame_system::Config>::AccountId,
>>::Balance;
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

/// Inspects runtime calls to detect asset transfer operations.
/// Used by the transaction extension to charge fees on direct pallet-assets calls.
pub trait CallInspector<Call, AssetId, Balance> {
    /// If the call involves an asset transfer, returns `(asset_id, amount)`.
    fn extract_asset_transfer(call: &Call) -> Option<(AssetId, Balance)>;
}

impl<C, A, B> CallInspector<C, A, B> for () {
    fn extract_asset_transfer(_: &C) -> Option<(A, B)> {
        None
    }
}
