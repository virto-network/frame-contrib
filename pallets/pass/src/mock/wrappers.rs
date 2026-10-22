//! A mock pallet with wrapper calls, and the [`CallMatcher`] that knows them.
//!
//! The calls dispatch what they carry with the origin they receive, as `utility.batch` or
//! `proxy.proxy` do. [`MockCallMatcher`] is also the reference for runtimes: it matches on
//! its wrapper variants to report the calls that they carry.

use crate::{CallMatcher, NestedCalls, ScaleCallMatcher};
pub use pallet::*;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
    use alloc::{boxed::Box, vec::Vec};
    use frame_support::{
        dispatch::{GetDispatchInfo, PostDispatchInfo},
        pallet_prelude::*,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Dispatchable;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
            + GetDispatchInfo
            + From<Call<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Dispatches every call, in order, with the same origin.
        pub fn batch(
            origin: OriginFor<T>,
            calls: Vec<<T as Config>::RuntimeCall>,
        ) -> DispatchResult {
            for call in calls {
                call.dispatch(origin.clone()).map_err(|e| e.error)?;
            }
            Ok(())
        }

        /// Dispatches the call with the same origin.
        pub fn proxy(
            origin: OriginFor<T>,
            call: Box<<T as Config>::RuntimeCall>,
        ) -> DispatchResult {
            call.dispatch(origin).map(|_| ()).map_err(|e| e.error)
        }

        /// Stands for a call that can't be inspected, like a scheduler call that only carries
        /// the hash of its preimage.
        pub fn opaque(_origin: OriginFor<T>, _hash: [u8; 32]) -> DispatchResult {
            Ok(())
        }
    }
}

/// [`ScaleCallMatcher`] for the indices, and the wrapper variants of the mock for the nested
/// calls.
pub struct MockCallMatcher;
impl CallMatcher<super::RuntimeCall> for MockCallMatcher {
    fn call_indices(call: &super::RuntimeCall) -> (u8, u8) {
        ScaleCallMatcher::call_indices(call)
    }

    fn nested_calls(call: &super::RuntimeCall) -> NestedCalls<'_, super::RuntimeCall> {
        use super::{NestedCallsAreReported, RuntimeCall};
        if !NestedCallsAreReported::get() {
            return NestedCalls::None;
        }
        match call {
            RuntimeCall::Wrapper(Call::batch { calls }) => NestedCalls::Many(calls),
            RuntimeCall::Wrapper(Call::proxy { call }) => NestedCalls::One(call),
            RuntimeCall::Wrapper(Call::opaque { .. }) => NestedCalls::Opaque,
            _ => NestedCalls::None,
        }
    }
}
