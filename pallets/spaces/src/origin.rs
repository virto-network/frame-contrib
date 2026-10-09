//! A Space's own origin, as a community has one.

use crate::{Config, Spaces};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::marker::PhantomData;
use frame_support::traits::{EnsureOrigin, OriginTrait};
use scale_info::TypeInfo;

/// A Space speaking as itself. Produced by
/// [`dispatch_as_space`](crate::Pallet::dispatch_as_space).
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct RawOrigin<SpaceId> {
    space: SpaceId,
}

impl<SpaceId: Copy> RawOrigin<SpaceId> {
    /// The origin of `space`.
    pub const fn new(space: SpaceId) -> Self {
        RawOrigin { space }
    }

    /// The Space it speaks for.
    pub fn id(&self) -> SpaceId {
        self.space
    }
}

/// Accepts a Space's origin for a Space that exists, and gives its id: what another pallet uses to
/// let a Space act as itself.
pub struct EnsureSpace<T>(PhantomData<T>);

impl<T: Config> EnsureOrigin<T::RuntimeOrigin> for EnsureSpace<T>
where
    T::RuntimeOrigin: Into<Result<RawOrigin<T::SpaceId>, T::RuntimeOrigin>>,
{
    type Success = T::SpaceId;

    fn try_origin(o: T::RuntimeOrigin) -> Result<Self::Success, T::RuntimeOrigin> {
        use frame_system::RawOrigin::{None, Root};
        if matches!(o.as_system_ref(), Some(Root) | Some(None)) {
            return Err(o);
        }
        match o.clone().into() {
            Ok(origin) if Spaces::<T>::contains_key(origin.id()) => Ok(origin.id()),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<T::RuntimeOrigin, ()> {
        let space = crate::NextSpaceId::<T>::get()
            .or_else(<T::SpaceId as frame_support::traits::Incrementable>::initial_value)
            .ok_or(())?;
        Ok(RawOrigin::new(space).into())
    }
}
