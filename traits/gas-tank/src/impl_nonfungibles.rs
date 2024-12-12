use core::marker::PhantomData;

use frame_support::{
    pallet_prelude::{Decode, Encode},
    traits::nonfungibles_v2,
    weights::Weight,
};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{Bounded, CheckedAdd, CheckedSub};

use crate::*;

pub const ATTR_MEMBERSHIP_GAS: &[u8] = b"membership_gas";
pub const ATTR_GAS_TX_PAY_WITH_MEMBERSHIP: &[u8] = b"mbmshp_pays_gas";

#[derive(Encode, Decode, Debug)]
pub struct MembershipWeightTank<T: frame_system::Config> {
    pub since: BlockNumberFor<T>,
    pub used: Weight,
    pub period: Option<BlockNumberFor<T>>,
    pub max_per_period: Option<Weight>,
}

impl<T> Default for MembershipWeightTank<T>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Default,
{
    fn default() -> Self {
        Self {
            since: Default::default(),
            used: Default::default(),
            period: Default::default(),
            max_per_period: Default::default(),
        }
    }
}

pub struct NonFungibleGasBurner<T, F, I>(PhantomData<(T, F, I)>);

impl<T, F, ItemConfig> GasBurner for NonFungibleGasBurner<T, F, ItemConfig>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Bounded,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, ItemConfig>,
    ItemConfig: Default,
{
    type AccountId = T::AccountId;
    type Gas = Weight;

    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        F::owned(who).find_map(|(collection, item)| {
            let mut gas_tank: MembershipWeightTank<T> =
                F::typed_system_attribute(&collection, Some(&item), &ATTR_MEMBERSHIP_GAS)?;

            let block_number = frame_system::Pallet::<T>::block_number();
            let period = gas_tank.period.unwrap_or(BlockNumberFor::<T>::max_value());

            let Some(max_weight) = gas_tank.max_per_period else {
                return Some(Weight::MAX);
            };

            if block_number.checked_sub(&gas_tank.since)? > period {
                gas_tank.since = block_number.checked_add(&period)?;
                gas_tank.used = Weight::zero();

                F::set_typed_attribute(&collection, &item, &ATTR_MEMBERSHIP_GAS, &gas_tank).ok()?;
            };

            let remaining = max_weight.checked_sub(&gas_tank.used.checked_add(estimated)?)?;
            F::set_typed_attribute(
                &collection,
                &item,
                &ATTR_GAS_TX_PAY_WITH_MEMBERSHIP,
                &remaining,
            )
            .ok()?;

            Some(remaining)
        })
    }

    fn burn_gas(who: &Self::AccountId, expected: &Self::Gas, used: &Self::Gas) -> Self::Gas {
        F::owned(who)
            .find_map(|(collection, item)| {
                if !expected.eq(&F::typed_system_attribute(
                    &collection,
                    Some(&item),
                    &ATTR_GAS_TX_PAY_WITH_MEMBERSHIP,
                )?) {
                    return None;
                }
                F::clear_typed_attribute(&collection, &item, &ATTR_GAS_TX_PAY_WITH_MEMBERSHIP)
                    .ok()?;

                let mut gas_tank: MembershipWeightTank<T> =
                    F::typed_system_attribute(&collection, Some(&item), &ATTR_MEMBERSHIP_GAS)?;

                if gas_tank.max_per_period.is_some() {
                    gas_tank.used = gas_tank.used.checked_add(used)?;
                }

                F::set_typed_attribute(&collection, &item, &ATTR_MEMBERSHIP_GAS, &gas_tank).ok()?;
                let max_weight = gas_tank.max_per_period?;
                Some(max_weight.saturating_sub(gas_tank.used))
            })
            .unwrap_or_default()
    }
}

impl<T, F, ItemConfig> GasFueler for NonFungibleGasBurner<T, F, ItemConfig>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Bounded,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, ItemConfig>,
    ItemConfig: Default,
    F::CollectionId: 'static,
    F::ItemId: 'static,
{
    type TankId = (F::CollectionId, F::ItemId);
    type Gas = Weight;

    fn refuel_gas((collection_id, item_id): &Self::TankId, gas: &Self::Gas) -> Self::Gas {
        let Some(mut gas_tank): Option<MembershipWeightTank<T>> =
            F::typed_system_attribute(collection_id, Some(item_id), &ATTR_MEMBERSHIP_GAS)
        else {
            return Self::Gas::zero();
        };

        gas_tank.used = gas_tank.used.saturating_sub(*gas);

        // Should infallibly save the tank, given that it already got a tank
        let _ = F::set_typed_attribute(collection_id, item_id, &ATTR_MEMBERSHIP_GAS, &gas_tank);

        if gas_tank.max_per_period.is_some() {
            Weight::MAX
        } else {
            gas_tank
                .max_per_period
                .unwrap_or_default()
                .saturating_sub(gas_tank.used)
        }
    }
}

impl<T, F, ItemConfig> MakeTank for NonFungibleGasBurner<T, F, ItemConfig>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Bounded,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, ItemConfig>,
    ItemConfig: Default,
    F::CollectionId: 'static,
    F::ItemId: 'static,
{
    type TankId = (F::CollectionId, F::ItemId);
    type Gas = Weight;
    type BlockNumber = BlockNumberFor<T>;

    fn make_tank(
        (collection_id, item_id): &Self::TankId,
        capacity: Option<Self::Gas>,
        periodicity: Option<Self::BlockNumber>,
    ) -> Option<()> {
        let tank = MembershipWeightTank::<T> {
            since: frame_system::Pallet::<T>::block_number(),
            used: Weight::zero(),
            period: periodicity,
            max_per_period: capacity,
        };

        F::set_typed_attribute(&collection_id, item_id, &ATTR_MEMBERSHIP_GAS, &tank).ok()
    }
}
