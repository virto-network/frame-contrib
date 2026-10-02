use super::*;

use alloc::vec;
use frame_benchmarking::v2::*;
use frame_support::traits::{fungible::Unbalanced, tokens::Precision};
use frame_system::RawOrigin;
use sp_runtime::traits::{Bounded, Saturating};

fn assert_has_event<T: Config<I>, I: 'static>(generic_event: T::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_has_event(generic_event)
}

type InventoryInfoOf<T, I> = (OriginFor<T>, InventoryIdFor<T, I>, AccountIdOf<T>);
type ItemSetupFor<T, I> = (OriginFor<T>, InventoryIdFor<T, I>, ItemIdOf<T, I>);

fn inventory_info<T: Config<I>, I: 'static>() -> Result<InventoryInfoOf<T, I>, DispatchError> {
    let inventory_id = T::BenchmarkHelper::inventory_id();
    let origin = T::CreateInventoryOrigin::try_successful_origin(&inventory_id)
        .map_err(|_| DispatchError::BadOrigin)?;

    let owner = T::CreateInventoryOrigin::ensure_origin(origin.clone(), &inventory_id)
        .map_err(|_| DispatchError::BadOrigin)?;

    let max = NativeBalanceOf::<T, I>::max_value();
    T::Balances::increase_balance(&owner, max, Precision::Exact)?;

    Ok((origin, inventory_id, owner))
}

fn setup_inventory<T: Config<I>, I: 'static>(
) -> Result<(OriginFor<T>, InventoryIdFor<T, I>), DispatchError> {
    let (origin, inventory_id, _) = inventory_info::<T, I>()?;
    Pallet::<T, I>::create_inventory(origin.clone(), inventory_id)?;
    Ok((origin, inventory_id))
}

fn setup_item<T: Config<I>, I: 'static>() -> Result<ItemSetupFor<T, I>, DispatchError>
where
    ItemIdOf<T, I>: Default,
{
    let (origin, inventory_id) = setup_inventory::<T, I>()?;
    let item_id = Default::default();

    Pallet::<T, I>::publish_item(
        origin.clone(),
        inventory_id,
        item_id,
        BoundedVec::truncate_from(b"".to_vec()),
        None,
    )?;

    Ok((origin, inventory_id, item_id))
}

// Subscriptions.

/// The ticks of the benchmarks' billing period: well beyond the lead and a bucket.
fn period<T: Config<I>, I: 'static>() -> MomentOf<T, I> {
    T::RenewalLead::get()
        .saturating_add(T::DueBucketSize::get())
        .saturating_mul(30u32.into())
        .max(30u32.into())
}

fn subscription_conditions<T: Config<I>, I: 'static>(
    min_commitment: Option<u32>,
) -> SubscriptionConditionsOf<T, I> {
    let (asset, amount) = T::BenchmarkHelper::price();
    let period = period::<T, I>();
    SubscriptionConditions {
        price: ItemPrice { asset, amount },
        period,
        term: Some(u32::MAX),
        min_commitment,
        grace: period / 10u32.into(),
    }
}

fn set_clock<T: Config<I>, I: 'static>(tick: MomentOf<T, I>) {
    T::BlockNumberProvider::set_block_number(tick);
}

/// An inventory with `n` subscription items, `0..n`, and the inventory's admin origin.
fn setup_subscription_items<T: Config<I>, I: 'static>(
    n: u32,
    min_commitment: Option<u32>,
) -> Result<(OriginFor<T>, InventoryIdFor<T, I>), BenchmarkError>
where
    ItemIdOf<T, I>: From<u32>,
{
    set_clock::<T, I>(1u32.into());
    let (origin, inventory_id) = setup_inventory::<T, I>()?;
    for i in 0..n {
        Pallet::<T, I>::publish_item(
            origin.clone(),
            inventory_id,
            i.into(),
            BoundedVec::truncate_from(b"subscription".to_vec()),
            None,
        )?;
        Pallet::<T, I>::set_subscription_conditions(
            origin.clone(),
            inventory_id,
            i.into(),
            subscription_conditions::<T, I>(min_commitment),
            None,
        )?;
    }
    Ok((origin, inventory_id))
}

/// A funded subscriber, and the origin that subscribes as it.
fn subscriber<T: Config<I>, I: 'static>(
    inventory_id: &InventoryIdFor<T, I>,
) -> Result<(OriginFor<T>, AccountIdOf<T>), BenchmarkError> {
    let origin = T::SubscribeOrigin::try_successful_origin(inventory_id)
        .map_err(|_| BenchmarkError::Weightless)?;
    let who = T::SubscribeOrigin::ensure_origin(origin.clone(), inventory_id)
        .map_err(|_| BenchmarkError::Weightless)?;
    fund::<T, I>(&who);
    Ok((origin, who))
}

fn fund<T: Config<I>, I: 'static>(who: &AccountIdOf<T>) {
    let (asset, amount) = T::BenchmarkHelper::price();
    T::BenchmarkHelper::fund(who, &asset, amount);
}

fn subscribe_as<T: Config<I>, I: 'static>(
    inventory_id: &InventoryIdFor<T, I>,
    item: ItemIdOf<T, I>,
    who: &AccountIdOf<T>,
) -> Result<(), BenchmarkError> {
    fund::<T, I>(who);
    <Pallet<T, I> as subs::Mutate<_>>::subscribe(&(*inventory_id).into(), &item, who)?;
    Ok(())
}

#[instance_benchmarks(
where
    AssetIdOf<T, I>: Default,
    ItemIdOf<T, I>: Default + From<u32>,
)]
mod benchmarks {
    use super::*;

    #[benchmark]
    pub fn create_inventory() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, owner) = inventory_info::<T, I>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id);

        // Verification code
        let InventoryId(merchant, id) = inventory_id;
        assert_has_event::<T, I>(
            Event::<T, I>::InventoryCreated {
                merchant,
                id,
                owner,
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn archive_inventory() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id) = setup_inventory::<T, I>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id);

        // Verification code
        let InventoryId(merchant, id) = inventory_id;
        assert_has_event::<T, I>(Event::<T, I>::InventoryArchived { merchant, id }.into());

        Ok(())
    }

    #[benchmark]
    pub fn publish_item(
        q: Linear<
            1,
            {
                T::NonfungiblesValueLimit::get()
                    - <Option<ItemPriceOf<T, I>> as MaxEncodedLen>::max_encoded_len() as u32
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
    ) -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id) = setup_inventory::<T, I>()?;
        let id = Default::default();
        let name = BoundedVec::truncate_from(vec![0u8; q as usize]);
        let price = ItemPrice {
            asset: Default::default(),
            amount: 1u32.into(),
        };

        #[extrinsic_call]
        _(
            origin as T::RuntimeOrigin,
            inventory_id,
            id,
            name,
            Some(price.clone()),
        );

        // Verification code
        assert_has_event::<T, I>(Event::<T, I>::ItemPublished { inventory_id, id }.into());
        assert_has_event::<T, I>(
            Event::<T, I>::ItemPriceSet {
                inventory_id,
                id,
                price,
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn mark_item_can_transfer() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, false);

        // Verification code
        assert!(!Pallet::<T, I>::transferable(&inventory_id.into(), &id));

        Ok(())
    }

    #[benchmark]
    pub fn mark_item_not_for_resale() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, true);

        // Verification code
        assert!(!Pallet::<T, I>::can_resell(&inventory_id.into(), &id));

        Ok(())
    }

    #[benchmark]
    pub fn set_item_price() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;
        let price = ItemPrice {
            asset: Default::default(),
            amount: 10u32.into(),
        };

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, price.clone());

        // Verification code
        assert_has_event::<T, I>(
            Event::<T, I>::ItemPriceSet {
                inventory_id,
                id,
                price,
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn clear_item_price() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;
        let price = ItemPrice {
            asset: Default::default(),
            amount: 10u32.into(),
        };
        Pallet::<T, I>::set_item_price(origin.clone(), inventory_id, id, price.clone())?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id);

        // Verification code
        assert_has_event::<T, I>(Event::<T, I>::ItemPriceCleared { inventory_id, id }.into());

        Ok(())
    }

    #[benchmark]
    pub fn set_item_attribute(
        p: Linear<
            1,
            {
                T::NonfungiblesKeyLimit::get()
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
        q: Linear<
            1,
            {
                T::NonfungiblesValueLimit::get()
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
    ) -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;

        let key = BoundedVec::truncate_from(vec![0u8; p as usize]);
        let value = BoundedVec::truncate_from(vec![0u8; q as usize]);

        #[extrinsic_call]
        _(
            origin as T::RuntimeOrigin,
            inventory_id,
            id,
            key.clone(),
            Some(value.clone()),
        );

        // Verification code
        assert_eq!(
            Pallet::<T, I>::attribute(&inventory_id.into(), &id, &key),
            Some(value)
        );

        Ok(())
    }

    #[benchmark]
    pub fn clear_item_attribute(
        p: Linear<
            1,
            {
                T::NonfungiblesKeyLimit::get()
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
        q: Linear<
            1,
            {
                T::NonfungiblesValueLimit::get()
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
    ) -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;

        let key = BoundedVec::truncate_from(vec![0u8; p as usize]);
        let value = BoundedVec::truncate_from(vec![0u8; q as usize]);

        Pallet::<T, I>::set_item_attribute(
            origin.clone(),
            inventory_id,
            id,
            key.clone(),
            Some(value),
        )?;

        #[extrinsic_call]
        set_item_attribute(
            origin as T::RuntimeOrigin,
            inventory_id,
            id,
            key.clone(),
            None,
        );

        // Verification code
        assert_eq!(
            Pallet::<T, I>::attribute(&inventory_id.into(), &id, &key),
            None::<Vec<u8>>
        );

        Ok(())
    }

    #[benchmark]
    pub fn set_subscription_conditions() -> Result<(), BenchmarkError> {
        let (origin, inventory_id) = setup_subscription_items::<T, I>(1, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let conditions = subscription_conditions::<T, I>(Some(1));
        let exclusive: AccountIdOf<T> = account("exclusive", 0, 0);

        #[extrinsic_call]
        _(
            origin as T::RuntimeOrigin,
            inventory_id,
            id,
            conditions.clone(),
            Some(exclusive.clone()),
        );

        assert_eq!(
            Pallet::<T, I>::subscription_conditions(&inventory_id.into(), &id),
            Some(conditions)
        );
        assert_eq!(
            Pallet::<T, I>::eligibility(&inventory_id.into(), &id),
            Some(Eligibility::Once(exclusive))
        );
        Ok(())
    }

    /// The worst case: an expired *Defaulted* record of the same subscriber, ended first (its
    /// entry taken out of a full bucket), an item amendment on record, and the new entry's first
    /// `MAX_QUEUE_PROBES - 1` buckets full, so it probes every bucket and lands in the last one,
    /// all but full. (A strict enqueue never reaches the overflow: it is refused instead.)
    #[benchmark]
    pub fn subscribe() -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(1, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        // An item amendment on record: one more read.
        <Pallet<T, I> as subs::Mutate<_>>::amend_item(
            &inventory_id.into(),
            &id,
            subscription_conditions::<T, I>(None),
        )?;
        let (origin, who) = subscriber::<T, I>(&inventory_id)?;
        let key: SubscriptionKeyOf<T, I> = (inventory_id, id, who.clone());
        let fillers = |tag: &'static str, n: u32| {
            (0..n)
                .map(|i| (inventory_id, id, account::<AccountIdOf<T>>(tag, i, 0)))
                .collect::<Vec<_>>()
        };
        let max = T::MaxDuePerBucket::get();

        // An earlier subscription of the same subscriber, *Defaulted* until now, queued in a full
        // bucket: subscribing ends it first.
        subscribe_as::<T, I>(&inventory_id, id, &who)?;
        let now = period::<T, I>().saturating_mul(2u32.into());
        let mut old_bucket = None;
        Subscriptions::<T, I>::mutate(key.clone(), |maybe_record| {
            if let Some(record) = maybe_record {
                record.subscription.state = SubscriptionState::Defaulted { until: now };
                old_bucket = record.queued_at;
            }
        });
        let old_bucket = old_bucket.ok_or(BenchmarkError::Stop("not queued"))?;
        let mut queue = fillers("old", max.saturating_sub(1));
        queue.push(key.clone());
        DueQueue::<T, I>::insert(old_bucket, BoundedVec::truncate_from(queue));

        // The new entry's buckets: full but for the last, which has one place left.
        let target = Pallet::<T, I>::bucket_of(
            now.saturating_add(period::<T, I>())
                .saturating_sub(T::RenewalLead::get()),
        )
        .max(Pallet::<T, I>::queue_floor(now));
        let mut bucket = target;
        for probe in 0..subscriptions::MAX_QUEUE_PROBES {
            let n = if probe + 1 < subscriptions::MAX_QUEUE_PROBES {
                max
            } else {
                max.saturating_sub(1)
            };
            DueQueue::<T, I>::insert(bucket, BoundedVec::truncate_from(fillers("new", n)));
            bucket = bucket.saturating_add(1u32.into());
        }
        let last = bucket.saturating_sub(1u32.into());
        set_clock::<T, I>(now);
        fund::<T, I>(&who);

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id);

        let record =
            Subscriptions::<T, I>::get(key.clone()).ok_or(BenchmarkError::Stop("no record"))?;
        assert_eq!(record.subscription.anchor, now);
        assert_eq!(record.queued_at, Some(last));
        assert!(!DueQueue::<T, I>::get(old_bucket).contains(&key));
        Ok(())
    }

    #[benchmark]
    pub fn charge_due() -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(1, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let who: AccountIdOf<T> = account("subscriber", 0, 0);
        subscribe_as::<T, I>(&inventory_id, id, &who)?;
        let due = Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who)
            .map(|s| s.paid_through)
            .ok_or(BenchmarkError::Weightless)?;
        set_clock::<T, I>(due.saturating_sub(T::RenewalLead::get()));
        let caller: AccountIdOf<T> = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), inventory_id, id, who.clone());

        assert_eq!(
            Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who)
                .map(|s| s.periods_charged),
            Some(2)
        );
        Ok(())
    }

    #[benchmark]
    pub fn cancel_subscription() -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(2, Some(3))?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let (origin, who) = subscriber::<T, I>(&inventory_id)?;
        subscribe_as::<T, I>(&inventory_id, id, &who)?;
        // A replacement to drop.
        Pallet::<T, I>::schedule_replacement(origin.clone(), inventory_id, id, 1u32.into())?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id);

        assert_eq!(
            Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who)
                .and_then(|s| s.cancel_requested),
            Some(Cancellation::Ordinary)
        );
        Ok(())
    }

    #[benchmark]
    pub fn terminate_subscription() -> Result<(), BenchmarkError> {
        let (origin, inventory_id) = setup_subscription_items::<T, I>(1, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let who: AccountIdOf<T> = account("subscriber", 0, 0);
        subscribe_as::<T, I>(&inventory_id, id, &who)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, who.clone());

        assert!(Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who).is_none());
        Ok(())
    }

    #[benchmark]
    pub fn schedule_replacement() -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(2, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let new_id: ItemIdOf<T, I> = 1u32.into();
        let (origin, who) = subscriber::<T, I>(&inventory_id)?;
        subscribe_as::<T, I>(&inventory_id, id, &who)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, new_id);

        assert_eq!(
            Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who)
                .and_then(|s| s.replacement),
            Some(new_id)
        );
        Ok(())
    }

    #[benchmark]
    pub fn amend_subscription() -> Result<(), BenchmarkError> {
        let (origin, inventory_id) = setup_subscription_items::<T, I>(2, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let (subscriber_origin, who) = subscriber::<T, I>(&inventory_id)?;
        subscribe_as::<T, I>(&inventory_id, id, &who)?;
        // A replacement the amendment drops.
        Pallet::<T, I>::schedule_replacement(subscriber_origin, inventory_id, id, 1u32.into())?;
        let conditions = subscription_conditions::<T, I>(Some(2));

        #[extrinsic_call]
        _(
            origin as T::RuntimeOrigin,
            inventory_id,
            id,
            who.clone(),
            conditions,
        );

        assert!(
            Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who)
                .and_then(|s| s.pending_conditions)
                .is_some()
        );
        Ok(())
    }

    #[benchmark]
    pub fn cancel_replacement() -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(2, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let (origin, who) = subscriber::<T, I>(&inventory_id)?;
        subscribe_as::<T, I>(&inventory_id, id, &who)?;
        Pallet::<T, I>::schedule_replacement(origin.clone(), inventory_id, id, 1u32.into())?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id);

        assert_eq!(
            Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who)
                .and_then(|s| s.replacement),
            None
        );
        Ok(())
    }

    #[benchmark]
    pub fn amend_item_subscriptions() -> Result<(), BenchmarkError> {
        let (origin, inventory_id) = setup_subscription_items::<T, I>(1, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        // A previous amendment, whose boundaries have all passed.
        let previous = <Pallet<T, I> as subs::Mutate<_>>::amend_item(
            &inventory_id.into(),
            &id,
            subscription_conditions::<T, I>(None),
        )?;
        set_clock::<T, I>(previous.last_boundary());
        let conditions = subscription_conditions::<T, I>(Some(1));

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, conditions);

        assert_eq!(
            Pallet::<T, I>::item_amendment(&inventory_id.into(), &id).map(|a| a.seq),
            Some(2)
        );
        Ok(())
    }

    /// `n` renewals due in one bucket, processed by the due queue.
    #[benchmark]
    pub fn process_due(n: Linear<1, { T::MaxDuePerBucket::get() }>) -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(1, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let subscribers = (0..n)
            .map(|i| account::<AccountIdOf<T>>("subscriber", i, 0))
            .collect::<Vec<_>>();
        for who in &subscribers {
            subscribe_as::<T, I>(&inventory_id, id, who)?;
        }
        let lead_tick = period::<T, I>()
            .saturating_add(1u32.into())
            .saturating_sub(T::RenewalLead::get());
        let bucket = Pallet::<T, I>::bucket_of(lead_tick);
        let now = bucket.saturating_mul(T::DueBucketSize::get());
        set_clock::<T, I>(now);
        let mut meter = WeightMeter::new();
        let mut budget = n;

        #[block]
        {
            Pallet::<T, I>::walk(bucket, bucket, now, &mut meter, &mut budget);
        }

        for who in &subscribers {
            assert_eq!(
                Pallet::<T, I>::subscription(&inventory_id.into(), &id, who)
                    .map(|s| s.periods_charged),
                Some(2)
            );
        }
        Ok(())
    }

    /// One scheduled replacement taking effect from the due queue.
    #[benchmark]
    pub fn process_due_replacement() -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(2, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let new_id: ItemIdOf<T, I> = 1u32.into();
        let (origin, who) = subscriber::<T, I>(&inventory_id)?;
        subscribe_as::<T, I>(&inventory_id, id, &who)?;
        Pallet::<T, I>::schedule_replacement(origin, inventory_id, id, new_id)?;

        // The entry stays at the lead; processed there, it moves to the due tick.
        let due = period::<T, I>().saturating_add(1u32.into());
        let lead_bucket = Pallet::<T, I>::bucket_of(due.saturating_sub(T::RenewalLead::get()));
        let lead_now = lead_bucket.saturating_mul(T::DueBucketSize::get());
        set_clock::<T, I>(lead_now);
        Pallet::<T, I>::walk(
            lead_bucket,
            lead_bucket,
            lead_now,
            &mut WeightMeter::new(),
            &mut 1,
        );

        let bucket = Pallet::<T, I>::bucket_of(due);
        let now = bucket.saturating_mul(T::DueBucketSize::get());
        set_clock::<T, I>(now);
        let mut meter = WeightMeter::new();
        let mut budget = 1;

        #[block]
        {
            Pallet::<T, I>::walk(bucket, bucket, now, &mut meter, &mut budget);
        }

        assert!(Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who).is_none());
        assert!(Pallet::<T, I>::subscription(&inventory_id.into(), &new_id, &who).is_some());
        Ok(())
    }

    /// One empty bucket read by the due queue.
    #[benchmark]
    pub fn skip_empty_bucket() -> Result<(), BenchmarkError> {
        let now: MomentOf<T, I> = 1_000u32.into();
        set_clock::<T, I>(now);
        let bucket = Pallet::<T, I>::current_bucket(now);
        let mut meter = WeightMeter::new();
        let mut budget = 1;

        #[block]
        {
            Pallet::<T, I>::walk(bucket, bucket, now, &mut meter, &mut budget);
        }

        assert_eq!(budget, 1);
        Ok(())
    }

    /// One renewal in a bucket's overflow, processed by the due queue, whose next entry finds the
    /// buckets it probes full and goes to the overflow again.
    #[benchmark]
    pub fn process_due_overflow() -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(1, None)?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let who: AccountIdOf<T> = account("subscriber", 0, 0);
        subscribe_as::<T, I>(&inventory_id, id, &who)?;
        fund::<T, I>(&who);
        let key: SubscriptionKeyOf<T, I> = (inventory_id, id, who.clone());

        // Its entry, moved to the overflow of its bucket.
        let lead_tick = period::<T, I>()
            .saturating_add(1u32.into())
            .saturating_sub(T::RenewalLead::get());
        let bucket = Pallet::<T, I>::bucket_of(lead_tick);
        DueQueue::<T, I>::remove(bucket);
        DueOverflow::<T, I>::insert(bucket, &key, ());

        // The buckets of its next entry, full.
        let next = Pallet::<T, I>::bucket_of(lead_tick.saturating_add(period::<T, I>()));
        let fillers = (0..T::MaxDuePerBucket::get())
            .map(|i| (inventory_id, id, account::<AccountIdOf<T>>("filler", i, 0)))
            .collect::<Vec<_>>();
        let mut full = next;
        for _ in 0..subscriptions::MAX_QUEUE_PROBES {
            DueQueue::<T, I>::insert(full, BoundedVec::truncate_from(fillers.clone()));
            full = full.saturating_add(1u32.into());
        }

        let now = bucket.saturating_mul(T::DueBucketSize::get());
        set_clock::<T, I>(now);
        let mut meter = WeightMeter::new();
        let mut budget = 1;

        #[block]
        {
            Pallet::<T, I>::walk(bucket, bucket, now, &mut meter, &mut budget);
        }

        assert_eq!(
            Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who)
                .map(|s| s.periods_charged),
            Some(2)
        );
        assert!(DueOverflow::<T, I>::contains_key(next, &key));
        Ok(())
    }

    #[benchmark]
    pub fn settle_subscription() -> Result<(), BenchmarkError> {
        let (_, inventory_id) = setup_subscription_items::<T, I>(2, Some(3))?;
        let id: ItemIdOf<T, I> = 0u32.into();
        let (origin, who) = subscriber::<T, I>(&inventory_id)?;
        subscribe_as::<T, I>(&inventory_id, id, &who)?;
        // A replacement the default drops.
        Pallet::<T, I>::schedule_replacement(origin, inventory_id, id, 1u32.into())?;
        // Suspended at its due tick, and past grace: it defaults, within its commitment.
        let mut grace_end = Zero::zero();
        Subscriptions::<T, I>::mutate((inventory_id, id, who.clone()), |maybe_record| {
            if let Some(record) = maybe_record {
                let sub = &mut record.subscription;
                sub.state = SubscriptionState::Suspended {
                    since: sub.paid_through,
                };
                grace_end = sub.paid_through.saturating_add(sub.conditions.grace);
            }
        });
        set_clock::<T, I>(grace_end);
        let caller: AccountIdOf<T> = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), inventory_id, id, who.clone());

        assert!(matches!(
            Pallet::<T, I>::subscription(&inventory_id.into(), &id, &who).map(|s| s.state),
            Some(SubscriptionState::Defaulted { .. })
        ));
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), mock::Test);
}
