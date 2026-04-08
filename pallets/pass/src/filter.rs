//! # Device Call Filters
//!
//! Per-device call authorization based on the principle of least privilege.
//! Each device is bound to a [`DeviceFilter`] that gates which extrinsics
//! it may dispatch. The first/recovery device gets `Admin` (unrestricted);
//! additional devices must have an explicit, more restrictive filter.
//!
//! ## Variants
//!
//! - [`DeviceFilter::Admin`] — unrestricted. Only the first device (from
//!   `register()`), and only one Admin device per account is expected.
//! - [`DeviceFilter::Pallets`] — whitelist by pallet index.
//! - [`DeviceFilter::Calls`] — whitelist by `(pallet_index, call_index)`.
//! - [`DeviceFilter::Spend`] — transfer listed assets up to per-asset limits.
//!   Non-spending calls are rejected.
//!
//! ## Delegation (no-escalation invariant)
//!
//! A device can only create new devices or session keys whose filter is a
//! subset of its own (see [`DeviceFilter::is_superset_of`]). The hierarchy:
//!
//! ```text
//! Admin  ──┬─► Pallets ──► Calls
//!          │
//!          └─► Spend
//! ```
//!
//! - `Admin` can delegate to anything
//! - `Pallets(S)` can delegate to `Pallets(S' ⊆ S)` or `Calls` within `S`
//! - `Calls(S)` can only delegate to `Calls(S' ⊆ S)`
//! - `Spend` can only delegate to `Spend` with lower/equal per-asset limits
//! - `Pallets`/`Calls` **cannot** delegate to `Spend` — this would let a
//!   device with access to e.g. `system.remark` spawn spend-capable devices.
//!   Spend filters must originate from an `Admin` device.
//!
//! ## Session keys
//!
//! Session keys are ephemeral and cannot use `Admin`. They still undergo the
//! no-escalation check against the device that created them.
//!
//! ## Spending inspection
//!
//! The runtime provides a [`SpendMatcher`] implementation that extracts
//! `(asset_id, amount)` from runtime calls. For runtimes that don't use
//! spend filters, `()` is a no-op implementation.

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
            // Pallets/Calls CANNOT delegate to Spend: a Calls device with
            // access to `system.remark` must not be able to spawn a Spend
            // device that can transfer arbitrary assets. Spend must come
            // directly from an Admin device.
            (Self::Pallets(_) | Self::Calls(_), Self::Spend(_)) => false,
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

/// Extracts spending information from a runtime call.
///
/// Runtimes implement this to support `Spend` device filters. If `Spend`
/// filters are not needed, use `()` which has no assets and rejects all
/// spending attempts.
pub trait SpendMatcher<Call> {
    /// The asset identifier used in `Spend` filter entries.
    type AssetId: Parameter + MaxEncodedLen + Ord + Copy;
    /// The balance type used for per-asset spend limits.
    type Balance: Parameter + MaxEncodedLen + Ord + Copy;

    /// If the call is a spending operation (transfer, payment, etc.),
    /// return `(asset_id, amount)`. Otherwise return `None`.
    fn spending_amount(call: &Call) -> Option<(Self::AssetId, Self::Balance)>;
}

impl<C> SpendMatcher<C> for () {
    type AssetId = ();
    type Balance = u128;

    fn spending_amount(_: &C) -> Option<((), u128)> {
        None
    }
}

/// Identifies a runtime call as a `(pallet_index, call_index)` pair for
/// filter matching.
///
/// The default implementation reads the first two bytes of the SCALE-encoded
/// call, which matches FRAME's current encoding. Runtimes with non-standard
/// encoding (e.g. XCM-wrapped calls) can provide a custom implementation.
pub trait CallMatcher<Call> {
    fn call_indices(call: &Call) -> (u8, u8);
}

/// Default matcher: reads `(pallet_index, call_index)` from the first two
/// bytes of the SCALE-encoded call. Suitable for standard FRAME runtimes.
pub struct ScaleCallMatcher;
impl<Call: Encode> CallMatcher<Call> for ScaleCallMatcher {
    fn call_indices(call: &Call) -> (u8, u8) {
        call.using_encoded(|bytes| {
            if bytes.len() >= 2 {
                (bytes[0], bytes[1])
            } else {
                (0, 0)
            }
        })
    }
}
