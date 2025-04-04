use alloc::boxed::Box;
use codec::alloc;
use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::{
    pallet_prelude::{Decode, Encode},
    traits::nonfungibles_v2,
    weights::Weight,
};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{Bounded, CheckedAdd, CheckedSub};

use crate::*;

pub use fc_traits_nonfungibles_helpers::SelectNonFungibleItem;

pub const ATTR_MEMBERSHIP_GAS: &[u8] = b"membership_gas";
pub const ATTR_GAS_TX_PAY_WITH_MEMBERSHIP: &[u8] = b"mbmshp_pays_gas";

#[derive(Encode, Decode, Debug)]
pub struct WeightTank<T: frame_system::Config> {
    pub(crate) since: BlockNumberFor<T>,
    pub(crate) used: Weight,
    pub(crate) period: Option<BlockNumberFor<T>>,
    pub(crate) capacity_per_period: Option<Weight>,
}

impl<T> WeightTank<T>
where
    T: frame_system::Config,
{
    fn new(capacity_per_period: Option<Weight>, period: Option<BlockNumberFor<T>>) -> Self {
        Self {
            since: frame_system::Pallet::<T>::block_number(),
            used: Weight::zero(),
            period,
            capacity_per_period,
        }
    }

    pub(crate) fn get<F>(collection_id: &F::CollectionId, item_id: &F::ItemId) -> Option<Self>
    where
        F: nonfungibles_v2::Inspect<T::AccountId>,
    {
        F::typed_system_attribute(collection_id, Some(item_id), &ATTR_MEMBERSHIP_GAS)
    }

    fn put<F, I>(&self, collection_id: &F::CollectionId, item_id: &F::ItemId) -> DispatchResult
    where
        F: nonfungibles_v2::Inspect<T::AccountId> + nonfungibles_v2::Mutate<T::AccountId, I>,
    {
        F::set_typed_attribute(collection_id, item_id, &ATTR_MEMBERSHIP_GAS, self)
    }
}

impl<T> Default for WeightTank<T>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Default,
{
    fn default() -> Self {
        Self {
            since: Default::default(),
            used: Default::default(),
            period: Default::default(),
            capacity_per_period: Default::default(),
        }
    }
}

pub struct Noop;
impl Get<Box<()>> for Noop {
    fn get() -> Box<()> {
        Box::new(())
    }
}

pub struct NonFungibleGasTank<T, F, I, S = Noop>(PhantomData<(T, F, I, S)>);

impl<T, F, I, S> GasBurner for NonFungibleGasTank<T, F, I, S>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Bounded,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, I>,
    I: Default,
    S: Get<Box<dyn SelectNonFungibleItem<F::CollectionId, F::ItemId>>>,
{
    type AccountId = T::AccountId;
    type Gas = Weight;

    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        F::owned(who).find_map(|(collection, item)| {
            if !S::get().select(collection.clone(), item.clone()) {
                return None;
            }

            let mut tank = WeightTank::<T>::get::<F>(&collection, &item)?;

            let block_number = frame_system::Pallet::<T>::block_number();
            let period = tank.period.unwrap_or(BlockNumberFor::<T>::max_value());

            let Some(capacity) = tank.capacity_per_period else {
                return Some(Weight::MAX);
            };

            if block_number.checked_sub(&tank.since)? > period {
                tank.since = block_number.checked_add(&period)?;
                tank.used = Weight::zero();
                tank.put::<F, I>(&collection, &item).ok()?;
            };

            let remaining = capacity.checked_sub(&tank.used.checked_add(estimated)?)?;
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

                let mut tank = WeightTank::<T>::get::<F>(&collection, &item)?;

                if tank.capacity_per_period.is_some() {
                    tank.used = tank.used.checked_add(used)?;
                }

                tank.put::<F, I>(&collection, &item).ok()?;

                let max_weight = tank.capacity_per_period?;
                Some(max_weight.saturating_sub(tank.used))
            })
            .unwrap_or_default()
    }
}

impl<T, F, ItemConfig, S> GasFueler for NonFungibleGasTank<T, F, ItemConfig, S>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Bounded,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, ItemConfig>,
    ItemConfig: Default,
    F::CollectionId: 'static,
    F::ItemId: 'static,
    S: Get<Box<dyn SelectNonFungibleItem<F::CollectionId, F::ItemId>>>,
{
    type TankId = (F::CollectionId, F::ItemId);
    type Gas = Weight;

    fn refuel_gas((collection_id, item_id): &Self::TankId, gas: &Self::Gas) -> Self::Gas {
        if !S::get().select(collection_id.clone(), item_id.clone()) {
            return Self::Gas::zero();
        }
        let Some(mut tank) = WeightTank::<T>::get::<F>(collection_id, item_id) else {
            return Self::Gas::zero();
        };

        if tank.capacity_per_period.is_none() {
            return Self::Gas::MAX;
        }

        tank.used = tank.used.saturating_sub(*gas);

        // Should infallibly save the tank, given that it already got a tank
        tank.put::<F, ItemConfig>(collection_id, item_id)
            .unwrap_or_default();

        tank.capacity_per_period
            .unwrap_or_default()
            .saturating_sub(tank.used)
    }
}

impl<T, F, ItemConfig, S> MakeTank for NonFungibleGasTank<T, F, ItemConfig, S>
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
    ) -> DispatchResult {
        WeightTank::<T>::new(capacity, periodicity).put::<F, ItemConfig>(collection_id, item_id)
    }
}
