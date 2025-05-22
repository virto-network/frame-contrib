use alloc::boxed::Box;
use codec::alloc;
use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::{
    pallet_prelude::{Decode, Encode},
    traits::nonfungibles_v2,
    weights::Weight,
};

use sp_runtime::traits::{BlockNumberProvider, Bounded, CheckedAdd, CheckedSub};

use crate::*;

pub use fc_traits_nonfungibles_helpers::SelectNonFungibleItem;

type BlockNumberFor<P> = <P as BlockNumberProvider>::BlockNumber;

pub const ATTR_MEMBERSHIP_GAS: &[u8] = b"membership_gas";
pub const ATTR_GAS_TX_PAY_WITH_MEMBERSHIP: &[u8] = b"mbmshp_pays_gas";

#[derive(Encode, Decode, Debug, Default)]
pub struct WeightTank<BlockNumber> {
    pub(crate) since: BlockNumber,
    pub(crate) used: Weight,
    pub(crate) period: Option<BlockNumber>,
    pub(crate) capacity_per_period: Option<Weight>,
}

impl<BlockNumber> WeightTank<BlockNumber> {
    fn new(
        capacity_per_period: Option<Weight>,
        since: BlockNumber,
        period: Option<BlockNumber>,
    ) -> Self {
        Self {
            since,
            used: Weight::zero(),
            period,
            capacity_per_period,
        }
    }

    pub(crate) fn get<T, F>(collection_id: &F::CollectionId, item_id: &F::ItemId) -> Option<Self>
    where
        T: frame_system::Config,
        F: nonfungibles_v2::Inspect<T::AccountId>,
        BlockNumber: Decode,
    {
        F::typed_system_attribute(collection_id, Some(item_id), &ATTR_MEMBERSHIP_GAS)
    }

    pub(crate) fn put<T, F, I>(
        &self,
        collection_id: &F::CollectionId,
        item_id: &F::ItemId,
    ) -> DispatchResult
    where
        T: frame_system::Config,
        F: nonfungibles_v2::Inspect<T::AccountId> + nonfungibles_v2::Mutate<T::AccountId, I>,
        BlockNumber: Encode,
    {
        F::set_typed_attribute(collection_id, item_id, &ATTR_MEMBERSHIP_GAS, self)
    }
}

pub struct Noop;
impl Get<Box<()>> for Noop {
    fn get() -> Box<()> {
        Box::new(())
    }
}

pub struct NonFungibleGasTank<T, P, F, I, S = Noop>(PhantomData<(T, P, F, I, S)>);

impl<T, P, F, I, S> GasBurner for NonFungibleGasTank<T, P, F, I, S>
where
    T: frame_system::Config,
    P: BlockNumberProvider,
    BlockNumberFor<P>: Bounded,
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

            let mut tank = WeightTank::<BlockNumberFor<P>>::get::<T, F>(&collection, &item)?;

            let block_number = P::current_block_number();
            let period = tank.period.unwrap_or(BlockNumberFor::<P>::max_value());

            let Some(capacity) = tank.capacity_per_period else {
                return Some(Weight::MAX);
            };

            if block_number.checked_sub(&tank.since)? > period {
                tank.since = block_number.checked_add(&period)?;
                tank.used = Weight::zero();
                tank.put::<T, F, I>(&collection, &item).ok()?;
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

                let mut tank = WeightTank::<BlockNumberFor<P>>::get::<T, F>(&collection, &item)?;

                if tank.capacity_per_period.is_some() {
                    tank.used = tank.used.checked_add(used)?;
                }

                tank.put::<T, F, I>(&collection, &item).ok()?;

                let max_weight = tank.capacity_per_period?;
                Some(max_weight.saturating_sub(tank.used))
            })
            .unwrap_or_default()
    }
}

impl<T, P, F, ItemConfig, S> GasFueler for NonFungibleGasTank<T, P, F, ItemConfig, S>
where
    T: frame_system::Config,
    P: BlockNumberProvider,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, ItemConfig>,
    ItemConfig: Default,
    BlockNumberFor<P>: Bounded,
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
        let Some(mut tank) = WeightTank::<BlockNumberFor<P>>::get::<T, F>(collection_id, item_id)
        else {
            return Self::Gas::zero();
        };

        if tank.capacity_per_period.is_none() {
            return Self::Gas::MAX;
        }

        tank.used = tank.used.saturating_sub(*gas);

        // Should infallibly save the tank, given that it already got a tank
        tank.put::<T, F, ItemConfig>(collection_id, item_id)
            .unwrap_or_default();

        tank.capacity_per_period
            .unwrap_or_default()
            .saturating_sub(tank.used)
    }
}

impl<T, P, F, ItemConfig, S> MakeTank for NonFungibleGasTank<T, P, F, ItemConfig, S>
where
    T: frame_system::Config,
    P: BlockNumberProvider,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, ItemConfig>,
    ItemConfig: Default,
    BlockNumberFor<P>: Bounded,
    F::CollectionId: 'static,
    F::ItemId: 'static,
{
    type TankId = (F::CollectionId, F::ItemId);
    type Gas = Weight;
    type BlockNumber = BlockNumberFor<P>;

    fn make_tank(
        (collection_id, item_id): &Self::TankId,
        capacity: Option<Self::Gas>,
        periodicity: Option<Self::BlockNumber>,
    ) -> DispatchResult {
        WeightTank::<Self::BlockNumber>::new(capacity, P::current_block_number(), periodicity)
            .put::<T, F, ItemConfig>(collection_id, item_id)
    }
}
