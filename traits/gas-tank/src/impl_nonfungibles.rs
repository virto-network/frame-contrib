use core::marker::PhantomData;

use frame_support::{
    pallet_prelude::{Decode, Encode},
    traits::{nonfungibles_v2, ConstU32, Get},
    weights::Weight,
    BoundedBTreeMap,
};

use crate::*;

pub const ATTR_MEMBER_GAS_SIZE: &[u8] = b"membership_gas_size";
pub type GasSizeConfigMap = BoundedBTreeMap<GasTankSize, Weight, ConstU32<3>>;

#[derive(Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum GasTankSize {
    Small,
    Medium,
    Large,
}

pub struct NonFungibleGasBurner<
    T: frame_system::Config,
    S: Get<GasSizeConfigMap>,
    F: nonfungibles_v2::Inspect<T::AccountId> + nonfungibles_v2::InspectEnumerable<T::AccountId>,
>(PhantomData<(S, T, F)>);

impl<T, S, F> GasBurner for NonFungibleGasBurner<T, S, F>
where
    T: frame_system::Config,
    S: Get<GasSizeConfigMap>,
    F: nonfungibles_v2::Inspect<T::AccountId> + nonfungibles_v2::InspectEnumerable<T::AccountId>,
    F::CollectionId: Into<u64> + TryFrom<u64>,
    <F::CollectionId as TryFrom<u64>>::Error: core::fmt::Debug,
    F::ItemId: Into<u64> + TryFrom<u64>,
    <F::ItemId as TryFrom<u64>>::Error: core::fmt::Debug,
{
    type AccountId = T::AccountId;
    type Gas = Weight;

    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        F::owned(who)
            .find(|(collection, item)| {
                let Some(gas_size) =
                    F::typed_system_attribute(collection, Some(item), &ATTR_MEMBER_GAS_SIZE)
                else {
                    return false;
                };
                let weight_configs = S::get().into_inner();
                let Some(max_weight) = weight_configs.get(&gas_size) else {
                    return false;
                };

                max_weight.checked_sub(estimated).is_some()
            })
            .map(|(collection, item)| {
                // Note: This is a hacky trick to store the item ID into the expected leftover,
                // which is returned back in `burn_gas` as `expected`
                Weight::from_parts(collection.into(), item.into())
            })
    }

    fn burn_gas(_: &Self::AccountId, _: &Self::Gas, _: &Self::Gas) -> Self::Gas {
        // TODO: Implement this. This naive implementation won't burn gas. It
        // will just check the gas tank from the item has enough size to cover
        // the call.
        Weight::from_parts(1, 1)
    }
}
