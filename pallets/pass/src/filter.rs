use core::fmt::Debug;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, CloneNoBound, DebugNoBound, EqNoBound, PartialEqNoBound};
use scale_info::TypeInfo;
use sp_runtime::BoundedBTreeSet;

/// Compact per-device call filter.
///
/// Determines what a device is authorized to do. Follows the principle of
/// least privilege: the default should be restrictive, and only the first
/// admin device gets unrestricted access.
#[derive(
    CloneNoBound,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    DebugNoBound,
    PartialEqNoBound,
    EqNoBound,
)]
#[scale_info(skip_type_params(MaxCalls, MaxAssets))]
#[codec(mel_bound(AssetId: MaxEncodedLen, Balance: MaxEncodedLen))]
pub enum DeviceFilter<
    AssetId: Ord + Clone + Debug + Eq,
    Balance: Clone + Debug + Eq,
    MaxCalls: Get<u32>,
    MaxAssets: Get<u32>,
> {
    /// Unrestricted access. Reserved for the first/admin/recovery device.
    Admin,
    /// Can call anything in the listed pallets (by pallet index).
    Pallets(BoundedBTreeSet<u8, MaxCalls>),
    /// Can call only the listed (pallet_index, call_index) pairs.
    Calls(BoundedBTreeSet<(u8, u8), MaxCalls>),
    /// Spend-only: can transfer listed assets, each up to a per-tx limit.
    /// Non-spending calls are rejected.
    Spend(BoundedVec<AssetSpendLimit<AssetId, Balance>, MaxAssets>),
}

/// A single asset spend limit.
#[derive(
    Clone, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug, PartialEq, Eq,
)]
pub struct AssetSpendLimit<AssetId: Clone + Debug + Eq, Balance: Clone + Debug + Eq> {
    pub asset: AssetId,
    pub max_amount: Balance,
}

impl<A: Ord + Clone + Debug + Eq, B: Clone + Debug + Eq, C: Get<u32>, D: Get<u32>> Default
    for DeviceFilter<A, B, C, D>
{
    fn default() -> Self {
        Self::Admin
    }
}

impl<
        AssetId: Ord + Clone + Debug + Eq,
        Balance: Ord + Clone + Debug + Eq,
        MaxCalls: Get<u32>,
        MaxAssets: Get<u32>,
    > DeviceFilter<AssetId, Balance, MaxCalls, MaxAssets>
{
    /// Check whether `self` is at least as permissive as `other`.
    /// Used for the no-escalation invariant: a device can only grant
    /// permissions it already has.
    pub fn is_superset_of(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Admin, _) => true,
            (_, Self::Admin) => false,
            (Self::Pallets(a), Self::Pallets(b)) => b.iter().all(|p| a.contains(p)),
            (Self::Pallets(a), Self::Calls(b)) => b.iter().all(|(p, _)| a.contains(p)),
            (Self::Calls(a), Self::Calls(b)) => b.iter().all(|c| a.contains(c)),
            (Self::Spend(a), Self::Spend(b)) => b.iter().all(|bl| {
                a.iter()
                    .any(|al| al.asset == bl.asset && al.max_amount >= bl.max_amount)
            }),
            // More permissive scopes can delegate down to Spend
            (Self::Pallets(_) | Self::Calls(_), Self::Spend(_)) => true,
            // Spend can't grant call/pallet access
            (Self::Spend(_), _) => false,
            // Calls can't grant pallet-wide access
            (Self::Calls(_), Self::Pallets(_)) => false,
        }
    }

    /// Check if a call is allowed by this filter.
    ///
    /// `call_index` is `(pallet_index, call_index)` — the first two bytes
    /// of the SCALE-encoded `RuntimeCall`.
    ///
    /// `spend_amount` is provided by the runtime's `SpendMatcher` and returns
    /// `Some((asset, amount))` for spending calls.
    pub fn allows(&self, call_index: (u8, u8), spend_amount: Option<(AssetId, Balance)>) -> bool {
        match self {
            Self::Admin => true,
            Self::Pallets(set) => set.contains(&call_index.0),
            Self::Calls(set) => set.contains(&call_index),
            Self::Spend(limits) => {
                let Some((asset, amount)) = spend_amount else {
                    return false;
                };
                limits
                    .iter()
                    .any(|l| l.asset == asset && amount <= l.max_amount)
            }
        }
    }
}

/// Trait to extract spending information from a runtime call.
/// Implemented by the runtime to support `Spend` device filters.
pub trait SpendMatcher<Call, AssetId, Balance> {
    /// If the call is a spending operation (transfer, payment, etc.),
    /// returns the `(asset_id, amount)`. Otherwise returns `None`.
    fn spending_amount(call: &Call) -> Option<(AssetId, Balance)>;
}

impl<C, A, B> SpendMatcher<C, A, B> for () {
    fn spending_amount(_: &C) -> Option<(A, B)> {
        None
    }
}
