use alloc::{boxed::Box, vec::Vec};
use codec::alloc;
use core::marker::PhantomData;
use frame_support::traits::{ConstU32, Get};
use frame_support::{
    pallet_prelude::{Decode, Encode},
    storage::{storage_prefix, unhashed},
    traits::nonfungibles_v2,
    weights::Weight,
    Blake2_128Concat, StorageHasher,
};

use sp_runtime::traits::{AtLeast32BitUnsigned, BlockNumberProvider, Bounded};

use crate::*;

pub use fc_traits_nonfungibles_helpers::SelectNonFungibleItem;

type BlockNumberFor<P> = <P as BlockNumberProvider>::BlockNumber;

pub const ATTR_MEMBERSHIP_GAS: &[u8] = b"membership_gas";

/// How many of an account's items [`NonFungibleGasTank`] reads, at most, to find a tank, unless the
/// runtime sets its own bound.
///
/// Every signed transaction pays for this scan (twice: in the check and in preparation), so the
/// benchmark of the payment step must set up an account with this many items.
pub type DefaultMaxScan = ConstU32<4>;

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

impl<BlockNumber: AtLeast32BitUnsigned + Copy> WeightTank<BlockNumber> {
    /// The start of the usage window that contains `now`, computed from the stored window start
    /// (`since`) and the period, without writing anything.
    ///
    /// - With no period, the window never ends: it is `since`.
    /// - With `now` before `since` (a start written into the future, as the reset fixed by `DEC-23`
    ///   did), the stored window stays current, and once the clock reaches it, it lasts a whole
    ///   period: its usage counts until `since + period`. A tank the old reset left behind gets one
    ///   stretched window, and never more allowance.
    /// - Otherwise it is `since + ⌊(now − since) / period⌋ · period`, so a window ends exactly when
    ///   `now − since ≥ period`. A zero period makes every tick its own window.
    pub(crate) fn window_start(&self, now: BlockNumber) -> BlockNumber {
        let Some(period) = self.period else {
            return self.since;
        };
        if now < self.since {
            return self.since;
        }
        match now.saturating_sub(self.since).checked_div(&period) {
            Some(windows) => self.since.saturating_add(windows.saturating_mul(period)),
            None => now,
        }
    }

    /// What the window that contains `now` has used: the stored usage if that window is the stored
    /// one, and nothing otherwise.
    pub(crate) fn used_at(&self, now: BlockNumber) -> Weight {
        if self.window_start(now) == self.since {
            self.used
        } else {
            Weight::zero()
        }
    }

    /// What would be left in the window that contains `now` after using `estimated`, or `None` if the
    /// tank cannot cover it in either component. An unlimited tank always has [`Weight::MAX`] left.
    pub(crate) fn remaining_at(&self, now: BlockNumber, estimated: &Weight) -> Option<Weight> {
        let Some(capacity) = self.capacity_per_period else {
            return Some(Weight::MAX);
        };
        capacity.checked_sub(&self.used_at(now).checked_add(estimated)?)
    }

    /// Moves the stored window to the one that contains `now`, resetting the usage if it moved.
    pub(crate) fn roll(&mut self, now: BlockNumber) {
        let start = self.window_start(now);
        if start != self.since {
            self.since = start;
            self.used = Weight::zero();
        }
    }
}

/// The paying-item note: the item whose tank pays for the transaction, and what
/// [`GasBurner::prepare_gas`] returned.
type PayingItem<T, F> = (
    <F as nonfungibles_v2::Inspect<<T as frame_system::Config>::AccountId>>::CollectionId,
    <F as nonfungibles_v2::Inspect<<T as frame_system::Config>::AccountId>>::ItemId,
    Weight,
);

/// Where [`NonFungibleGasTank`] keeps `who`'s paying-item note between preparation and the burn:
/// `twox_128(b"NonFungibleGasTank") ++ twox_128(b"PayingItem") ++ blake2_128_concat(who.encode())`.
pub(crate) fn paying_item_key<AccountId: Encode>(who: &AccountId) -> Vec<u8> {
    let mut key = storage_prefix(b"NonFungibleGasTank", b"PayingItem").to_vec();
    key.extend(Blake2_128Concat::hash(&who.encode()));
    key
}

pub struct Noop;
impl Get<Box<()>> for Noop {
    fn get() -> Box<()> {
        Box::new(())
    }
}

/// A [`GasBurner`], [`GasFueler`] and [`MakeTank`] that keeps a periodic weight tank on a
/// non-fungible item (for example, a membership), in the item's `membership_gas` system attribute.
///
/// - `T`: the runtime. `P`: the clock the tank's periods are counted in.
/// - `F`: the non-fungibles implementation (for example, a `pallet_nfts` instance), `I` its item
///   config.
/// - `S`: which items may hold a tank.
/// - `MaxScan`: how many of an account's items, at most, are read to find a tank (default
///   [`DefaultMaxScan`]). Items past the bound are never considered, so the account pays fees.
///
/// The check reads only. The usage window is computed from the stored start, never reset by a
/// write. [`GasBurner::prepare_gas`] writes the **paying-item note**: which `(collection, item)` pays
/// for the transaction, with what `prepare_gas` returned, under a key of the account (see below).
/// [`GasBurner::burn_gas`] takes the note, rolls that item's stored window and adds the usage, with no
/// scan, so nothing the call does to the account's items during dispatch hides the tank from it.
/// [`GasBurner::cancel_gas`] takes the note and charges nothing. Either way the note does not outlive
/// the transaction.
///
/// The note's key, in unhashed storage, is `twox_128(b"NonFungibleGasTank") ++
/// twox_128(b"PayingItem") ++ blake2_128_concat(who.encode())`, and its value is
/// `(collection, item, remaining)`, SCALE-encoded. A runtime has one such key per account, shared by
/// every `NonFungibleGasTank` it defines, so a runtime may have at most one payment step backed by a
/// `NonFungibleGasTank`. A runtime test can hold it to that: after applying a transaction a tank
/// pays for, no key is left under `twox_128(b"NonFungibleGasTank") ++ twox_128(b"PayingItem")`.
///
/// The tank that admitted a transaction pays for it. If the call moves the noted item to another
/// account during dispatch, its tank is still charged, now in the new owner's hands. If the item's
/// tank is gone by then, nothing is charged.
pub struct NonFungibleGasTank<T, P, F, I, S = Noop, MaxScan = DefaultMaxScan>(
    PhantomData<(T, P, F, I, S, MaxScan)>,
);

impl<T, P, F, I, S, MaxScan> NonFungibleGasTank<T, P, F, I, S, MaxScan>
where
    T: frame_system::Config,
    P: BlockNumberProvider,
    F: nonfungibles_v2::Inspect<T::AccountId> + nonfungibles_v2::InspectEnumerable<T::AccountId>,
    S: Get<Box<dyn SelectNonFungibleItem<F::CollectionId, F::ItemId>>>,
    MaxScan: Get<u32>,
{
    /// The first selected item, among at most `MaxScan` of `who`'s items, whose tank covers
    /// `estimated` in the current window, with what would be left. Reads only.
    fn find_tank(
        who: &T::AccountId,
        estimated: &Weight,
    ) -> Option<(F::CollectionId, F::ItemId, Weight)> {
        let now = P::current_block_number();
        let selector = S::get();

        F::owned(who)
            .take(MaxScan::get() as usize)
            .find_map(|(collection, item)| {
                if !selector.select(collection.clone(), item.clone()) {
                    return None;
                }

                let remaining = WeightTank::<BlockNumberFor<P>>::get::<T, F>(&collection, &item)?
                    .remaining_at(now, estimated)?;

                Some((collection, item, remaining))
            })
    }
}

impl<T, P, F, I, S, MaxScan> GasBurner for NonFungibleGasTank<T, P, F, I, S, MaxScan>
where
    T: frame_system::Config,
    P: BlockNumberProvider,
    BlockNumberFor<P>: Bounded,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, I>,
    I: Default,
    S: Get<Box<dyn SelectNonFungibleItem<F::CollectionId, F::ItemId>>>,
    MaxScan: Get<u32>,
{
    type AccountId = T::AccountId;
    type Gas = Weight;

    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        Self::find_tank(who, estimated).map(|(_, _, remaining)| remaining)
    }

    fn prepare_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        let (collection, item, remaining) = Self::find_tank(who, estimated)?;

        unhashed::put(&paying_item_key(who), &(collection, item, remaining));

        Some(remaining)
    }

    fn burn_gas(who: &Self::AccountId, expected: &Self::Gas, used: &Self::Gas) -> Self::Gas {
        // Taken before anything else, so the note never outlives the transaction.
        let Some((collection, item, noted)) =
            unhashed::take::<PayingItem<T, F>>(&paying_item_key(who))
        else {
            return Weight::zero();
        };
        // A note that does not match is not this transaction's preparation.
        if noted != *expected {
            return Weight::zero();
        }

        let Some(mut tank) = WeightTank::<BlockNumberFor<P>>::get::<T, F>(&collection, &item)
        else {
            return Weight::zero();
        };
        let Some(capacity) = tank.capacity_per_period else {
            return Weight::MAX;
        };

        tank.roll(P::current_block_number());
        tank.used = tank.used.saturating_add(*used);
        if tank.put::<T, F, I>(&collection, &item).is_err() {
            return Weight::zero();
        }

        capacity.saturating_sub(tank.used)
    }

    fn cancel_gas(who: &Self::AccountId, _expected: &Self::Gas) {
        unhashed::kill(&paying_item_key(who));
    }
}

impl<T, P, F, ItemConfig, S, MaxScan> GasFueler
    for NonFungibleGasTank<T, P, F, ItemConfig, S, MaxScan>
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

        tank.roll(P::current_block_number());
        tank.used = tank.used.saturating_sub(*gas);

        // Should infallibly save the tank, given that it already got a tank
        tank.put::<T, F, ItemConfig>(collection_id, item_id)
            .unwrap_or_default();

        tank.capacity_per_period
            .unwrap_or_default()
            .saturating_sub(tank.used)
    }
}

impl<T, P, F, ItemConfig, S, MaxScan> MakeTank
    for NonFungibleGasTank<T, P, F, ItemConfig, S, MaxScan>
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
