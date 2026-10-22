//! # Device Call Filters
//!
//! Per-device call authorization based on the principle of least privilege.
//! Each device is bound to a [`DeviceFilter`] that gates which extrinsics
//! it may dispatch. The first/recovery device gets `Admin` (unrestricted);
//! additional devices are expected to have an explicit, more restrictive filter,
//! but an `Admin` device may create other `Admin` devices.
//!
//! ## Variants
//!
//! - [`DeviceFilter::Admin`] — unrestricted. The first device (from
//!   `register()`) gets it. Devices created before per-device filters existed
//!   have it too (the migration gives it to them), and so do those created with
//!   the unfiltered `add_device`.
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
//! Session keys are ephemeral, so `add_session_key_with_filter` does not accept
//! `Admin`. The deprecated `add_session_key` is the only way to get an `Admin`
//! session key, while the runtime allows it (`Config::UnrestrictedSessionKeys`).
//! They still undergo the no-escalation check against the device that created
//! them.
//!
//! ## Wrapper calls
//!
//! A call that dispatches other calls with the origin it received (`utility.batch`,
//! `proxy.proxy`, ...) would let a restricted device run anything. `PassAuthenticate` therefore
//! asks the runtime's [`CallMatcher`] for the calls that each call wraps
//! ([`CallMatcher::nested_calls`]) and requires the filter to allow all of them, up to
//! `Config::MaxNestedCallDepth` levels and `Config::MaxNestedCalls` calls. A wrapper that can't
//! be inspected ([`NestedCalls::Opaque`]), or a call over those bounds, is rejected. `Admin` is
//! not inspected. See [`DeviceFilter::allows_call`].
//!
//! ## Spending inspection
//!
//! The runtime provides a [`SpendMatcher`] implementation that extracts
//! `(asset_id, amount)` from runtime calls. For runtimes that don't use
//! spend filters, `()` is a no-op implementation.

use alloc::{boxed::Box, vec::Vec};
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
pub struct AssetSpendLimit<AssetId, Balance> {
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
/// filter matching, and lists the calls that it wraps.
///
/// The default implementation reads the first two bytes of the SCALE-encoded
/// call, which matches FRAME's current encoding. Runtimes with non-standard
/// encoding (e.g. XCM-wrapped calls) can provide a custom implementation.
///
/// ## Wrapper calls
///
/// A *wrapper* is a call that dispatches other calls with (a derivative of) the origin it
/// received: `utility.batch`, `utility.as_derivative`, `proxy.proxy`, `multisig.as_multi`,
/// `scheduler.schedule`, a community's `dispatch_as_account`, and so on. A device filter that
/// only looked at the outer call would let such a wrapper carry any call the filter forbids.
///
/// So a runtime that lets a restricted device (or session key) use wrappers **must** implement
/// [`nested_calls`][CallMatcher::nested_calls] for every one of them. `PassAuthenticate` then
/// requires the filter to allow the wrapper itself **and every call it carries**, at any depth
/// (up to `Config::MaxNestedCallDepth` levels and `Config::MaxNestedCalls` calls).
///
/// The default `nested_calls` returns [`NestedCalls::None`] ("this call wraps nothing"), which
/// is what [`ScaleCallMatcher`] does: it knows nothing about the wrappers of a runtime. **A
/// runtime that uses `ScaleCallMatcher` and allows a wrapper in a restricted filter still lets
/// that device run anything through the wrapper.** Implement the trait for the runtime's call
/// instead, like this:
///
/// ```ignore
/// pub struct KreivoCallMatcher;
/// impl CallMatcher<RuntimeCall> for KreivoCallMatcher {
///     fn call_indices(call: &RuntimeCall) -> (u8, u8) {
///         ScaleCallMatcher::call_indices(call)
///     }
///
///     fn nested_calls(call: &RuntimeCall) -> NestedCalls<'_, RuntimeCall> {
///         match call {
///             RuntimeCall::Utility(
///                 pallet_utility::Call::batch { calls }
///                 | pallet_utility::Call::batch_all { calls }
///                 | pallet_utility::Call::force_batch { calls },
///             ) => NestedCalls::Many(calls),
///             RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. })
///             | RuntimeCall::Proxy(pallet_proxy::Call::proxy { call, .. })
///             | RuntimeCall::Scheduler(pallet_scheduler::Call::schedule { call, .. }) => {
///                 NestedCalls::One(call)
///             }
///             // Two separate calls
///             RuntimeCall::Utility(pallet_utility::Call::if_else { main, fallback }) => {
///                 NestedCalls::from_calls([&**main, &**fallback].into_iter())
///             }
///             // The call is only a hash (and its length): it can't be inspected.
///             RuntimeCall::Whitelist(
///                 pallet_whitelist::Call::dispatch_whitelisted_call { .. },
///             ) => NestedCalls::Opaque,
///             _ => NestedCalls::None,
///         }
///     }
/// }
/// ```
///
/// Be exhaustive: a wrapper that is not listed is a wrapper that is not checked.
pub trait CallMatcher<Call> {
    /// The `(pallet_index, call_index)` of `call`, **not** including the calls it wraps.
    fn call_indices(call: &Call) -> (u8, u8);

    /// The calls that `call` dispatches itself, **one level deep**: `PassAuthenticate` descends
    /// into each of them by calling this again, so a wrapper only reports its direct children.
    ///
    /// Defaults to [`NestedCalls::None`].
    fn nested_calls(_call: &Call) -> NestedCalls<'_, Call> {
        NestedCalls::None
    }
}

/// The calls wrapped by a call, as reported by [`CallMatcher::nested_calls`].
///
/// This is a lazy view over the call, not a collection: the checker pulls entries one at a
/// time and stops at the first one over the bounds, so a wrapper that carries a huge number of
/// calls costs a bounded amount of work. The common shapes (`Box<Call>`, `Vec<Call>`) borrow
/// the call without allocating; `Iter` covers any other layout (e.g. a call with two separate
/// `Box`es, like `utility.if_else`).
pub enum NestedCalls<'a, Call> {
    /// The call wraps nothing.
    None,
    /// The call wraps something that can't be inspected (for example, a scheduler call that
    /// carries the hash of a preimage instead of the call). It is **denied**.
    Opaque,
    /// A single wrapped call.
    One(&'a Call),
    /// A list of wrapped calls (a batch).
    Many(&'a [Call]),
    /// Any other layout.
    Iter(Box<dyn Iterator<Item = &'a Call> + 'a>),
}

impl<'a, Call> NestedCalls<'a, Call> {
    /// Wraps any iterator of calls.
    pub fn from_calls(calls: impl Iterator<Item = &'a Call> + 'a) -> Self {
        Self::Iter(Box::new(calls))
    }
}

/// The reason why a call tree was not accepted. All of them are rejected the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallTreeError {
    /// The filter does not allow one of the calls.
    Denied,
    /// A call is nested deeper than the allowed depth.
    TooDeep,
    /// The call carries more calls than allowed.
    TooMany,
    /// A wrapper whose calls can't be inspected ([`NestedCalls::Opaque`]).
    Opaque,
}

/// Iterates one level of [`NestedCalls`].
enum Children<'a, Call> {
    One(Option<&'a Call>),
    Many(core::slice::Iter<'a, Call>),
    Iter(Box<dyn Iterator<Item = &'a Call> + 'a>),
}

impl<'a, Call> Iterator for Children<'a, Call> {
    type Item = &'a Call;

    fn next(&mut self) -> Option<&'a Call> {
        match self {
            Self::One(call) => call.take(),
            Self::Many(calls) => calls.next(),
            Self::Iter(calls) => calls.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::One(call) => (call.is_some() as usize, Some(call.is_some() as usize)),
            Self::Many(calls) => calls.size_hint(),
            Self::Iter(calls) => calls.size_hint(),
        }
    }
}

/// Visits `call` and every call it wraps, at any depth, in pre-order, and stops at the first
/// one that `allowed` rejects.
///
/// - `call` is at depth 0, the calls it wraps are at depth 1, and so on. A call deeper than
///   `max_depth` is rejected ([`CallTreeError::TooDeep`]).
/// - Every call under `call` counts, wrappers included, and `call` does not.
///   More than `max_calls` is rejected ([`CallTreeError::TooMany`]).
/// - An [`Opaque`][NestedCalls::Opaque] wrapper is rejected ([`CallTreeError::Opaque`]).
///
/// The traversal keeps its own stack (at most `max_depth + 1` entries) instead of recursing,
/// so adversarially deep calls can't overflow the runtime's stack, and it never pulls more
/// than `max_calls + 1` calls from the matcher.
pub fn visit_call_tree<'a, Call, M: CallMatcher<Call>>(
    call: &'a Call,
    max_depth: u32,
    max_calls: u32,
    mut allowed: impl FnMut(&Call) -> bool,
) -> Result<(), CallTreeError> {
    if !allowed(call) {
        return Err(CallTreeError::Denied);
    }

    // One entry per level that is being iterated: the entry at `stack[d]` yields calls at
    // depth `d + 1`.
    let mut stack: Vec<Children<'a, Call>> = Vec::new();
    let mut seen = 0u32;

    // Pushes the children of `call`, if any. Rejects a wrapper that can't be inspected and
    // lists that are already over the bound without touching their elements.
    let descend = |call: &'a Call,
                   stack: &mut Vec<Children<'a, Call>>,
                   seen: u32|
     -> Result<(), CallTreeError> {
        let children = match M::nested_calls(call) {
            NestedCalls::None => return Ok(()),
            NestedCalls::Opaque => return Err(CallTreeError::Opaque),
            NestedCalls::One(call) => Children::One(Some(call)),
            NestedCalls::Many(calls) => Children::Many(calls.iter()),
            NestedCalls::Iter(calls) => Children::Iter(calls),
        };
        let (at_least, _) = children.size_hint();
        if (seen as usize).saturating_add(at_least) > max_calls as usize {
            return Err(CallTreeError::TooMany);
        }
        stack.push(children);
        Ok(())
    };

    descend(call, &mut stack, seen)?;
    while let Some(level) = stack.last_mut() {
        let Some(child) = level.next() else {
            stack.pop();
            continue;
        };
        // `child` is at depth `stack.len()`.
        if stack.len() > max_depth as usize {
            return Err(CallTreeError::TooDeep);
        }
        seen = seen.saturating_add(1);
        if seen > max_calls {
            return Err(CallTreeError::TooMany);
        }
        if !allowed(child) {
            return Err(CallTreeError::Denied);
        }
        descend(child, &mut stack, seen)?;
    }
    Ok(())
}

impl<
        AssetId: Ord + Clone + Debug + Eq,
        Balance: Ord + Clone + Debug + Eq,
        MaxCalls: Get<u32>,
        MaxAssets: Get<u32>,
    > DeviceFilter<AssetId, Balance, MaxCalls, MaxAssets>
{
    /// Check whether this filter allows `call` **and every call it wraps** (see
    /// [`CallMatcher::nested_calls`]). This is what a transaction needs to pass: the wrappers
    /// dispatch the calls they carry with the same signed origin.
    ///
    /// `Admin` is unrestricted, so its calls aren't inspected at all.
    ///
    /// Spending limits are checked **per call**, not per transaction: the amounts of the calls
    /// inside a wrapper are not added up. With a `SpendMatcher` that reports no spend for
    /// wrappers (as `()` and any matcher that only knows transfers), a `Spend` device can't use
    /// wrappers at all, which keeps its limit per transaction. A `SpendMatcher` that does report
    /// wrappers as spends turns the limit into a per-call one: a batch of `MaxNestedCalls`
    /// spends of the limit would pass.
    pub fn allows_call<Call, M, S>(
        &self,
        call: &Call,
        max_depth: u32,
        max_calls: u32,
    ) -> Result<(), CallTreeError>
    where
        M: CallMatcher<Call>,
        S: SpendMatcher<Call, AssetId = AssetId, Balance = Balance>,
    {
        if matches!(self, Self::Admin) {
            return Ok(());
        }
        visit_call_tree::<Call, M>(call, max_depth, max_calls, |call| {
            self.allows(M::call_indices(call), S::spending_amount(call))
        })
    }
}

/// Default matcher: reads `(pallet_index, call_index)` from the first two
/// bytes of the SCALE-encoded call. Suitable for standard FRAME runtimes.
///
/// It does not know any wrapper call: see [`CallMatcher`].
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use frame_support::traits::ConstU32;

    /// A call with every shape of wrapper that [`NestedCalls`] supports.
    #[derive(Clone, Debug)]
    enum MockCall {
        /// `(pallet, call)`, and the amount that it spends, if any.
        Leaf(u8, u8, Option<u128>),
        Batch(Vec<MockCall>),
        Proxy(Box<MockCall>),
        /// Two separate calls, like `utility.if_else`.
        IfElse(Box<MockCall>, Box<MockCall>),
        Opaque,
    }
    use MockCall::*;

    fn leaf(call: u8) -> MockCall {
        Leaf(1, call, None)
    }

    struct Matcher;
    impl CallMatcher<MockCall> for Matcher {
        fn call_indices(call: &MockCall) -> (u8, u8) {
            match call {
                Leaf(p, c, _) => (*p, *c),
                Batch(_) => (9, 0),
                Proxy(_) => (9, 1),
                IfElse(..) => (9, 2),
                Opaque => (9, 3),
            }
        }

        fn nested_calls(call: &MockCall) -> NestedCalls<'_, MockCall> {
            match call {
                Leaf(..) => NestedCalls::None,
                Batch(calls) => NestedCalls::Many(calls),
                Proxy(call) => NestedCalls::One(call),
                IfElse(a, b) => NestedCalls::from_calls([&**a, &**b].into_iter()),
                Opaque => NestedCalls::Opaque,
            }
        }
    }

    /// Spends what `Leaf` says, and nothing else.
    struct Spends;
    impl SpendMatcher<MockCall> for Spends {
        type AssetId = u32;
        type Balance = u128;

        fn spending_amount(call: &MockCall) -> Option<(u32, u128)> {
            match call {
                Leaf(_, _, Some(amount)) => Some((1, *amount)),
                _ => None,
            }
        }
    }

    type Filter = DeviceFilter<u32, u128, ConstU32<10>, ConstU32<5>>;

    fn calls(calls: &[(u8, u8)]) -> Filter {
        Filter::Calls(
            calls
                .iter()
                .copied()
                .collect::<alloc::collections::BTreeSet<_>>()
                .try_into()
                .unwrap(),
        )
    }

    fn spend(max: u128) -> Filter {
        Filter::Spend(
            vec![AssetSpendLimit {
                asset: 1,
                max_amount: max,
            }]
            .try_into()
            .unwrap(),
        )
    }

    fn check(filter: &Filter, call: &MockCall, depth: u32, max: u32) -> Result<(), CallTreeError> {
        filter.allows_call::<_, Matcher, Spends>(call, depth, max)
    }

    #[test]
    fn a_call_without_wrapped_calls_is_checked_as_before() {
        let filter = calls(&[(1, 1)]);
        assert_eq!(check(&filter, &leaf(1), 0, 0), Ok(()));
        assert_eq!(check(&filter, &leaf(2), 0, 0), Err(CallTreeError::Denied));
    }

    #[test]
    fn every_shape_of_wrapper_is_inspected() {
        let filter = calls(&[(1, 1), (9, 0), (9, 1), (9, 2)]);
        for bad in [
            Batch(vec![leaf(1), leaf(2)]),
            Proxy(Box::new(leaf(2))),
            IfElse(Box::new(leaf(1)), Box::new(leaf(2))),
            IfElse(Box::new(leaf(2)), Box::new(leaf(1))),
        ] {
            assert_eq!(
                check(&filter, &bad, 4, 64),
                Err(CallTreeError::Denied),
                "{bad:?}"
            );
        }
        for good in [
            Batch(vec![leaf(1), leaf(1)]),
            Proxy(Box::new(leaf(1))),
            IfElse(Box::new(leaf(1)), Box::new(Proxy(Box::new(leaf(1))))),
        ] {
            assert_eq!(check(&filter, &good, 4, 64), Ok(()), "{good:?}");
        }
        // The wrapper must be allowed as well
        assert_eq!(
            check(&calls(&[(1, 1)]), &Batch(vec![leaf(1)]), 4, 64),
            Err(CallTreeError::Denied)
        );
    }

    #[test]
    fn the_bounds_are_inclusive() {
        let filter = calls(&[(1, 1), (9, 0), (9, 1)]);
        let proxies = |n| (0..n).fold(leaf(1), |c, _| Proxy(Box::new(c)));
        assert_eq!(check(&filter, &proxies(3), 3, 64), Ok(()));
        assert_eq!(
            check(&filter, &proxies(4), 3, 64),
            Err(CallTreeError::TooDeep)
        );

        let batch = |n| Batch(vec![leaf(1); n]);
        assert_eq!(check(&filter, &batch(3), 4, 3), Ok(()));
        assert_eq!(check(&filter, &batch(4), 4, 3), Err(CallTreeError::TooMany));
        // wrappers count, the outer call doesn't
        assert_eq!(check(&filter, &Proxy(Box::new(batch(2))), 4, 3), Ok(()));
        assert_eq!(
            check(&filter, &Proxy(Box::new(batch(3))), 4, 3),
            Err(CallTreeError::TooMany)
        );
        // nothing nests under a zero depth
        assert_eq!(check(&filter, &leaf(1), 0, 0), Ok(()));
        assert_eq!(
            check(&filter, &batch(1), 0, 64),
            Err(CallTreeError::TooDeep)
        );
        assert_eq!(check(&filter, &batch(0), 0, 64), Ok(()));
    }

    #[test]
    fn opaque_is_denied_at_any_level() {
        let filter = calls(&[(1, 1), (9, 0), (9, 1), (9, 3)]);
        assert_eq!(check(&filter, &Opaque, 4, 64), Err(CallTreeError::Opaque));
        assert_eq!(
            check(
                &filter,
                &Batch(vec![leaf(1), Proxy(Box::new(Opaque))]),
                4,
                64
            ),
            Err(CallTreeError::Opaque)
        );
    }

    #[test]
    fn admin_is_not_inspected() {
        let call = Batch(vec![Opaque; 1_000]);
        assert_eq!(check(&Filter::Admin, &call, 0, 0), Ok(()));
    }

    #[test]
    fn a_pallet_filter_checks_every_call() {
        let filter = Filter::Pallets(
            [9u8]
                .into_iter()
                .collect::<alloc::collections::BTreeSet<_>>()
                .try_into()
                .unwrap(),
        );
        let nested = |inner| Batch(vec![Proxy(Box::new(inner))]);
        assert_eq!(check(&filter, &nested(Batch(vec![])), 4, 64), Ok(()));
        assert_eq!(
            check(&filter, &nested(leaf(1)), 4, 64),
            Err(CallTreeError::Denied)
        );
    }

    #[test]
    fn spend_limits_apply_to_the_nested_calls() {
        let spending = |amount| Leaf(1, 1, Some(amount));
        let filter = spend(100);
        assert_eq!(check(&filter, &spending(100), 4, 64), Ok(()));
        assert_eq!(
            check(&filter, &spending(101), 4, 64),
            Err(CallTreeError::Denied)
        );
        // A wrapper doesn't spend, so a `Spend` device can't use it
        assert_eq!(
            check(&filter, &Batch(vec![spending(1)]), 4, 64),
            Err(CallTreeError::Denied)
        );
        assert_eq!(
            check(&filter, &Proxy(Box::new(spending(1))), 4, 64),
            Err(CallTreeError::Denied)
        );
    }

    #[test]
    fn a_matcher_that_reports_wrappers_as_spends_makes_the_limit_per_call() {
        // Pins down what such a matcher means (see `allows_call`): each nested spend is
        // checked against the limit on its own, and they are not added up.
        struct WrappersSpendNothing;
        impl SpendMatcher<MockCall> for WrappersSpendNothing {
            type AssetId = u32;
            type Balance = u128;

            fn spending_amount(call: &MockCall) -> Option<(u32, u128)> {
                match call {
                    Leaf(_, _, amount) => amount.map(|a| (1, a)),
                    _ => Some((1, 0)),
                }
            }
        }
        let check = |call| spend(100).allows_call::<_, Matcher, WrappersSpendNothing>(&call, 4, 64);
        assert_eq!(check(Batch(vec![Leaf(1, 1, Some(100))])), Ok(()));
        assert_eq!(
            check(Batch(vec![
                Leaf(1, 1, Some(100)),
                Proxy(Box::new(Leaf(1, 1, Some(101))))
            ])),
            Err(CallTreeError::Denied)
        );
        // Not added up: three spends of the limit in one transaction.
        assert_eq!(check(Batch(vec![Leaf(1, 1, Some(100)); 3])), Ok(()));
    }

    #[test]
    fn deep_nesting_is_rejected_without_recursing() {
        let filter = calls(&[(1, 1), (9, 1)]);
        // Built in a loop and leaked: encoding, cloning and dropping it all recurse.
        let mut call = leaf(1);
        for _ in 0..200_000 {
            call = Proxy(Box::new(call));
        }
        assert_eq!(check(&filter, &call, 4, 64), Err(CallTreeError::TooDeep));
        // even with a bound as deep as the call: the stack of the traversal is on the heap
        assert_eq!(check(&filter, &call, u32::MAX, u32::MAX), Ok(()));
        core::mem::forget(call);
    }

    #[test]
    fn a_huge_batch_is_rejected_without_visiting_it() {
        let filter = calls(&[(1, 1), (9, 0)]);
        // The slice says how long it is: no element is checked.
        let visited = Cell::new(0u32);
        let call = Batch(vec![leaf(1); 100_000]);
        let result = visit_call_tree::<_, Matcher>(&call, 4, 64, |_| {
            visited.set(visited.get() + 1);
            true
        });
        assert_eq!(result, Err(CallTreeError::TooMany));
        assert_eq!(visited.get(), 1, "only the outer call");
        assert_eq!(check(&filter, &call, 4, 64), Err(CallTreeError::TooMany));
    }

    #[test]
    fn an_iterator_is_not_pulled_beyond_the_bound() {
        thread_local!(static PULLED: Cell<u32> = const { Cell::new(0) });
        struct Endless;
        impl CallMatcher<Endless> for Endless {
            fn call_indices(_: &Endless) -> (u8, u8) {
                (0, 0)
            }
            fn nested_calls(call: &Endless) -> NestedCalls<'_, Endless> {
                // A wrapper that never ends and doesn't say so
                struct Forever<'a>(&'a Endless);
                impl<'a> Iterator for Forever<'a> {
                    type Item = &'a Endless;
                    fn next(&mut self) -> Option<&'a Endless> {
                        PULLED.with(|p| p.set(p.get() + 1));
                        Some(self.0)
                    }
                }
                NestedCalls::from_calls(Forever(call))
            }
        }
        let call = Endless;
        let result = visit_call_tree::<_, Endless>(&call, u32::MAX, 64, |_| true);
        assert_eq!(result, Err(CallTreeError::TooMany));
        assert_eq!(PULLED.with(Cell::get), 65);
    }
}
