//! The pool and the payment step (`US-C1`–`US-C4`, `US-D2`, `REQ-PL-*`, `REQ-PC-*`, `CTR-FEE-*`,
//! `REQ-TX-*`), through `ChargeUsageSubscription`, alone and after `CheckWeight` as the crate guide
//! says.

use super::*;
use codec::Encode;
use frame_support::dispatch::{
    CheckIfFeeless, DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo,
};
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::{
    generic::ExtrinsicFormat,
    traits::{Applyable, DispatchTransaction, TransactionExtension},
    transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidityError},
    StateVersion,
};

/// A member with no balance at all: it can only transact on the pool path.
const DAVE: AccountId = AccountId::new([9u8; 32]);

fn storage_root() -> Vec<u8> {
    sp_io::storage::root(StateVersion::V1)
}

fn remark() -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::remark {
        remark: b"Hello world".to_vec(),
    })
}

fn usage_ext() -> UsageExtension {
    UsageExtension::new(ChargeTransactionPayment::from(0))
}

fn tx_ext() -> TxExtensions {
    (frame_system::CheckWeight::new(), usage_ext())
}

/// The dispatch info of `call` in a block, with the declared weight of the runtime's extensions.
fn info(call: &RuntimeCall) -> DispatchInfo {
    DispatchInfo {
        extension_weight: tx_ext().weight(call),
        ..call.get_dispatch_info()
    }
}

/// A dispatch info declaring `call_weight` and the payment step's declared weight, for a test
/// that runs the payment step alone.
fn declared(call_weight: Weight) -> DispatchInfo {
    DispatchInfo {
        call_weight,
        extension_weight: usage_ext().weight(&remark()),
        ..Default::default()
    }
}

/// Validates, prepares, dispatches and post-dispatches `call` from `who`, through `CheckWeight`
/// and the payment step.
fn apply(
    who: &AccountId,
    call: RuntimeCall,
) -> sp_runtime::ApplyExtrinsicResultWithInfo<PostDispatchInfo> {
    let info = info(&call);
    let len = call.encoded_size();
    CheckedExtrinsic {
        format: ExtrinsicFormat::Signed(who.clone(), tx_ext()),
        function: call,
    }
    .apply::<Test>(&info, len)
}

/// Validates, prepares, dispatches and post-dispatches `call` from `who` through `ext`.
fn apply_with<E: TransactionExtension<RuntimeCall>>(
    who: &AccountId,
    call: RuntimeCall,
    ext: E,
) -> sp_runtime::ApplyExtrinsicResultWithInfo<PostDispatchInfo> {
    let info = DispatchInfo {
        extension_weight: ext.weight(&call),
        ..call.get_dispatch_info()
    };
    let len = call.encoded_size();
    sp_runtime::generic::CheckedExtrinsic::<AccountId, RuntimeCall, E> {
        format: ExtrinsicFormat::Signed(who.clone(), ext),
        function: call,
    }
    .apply::<Test>(&info, len)
}

/// The path validation chooses for `who`, with `info` and `len`.
fn path_of(who: &AccountId, info: &DispatchInfo, len: usize) -> Path<(), ()> {
    let (_, val, _) = usage_ext()
        .validate_only(
            RuntimeOrigin::signed(who.clone()),
            &remark(),
            info,
            len,
            TransactionSource::External,
            0,
        )
        .expect("valid on one path or the other; qed");
    match val {
        Path::Pool(_) => Path::Pool(()),
        Path::Fee(_) => Path::Fee(()),
    }
}

fn is_pool(path: Path<(), ()>) -> bool {
    matches!(path, Path::Pool(_))
}

fn used(group: u32) -> Weight {
    Contracts::<Test>::get(group)
        .map(|contract| contract.used)
        .unwrap_or_default()
}

fn usage_charged() -> Vec<(u32, AccountId, Weight, Weight)> {
    events()
        .into_iter()
        .filter_map(|event| match event {
            crate::Event::UsageCharged {
                group,
                who,
                weight,
                remaining,
            } => Some((group, who, weight, remaining)),
            _ => None,
        })
        .collect()
}

/// A standard offer; group A subscribed to it; ALICE and DAVE members of A.
fn setup() -> u32 {
    let offer = standard();
    subscribe(GROUP_A, offer);
    add_member(GROUP_A, 1, &ALICE);
    add_member(GROUP_A, 2, &DAVE);
    clear_events();
    offer
}

/// `REQ-PL-6`: the metered weight of a transaction declaring `call_weight`, with `len` bytes.
fn metered(call_weight: Weight, len: usize) -> Weight {
    call_weight
        .saturating_add(BASE_EXTRINSIC)
        .saturating_add(Weight::from_parts(0, len as u64))
}

/// `CTR-FEE-5`: the payment step's own unspent weight on the pool path, what the pool path leaves
/// of its declared weight.
fn pool_path_unspent() -> Weight {
    usage_ext()
        .weight(&remark())
        .saturating_sub(TestWeights::pool_path())
}

/// `REQ-PL-6`: what the block books, and the pool is charged, for a pool-path transaction with
/// `info` and `len` that uses all of its declared call weight: the estimate less the payment
/// step's own unspent weight.
fn pool_booked(info: &DispatchInfo, len: usize) -> Weight {
    metered(info.total_weight().saturating_sub(pool_path_unspent()), len)
}

/// What the fee path uses of the payment step's declared weight after the full check: the check,
/// then the fee extension.
fn fee_path_used(call: &RuntimeCall) -> Weight {
    TestWeights::fee_path_check()
        .saturating_add(ChargeTransactionPayment::<Test>::from(0).weight(call))
}

/// Makes the pool path heavier than the fee path in both components, so the payment step leaves
/// weight unspent on the fee path.
fn pool_path_heavier() {
    PoolPathWeight::set(Some(
        fee_path_used(&remark()).saturating_add(Weight::from_parts(1_000_000, 1_000)),
    ));
}

/// Makes the fee path heavier than the pool path in both components, so the payment step leaves
/// weight unspent on the pool path.
fn fee_path_heavier() {
    PoolPathWeight::set(Some(Weight::from_parts(1_000, 10)));
}

/// Exhausts the pool of `group` in its current window: its members take the fee path, after the
/// full check.
fn exhaust(group: u32) {
    Contracts::<Test>::mutate(group, |c| {
        if let Some(c) = c {
            c.used = ALLOWANCE;
        }
    });
}

/// Which path validation takes, and preparation keeping to it.
mod admission {
    use super::*;

    // INV-1, CTR-FEE-1
    #[test]
    fn admission_writes_nothing_and_is_deterministic() {
        new_test_ext().execute_with(|| {
            setup();
            let call = remark();
            let info = info(&call);
            let len = call.encoded_size();
            let before = storage_root();

            // The pool path, and the fee path.
            assert!(is_pool(path_of(&ALICE, &info, len)));
            assert!(!is_pool(path_of(&BOB, &info, len)));
            let estimate = metered(info.total_weight(), len);
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate),
                UsageSubscription::check(&ALICE, estimate)
            );
            assert_eq!(
                UsageSubscription::check(&BOB, estimate),
                Err(FeePathReason::NoPayingGroup)
            );

            assert_eq!(storage_root(), before);
        });
    }

    // INV-2, INV-15, AC-C2.3
    #[test]
    fn preparation_rejects_a_changed_answer_without_panicking() {
        new_test_ext().execute_with(|| {
            setup();
            let call = remark();
            let info = info(&call);
            let len = call.encoded_size();

            // Each change between validation and preparation: the pool exhausted, the contract
            // terminated, the membership released.
            let changes: [fn(); 3] = [
                || {
                    Contracts::<Test>::mutate(GROUP_A, |c| {
                        if let Some(c) = c {
                            c.used = ALLOWANCE;
                        }
                    })
                },
                || {
                    assert_ok!(UsageSubscription::terminate_contract(
                        RuntimeOrigin::root(),
                        GROUP_A
                    ));
                },
                || {
                    use fc_traits_memberships::Manager;
                    assert_ok!(MembershipsManager::release(&GROUP_A, &1));
                },
            ];
            for change in changes {
                let (_, val, origin) = usage_ext()
                    .validate_only(
                        RuntimeOrigin::signed(ALICE),
                        &call,
                        &info,
                        len,
                        TransactionSource::InBlock,
                        0,
                    )
                    .expect("the pool covers it; qed");
                assert!(matches!(val, Path::Pool(_)));

                frame_support::storage::with_transaction(|| {
                    change();
                    let balance = Balances::free_balance(&ALICE);
                    assert_eq!(
                        usage_ext()
                            .prepare(val, &origin, &call, &info, len)
                            .map(|_| ()),
                        Err(InvalidTransaction::Custom(PATH_MISMATCH).into())
                    );
                    // Never the fee path, which was not validated.
                    assert_eq!(Balances::free_balance(&ALICE), balance);
                    sp_runtime::TransactionOutcome::Rollback(Ok::<_, DispatchError>(()))
                })
                .expect("rolled back; qed");
            }
        });
    }

    // INV-2
    #[test]
    fn fee_path_never_asks_the_pool_again() {
        new_test_ext().execute_with(|| {
            setup();
            let call = remark();
            let info = info(&call);
            let len = call.encoded_size();
            let (_, val, origin) = usage_ext()
                .validate_only(
                    RuntimeOrigin::signed(BOB),
                    &call,
                    &info,
                    len,
                    TransactionSource::InBlock,
                    0,
                )
                .expect("BOB pays; qed");

            // BOB becomes a member between the phases. Preparation still takes the fee path.
            add_member(GROUP_A, 3, &BOB);
            let balance = Balances::free_balance(&BOB);
            let pre = usage_ext()
                .prepare(val, &origin, &call, &info, len)
                .expect("the fee path was validated; qed");
            assert!(matches!(pre, Path::Fee(_)));
            assert!(Balances::free_balance(&BOB) < balance);
        });
    }

    // REQ-PL-10
    #[test]
    fn pool_path_has_default_priority() {
        new_test_ext().execute_with(|| {
            setup();
            let (valid, _, _) = UsageExtension::new(ChargeTransactionPayment::from(1_000))
                .validate_only(
                    RuntimeOrigin::signed(ALICE),
                    &remark(),
                    &declared(Weight::from_parts(10, 0)),
                    0,
                    TransactionSource::External,
                    0,
                )
                .expect("pool; qed");
            assert_eq!(valid.priority, 0);
        });
    }

    // REQ-TX-1
    #[test]
    fn failing_fee_path_is_invalid_as_the_fee_path_alone() {
        new_test_ext().execute_with(|| {
            // DAVE has no balance and no pool.
            assert_eq!(
                usage_ext()
                    .validate_only(
                        RuntimeOrigin::signed(DAVE),
                        &remark(),
                        &declared(Weight::from_parts(10, 0)),
                        0,
                        TransactionSource::External,
                        0,
                    )
                    .map(|_| ()),
                Err(TransactionValidityError::Invalid(
                    InvalidTransaction::Payment
                ))
            );
        });
    }

    // INV-7
    #[test]
    fn no_pool_path_without_a_usable_group_a_paid_contract_and_a_member() {
        new_test_ext().execute_with(|| {
            let call = remark();
            let info = info(&call);
            let len = call.encoded_size();
            // Nothing yet.
            add_member(GROUP_A, 1, &ALICE);
            assert!(!is_pool(path_of(&ALICE, &info, len)));
            // A paid contract.
            subscribe(GROUP_A, standard());
            assert!(is_pool(path_of(&ALICE, &info, len)));
            // Unpaid.
            Clock::set(B);
            set_funds(GROUP_A, 0);
            assert!(!is_pool(path_of(&ALICE, &info, len)));
        });
    }

    // AC-C2.1
    #[test]
    fn fee_path_when_the_pool_is_exhausted() {
        new_test_ext().execute_with(|| {
            setup();
            Contracts::<Test>::mutate(GROUP_A, |c| {
                if let Some(c) = c {
                    c.used = ALLOWANCE;
                }
            });
            let balance = Balances::free_balance(&ALICE);
            let result = apply(&ALICE, remark());
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");
            assert!(Balances::free_balance(&ALICE) < balance);
        });
    }
}

/// What a transaction is charged after dispatch, and the weight the payment step declares.
mod charging {
    use super::*;

    // AC-C1.1, REQ-PL-10
    #[test]
    fn pool_path_costs_nothing_and_charges_the_pool() {
        new_test_ext().execute_with(|| {
            setup();
            let call = remark();
            let len = call.encoded_size();
            let charged = pool_booked(&info(&call), len);
            let balance = Balances::free_balance(&ALICE);

            let result = apply(&ALICE, call);
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");

            assert_eq!(Balances::free_balance(&ALICE), balance);
            assert_eq!(used(GROUP_A), charged);
            assert_eq!(
                usage_charged(),
                vec![(GROUP_A, ALICE, charged, ALLOWANCE.saturating_sub(charged))]
            );
            // `CTR-EVT-2`: the only event of this pallet for a pool-path transaction.
            assert_eq!(events().len(), 1);
        });
    }

    // AC-C1.2, INV-3
    #[test]
    fn pool_is_charged_the_actual_not_the_estimate() {
        new_test_ext().execute_with(|| {
            setup();
            // The payment step uses all of its declared weight on the pool path.
            pool_path_heavier();
            // Estimate: 95 declared, the payment step's, and 5 base. Actual: 5, the payment
            // step's, and 5 base.
            let info = declared(Weight::from_parts(95, 0));
            let actual = metered(
                Weight::from_parts(5, 0).saturating_add(info.extension_weight),
                0,
            );
            assert_ok!(usage_ext().test_run(
                RuntimeOrigin::signed(ALICE),
                &remark(),
                &info,
                0,
                0,
                |_| Ok(PostDispatchInfo {
                    actual_weight: Some(Weight::from_parts(5, 0)),
                    pays_fee: Pays::Yes,
                }),
            ));
            assert_eq!(used(GROUP_A), actual);

            // Length counts as proof size, and the charge is capped at the estimate.
            let len = 100;
            assert_ok!(usage_ext().test_run(
                RuntimeOrigin::signed(ALICE),
                &remark(),
                &info,
                len,
                0,
                |_| Ok(PostDispatchInfo {
                    actual_weight: Some(Weight::from_parts(1_000, 0)),
                    pays_fee: Pays::Yes,
                }),
            ));
            assert_eq!(
                used(GROUP_A),
                actual.saturating_add(metered(info.total_weight(), len))
            );
        });
    }

    #[test]
    fn pays_no_charges_nothing() {
        new_test_ext().execute_with(|| {
            setup();
            assert_ok!(usage_ext().test_run(
                RuntimeOrigin::signed(ALICE),
                &remark(),
                &declared(Weight::from_parts(95, 0)),
                0,
                0,
                |_| Ok(PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::No,
                }),
            ));
            assert_eq!(used(GROUP_A), Weight::zero());
            assert!(usage_charged().is_empty());
        });
    }

    // INV-11, CTR-FEE-5
    #[test]
    fn call_weight_is_not_reported_as_unspent() {
        new_test_ext().execute_with(|| {
            setup();
            fee_path_heavier();
            assert!(pool_path_unspent().all_gt(Weight::zero()));
            let result = usage_ext()
                .test_run(
                    RuntimeOrigin::signed(ALICE),
                    &remark(),
                    &declared(Weight::from_parts(100, 0)),
                    0,
                    0,
                    |_| {
                        Ok(PostDispatchInfo {
                            actual_weight: Some(Weight::from_parts(60, 0)),
                            pays_fee: Pays::Yes,
                        })
                    },
                )
                .expect("valid; qed")
                .expect("dispatched; qed");
            // The call's unused 40 stays booked: only the payment step's own unspent weight is
            // refunded, and what is left of it is the pool path's.
            assert_eq!(
                result.actual_weight,
                Some(Weight::from_parts(60, 0).saturating_add(TestWeights::pool_path()))
            );
        });
    }

    // NFR-2, CTR-FEE-5, INV-11
    #[test]
    fn fee_path_reports_the_extensions_own_unspent_weight() {
        new_test_ext().execute_with(|| {
            setup();
            pool_path_heavier();
            let call = remark();
            let info = declared(Weight::from_parts(95, 0));
            let len = call.encoded_size();

            // ALICE's pool is exhausted: the fee path, after the full check.
            exhaust(GROUP_A);
            let (_, val, origin) = usage_ext()
                .validate_only(
                    RuntimeOrigin::signed(ALICE),
                    &call,
                    &info,
                    len,
                    TransactionSource::External,
                    0,
                )
                .expect("valid; qed");
            assert!(matches!(val, Path::Fee(_)));
            let pre = usage_ext()
                .prepare(val, &origin, &call, &info, len)
                .expect("prepared; qed");
            // As block execution reports it: the declared weights, the payment step's included.
            let post_info = PostDispatchInfo {
                actual_weight: Some(info.total_weight()),
                pays_fee: Pays::Yes,
            };
            let unspent =
                UsageExtension::post_dispatch_details(pre, &info, &post_info, len, &Ok(()))
                    .expect("post-dispatched; qed");

            // The declared weight less what the fee path used; the inner extension reports none
            // of its own for a transaction that paid its fee.
            assert!(unspent.all_gt(Weight::zero()));
            assert_eq!(
                unspent,
                info.extension_weight.saturating_sub(fee_path_used(&call))
            );
        });
    }

    // CTR-FEE-5, CTR-FEE-6
    #[test]
    fn fee_path_is_charged_the_check_only_up_to_where_it_stopped() {
        new_test_ext().execute_with(|| {
            setup();
            let call = remark();
            let inner = ChargeTransactionPayment::<Test>::from(0).weight(&call);
            // BOB holds no membership: the check stops at the paying group. CHARLIE's only group
            // has no contract: it stops there.
            add_member(GROUP_C, 40, &CHARLIE);
            let info = declared(Weight::from_parts(1_000, 0));
            let post = PostDispatchInfo {
                actual_weight: Some(Weight::from_parts(400, 0)),
                pays_fee: Pays::Yes,
            };

            for (who, check) in [
                (BOB, TestWeights::fee_path_no_paying_group()),
                (CHARLIE, TestWeights::fee_path_no_contract()),
            ] {
                // No more than the worst case the payment step declares in any component (two
                // paths may prove the same storage maps), and strictly less in time.
                assert!(check.all_lte(TestWeights::fee_path_check()));
                assert!(check.ref_time() < TestWeights::fee_path_check().ref_time());

                let before = Balances::free_balance(&who);
                let result = usage_ext()
                    .test_run(
                        RuntimeOrigin::signed(who.clone()),
                        &call,
                        &info,
                        10,
                        0,
                        |_| Ok(post),
                    )
                    .expect("valid; qed")
                    .expect("dispatched; qed");
                let wrapped = before - Balances::free_balance(&who);
                // The block keeps the call's actual weight, the check up to where it stopped,
                // and the fee extension's.
                assert_eq!(
                    result.actual_weight,
                    Some(
                        Weight::from_parts(400, 0)
                            .saturating_add(check)
                            .saturating_add(inner)
                    )
                );

                // The fee extension alone, declaring that much, charges the same.
                let alone_info = DispatchInfo {
                    extension_weight: check.saturating_add(inner),
                    ..info
                };
                let before = Balances::free_balance(&who);
                assert_ok!(ChargeTransactionPayment::<Test>::from(0).test_run(
                    RuntimeOrigin::signed(who.clone()),
                    &call,
                    &alone_info,
                    10,
                    0,
                    |_| Ok(post)
                ));
                assert_eq!(wrapped, before - Balances::free_balance(&who));
            }
        });
    }

    // CTR-FEE-6
    #[test]
    fn declared_weight_covers_both_paths() {
        new_test_ext().execute_with(|| {
            let call = remark();
            let inner = ChargeTransactionPayment::<Test>::from(0).weight(&call);
            assert_eq!(
                usage_ext().weight(&call),
                TestWeights::pool_path().max(TestWeights::fee_path_check().saturating_add(inner))
            );
        });
    }

    // CTR-FEE-5, REQ-PL-6
    #[test]
    fn pool_is_not_charged_the_fee_paths_weight() {
        new_test_ext().execute_with(|| {
            setup();
            fee_path_heavier();
            let call = remark();
            let len = call.encoded_size();
            let info = info(&call);
            let unspent = pool_path_unspent();
            assert!(unspent.all_gt(Weight::zero()));

            let result = apply(&ALICE, call.clone());
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");

            // The call, `CheckWeight` and the pool path, and none of the fee path's weight,
            // which never ran.
            let charged = metered(
                info.call_weight
                    .saturating_add(frame_system::CheckWeight::<Test>::new().weight(&call))
                    .saturating_add(TestWeights::pool_path()),
                len,
            );
            assert_eq!(used(GROUP_A), charged);
            assert_eq!(
                charged,
                metered(info.total_weight(), len).saturating_sub(unspent)
            );
        });
    }

    // INV-11, CTR-FEE-5, REQ-PL-6
    #[test]
    fn block_books_only_what_the_path_taken_used() {
        // The payment step between `CheckWeight` and `WeightReclaim`, which re-books the
        // block's weight from the refunds of the extensions before it.
        type Extensions = (
            frame_system::CheckWeight<Test>,
            UsageExtension,
            frame_system::WeightReclaim<Test>,
        );
        let extensions = || -> Extensions {
            (
                frame_system::CheckWeight::new(),
                usage_ext(),
                frame_system::WeightReclaim::new(),
            )
        };

        for pool in [true, false] {
            new_test_ext().execute_with(|| {
                setup();
                let call = remark();
                let len = call.encoded_size();
                let path = if pool {
                    fee_path_heavier();
                    TestWeights::pool_path()
                } else {
                    pool_path_heavier();
                    exhaust(GROUP_A);
                    fee_path_used(&call)
                };
                // The payment step leaves weight unspent on the path taken.
                assert!(usage_ext().weight(&call).all_gt(path));
                let others = frame_system::CheckWeight::<Test>::new()
                    .weight(&call)
                    .saturating_add(frame_system::WeightReclaim::<Test>::new().weight(&call));

                let before = *System::block_weight().get(DispatchClass::Normal);
                let result = apply_with(&ALICE, call.clone(), extensions());
                assert!(matches!(result, Ok(Ok(_))), "{result:?}");
                let booked = System::block_weight()
                    .get(DispatchClass::Normal)
                    .saturating_sub(before);

                // The call, the other extensions, and what the path taken used.
                assert_eq!(
                    booked,
                    metered(
                        call.get_dispatch_info()
                            .call_weight
                            .saturating_add(others)
                            .saturating_add(path),
                        len
                    )
                );
                if pool {
                    // The pool is charged what the block books.
                    assert_eq!(used(GROUP_A), booked);
                }
            });
        }
    }

    // INV-13, AC-C4.1
    #[test]
    fn ticket_decides_who_pays() {
        new_test_ext().execute_with(|| {
            setup();
            let info = declared(Weight::from_parts(95, 0));
            let charge = pool_booked(&info, 0);
            let post = PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::Yes,
            };

            // A call that releases the signer's membership, and one that cancels the contract.
            assert_ok!(usage_ext().test_run(
                RuntimeOrigin::signed(ALICE),
                &remark(),
                &info,
                0,
                0,
                |_| {
                    use fc_traits_memberships::Manager;
                    MembershipsManager::release(&GROUP_A, &1)?;
                    Ok(post)
                }
            ));
            assert_eq!(used(GROUP_A), charge);

            assert_ok!(usage_ext().test_run(
                RuntimeOrigin::signed(DAVE),
                &remark(),
                &info,
                0,
                0,
                |_| {
                    UsageSubscription::cancel(group_origin(GROUP_A))?;
                    Ok(post)
                }
            ));
            assert_eq!(used(GROUP_A), charge.saturating_add(charge));
            assert_eq!(
                usage_charged()
                    .into_iter()
                    .map(|(group, who, weight, _)| (group, who, weight))
                    .collect::<Vec<_>>(),
                vec![(GROUP_A, ALICE, charge), (GROUP_A, DAVE, charge)]
            );
        });
    }

    // INV-6, INV-13, 0009-A19.4
    #[test]
    fn contract_gone_during_dispatch_is_charged_nothing() {
        new_test_ext().execute_with(|| {
            setup();
            assert_ok!(usage_ext().test_run(
                RuntimeOrigin::signed(ALICE),
                &remark(),
                &declared(Weight::from_parts(95, 0)),
                0,
                0,
                |_| {
                    UsageSubscription::terminate_contract(RuntimeOrigin::root(), GROUP_A)?;
                    Ok(PostDispatchInfo::default())
                }
            ));
            assert!(Contracts::<Test>::get(GROUP_A).is_none());
            assert!(usage_charged().is_empty());
        });
    }

    // REQ-PL-8, INV-13
    #[test]
    fn contract_replaced_during_dispatch_is_charged_nothing() {
        new_test_ext().execute_with(|| {
            setup();
            let other = standard();
            assert_ok!(usage_ext().test_run(
                RuntimeOrigin::signed(ALICE),
                &remark(),
                &declared(Weight::from_parts(95, 0)),
                0,
                0,
                |_| {
                    // Another contract in the group's slot: the ticket names the old one.
                    Contracts::<Test>::mutate(GROUP_A, |contract| {
                        if let Some(contract) = contract {
                            contract.offer = other;
                        }
                    });
                    Ok(PostDispatchInfo::default())
                }
            ));
            assert_eq!(used(GROUP_A), Weight::zero());
            assert!(usage_charged().is_empty());
        });
    }
}

/// Usage windows: where they start, and the allowance each one admits.
mod usage_windows {
    use super::*;

    // AC-C2.2
    #[test]
    fn first_transaction_after_a_window_boundary_is_admitted() {
        new_test_ext().execute_with(|| {
            setup();
            let call = remark();
            let estimate = metered(info(&call).total_weight(), call.encoded_size());
            // What the pool is charged for each of these transactions.
            let charged = pool_booked(&info(&call), call.encoded_size());

            // DAVE, with no balance, exhausts window 0.
            Contracts::<Test>::mutate(GROUP_A, |c| {
                if let Some(c) = c {
                    c.used = ALLOWANCE.saturating_sub(estimate);
                }
            });
            assert!(matches!(apply(&DAVE, remark()), Ok(Ok(_))));
            assert_eq!(
                apply(&DAVE, remark()),
                Err(InvalidTransaction::Payment.into())
            );

            // The window has just ended.
            Clock::set(U);
            let before = storage_root();
            assert!(is_pool(path_of(&DAVE, &info(&call), call.encoded_size())));
            assert_eq!(storage_root(), before, "validation wrote nothing");

            // Validated, prepared, dispatched and post-dispatched, without a panic or a fee.
            let result = apply(&DAVE, remark());
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");
            let contract = Contracts::<Test>::get(GROUP_A).expect("live; qed");
            assert_eq!(contract.window_start, U);
            assert_eq!(contract.used, charged);
        });
    }

    // INV-4
    #[test]
    fn usage_never_exceeds_the_allowance() {
        new_test_ext().execute_with(|| {
            setup();
            let mut seed: u64 = 0x5eed;
            let mut next = move || {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                seed >> 33
            };
            for _ in 0..500 {
                let estimate = Weight::from_parts(
                    next() % (ALLOWANCE.ref_time() / 8),
                    next() % (ALLOWANCE.proof_size() / 8),
                );
                let contract = Contracts::<Test>::get(GROUP_A).expect("live; qed");
                let window = contract.window_at(Clock::now()).expect("anchored; qed");
                let used_before = contract.used_in(window);
                match UsageSubscription::check(&ALICE, estimate) {
                    Ok(ticket) => {
                        assert!(used_before.saturating_add(estimate).all_lte(ALLOWANCE));
                        UsageSubscription::charge(&ticket, estimate);
                    }
                    Err(reason) => {
                        assert_eq!(reason, FeePathReason::AllowanceExceeded);
                        assert!(!used_before.saturating_add(estimate).all_lte(ALLOWANCE));
                    }
                }
                assert!(used(GROUP_A).all_lte(ALLOWANCE));
                // Sometimes, the clock moves on within the paid period.
                if next() % 7 == 0 {
                    Clock::set((Clock::now() + next() % (2 * U)).min(B - 1));
                }
            }
        });
    }

    // INV-5, REQ-PL-1
    #[test]
    fn current_window_contains_now() {
        let mut seed: u64 = 42;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed >> 20
        };
        for _ in 0..10_000 {
            let anchor = next() % 1_000_000;
            let period = 1 + next() % 100_000;
            let now = anchor + next() % 10_000_000;
            let start = window_start(anchor, period, now).expect("now ≥ anchor; qed");
            assert!(start <= now && now < start + period);
            assert_eq!((start - anchor) % period, 0);
            // No window starts before the anchor, nor in the future.
            assert!(start >= anchor);
            // The first window at or after a tick is a window start at or after it.
            let tick = anchor + next() % 10_000_000;
            let first = first_window_from(anchor, period, tick);
            assert!(first >= tick && first < tick + period);
            assert_eq!((first - anchor) % period, 0);
        }
        // Before the anchor there is no window; overflow refuses.
        assert_eq!(window_start(10u64, 5, 9), None);
        assert_eq!(window_start(10u64, 0, 20), None);
    }

    // INV-5, REQ-PL-2, REQ-PL-5
    #[test]
    fn windows_follow_the_clock_through_skips_and_suspension() {
        new_test_ext().execute_with(|| {
            setup();
            let estimate = Weight::from_parts(1_000, 10);
            let mut seed: u64 = 7;
            let mut now = 0;
            for _ in 0..200 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                now = (now + (seed >> 40) % (3 * U)).min(B - 1);
                Clock::set(now);
                let ticket = UsageSubscription::check(&ALICE, estimate).expect("paid; qed");
                assert!(ticket.window_start <= now && now < ticket.window_start + U);
                let used_before = Contracts::<Test>::get(GROUP_A)
                    .map(|c| c.used_in(ticket.window_start))
                    .unwrap_or_default();
                UsageSubscription::charge(&ticket, estimate);
                assert_eq!(
                    used(GROUP_A),
                    used_before.saturating_add(estimate),
                    "usage resets exactly at each boundary"
                );
            }

            // A suspension neither pauses nor shifts windows: after a restore, usage is in whatever
            // window now contains the clock.
            set_funds(GROUP_A, 0);
            advance_to(B);
            set_funds(GROUP_A, GROUP_FUNDS);
            Clock::set(B + 5 * U + 7);
            assert_ok!(Listings::charge_due(
                RuntimeOrigin::signed(BOB),
                fc_pallet_listings::InventoryId(0, 0),
                0,
                group_account(GROUP_A)
            ));
            let ticket = UsageSubscription::check(&ALICE, estimate).expect("restored; qed");
            assert_eq!(ticket.window_start, B + 5 * U);
        });
    }

    // INV-20
    #[test]
    fn amended_allowance_is_admitted_only_from_its_window() {
        new_test_ext().execute_with(|| {
            const U7: u64 = 700;
            let offer = publish(
                OfferKind::Custom(GROUP_A),
                Terms {
                    usage_period: U7,
                    ..terms()
                },
            );
            subscribe(GROUP_A, offer);
            add_member(GROUP_A, 1, &ALICE);
            advance_to(27 * DAYS);
            assert_ok!(UsageSubscription::amend_contract(
                RuntimeOrigin::signed(AMENDER),
                GROUP_A,
                Terms {
                    usage_period: U7,
                    allowance: ALLOWANCE * 2,
                    ..terms()
                }
            ));
            // More than the old allowance, within the new one.
            let estimate = ALLOWANCE.saturating_add(Weight::from_parts(1, 1));
            let from_window = (2 * B).div_ceil(U7) * U7;

            assert_eq!(
                UsageSubscription::check(&ALICE, estimate),
                Err(FeePathReason::AllowanceExceeded)
            );
            advance_to(2 * B);
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate),
                Err(FeePathReason::AllowanceExceeded)
            );
            Clock::set(from_window - 1);
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate),
                Err(FeePathReason::AllowanceExceeded)
            );
            Clock::set(from_window);
            assert!(UsageSubscription::check(&ALICE, estimate).is_ok());
        });
    }

    // INV-20
    #[test]
    fn offer_amendment_is_admitted_only_from_its_window() {
        new_test_ext().execute_with(|| {
            let offer = standard();
            subscribe(GROUP_A, offer);
            add_member(GROUP_A, 1, &ALICE);
            advance_to(27 * DAYS);
            assert_ok!(UsageSubscription::amend_offer(
                RuntimeOrigin::signed(AMENDER),
                offer,
                Terms {
                    allowance: ALLOWANCE * 2,
                    ..terms()
                }
            ));
            let estimate = ALLOWANCE.saturating_add(Weight::from_parts(1, 1));

            // Read lazily from the offer's amendment: the old allowance before b.
            advance_to(B);
            Clock::set(2 * B - 1);
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate),
                Err(FeePathReason::AllowanceExceeded)
            );
            advance_to(2 * B);
            assert!(UsageSubscription::check(&ALICE, estimate).is_ok());
        });
    }
}

/// The conditions of admission (`REQ-PL-7`): one test per condition failing.
mod fee_path_conditions {
    use super::*;

    fn estimate() -> Weight {
        Weight::from_parts(1_000, 10)
    }

    // REQ-PL-7 (condition 1)
    #[test]
    fn origin_not_signed() {
        new_test_ext().execute_with(|| {
            setup();
            // The fee path decides, exactly as it would alone.
            for origin in [RuntimeOrigin::root(), RuntimeOrigin::none()] {
                let info = declared(Weight::from_parts(10, 0));
                let wrapped = usage_ext().validate_only(
                    origin.clone(),
                    &remark(),
                    &info,
                    0,
                    TransactionSource::External,
                    0,
                );
                let alone = ChargeTransactionPayment::<Test>::from(0).validate_only(
                    origin,
                    &remark(),
                    &info,
                    0,
                    TransactionSource::External,
                    0,
                );
                match (wrapped, alone) {
                    (Ok((_, val, _)), Ok(_)) => assert!(matches!(val, Path::Fee(_))),
                    (Err(wrapped), Err(alone)) => assert_eq!(wrapped, alone),
                    _ => panic!("the paths differ"),
                }
            }
        });
    }

    // REQ-PL-7 (condition 2)
    #[test]
    fn no_paying_group() {
        new_test_ext().execute_with(|| {
            setup();
            // No membership.
            assert_eq!(
                UsageSubscription::check(&BOB, estimate()),
                Err(FeePathReason::NoPayingGroup)
            );
            assert_eq!(
                UsageSubscription::resolve_paying_group(&BOB),
                PayingGroupResolution::NoMembership
            );
            // `AC-C3.1`: several groups, none named.
            subscribe(GROUP_B, standard());
            add_member(GROUP_B, 10, &ALICE);
            assert_eq!(
                UsageSubscription::resolve_paying_group(&ALICE),
                PayingGroupResolution::SeveralGroups
            );
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate()),
                Err(FeePathReason::NoPayingGroup)
            );
            // `REQ-PC-3`: more memberships than the bound, all of one group.
            for m in 20..20 + MaxMembershipScan::get() {
                add_member(GROUP_A, m, &CHARLIE);
            }
            assert!(UsageSubscription::check(&CHARLIE, estimate()).is_ok());
            add_member(GROUP_A, 30, &CHARLIE);
            assert_eq!(
                UsageSubscription::resolve_paying_group(&CHARLIE),
                PayingGroupResolution::TooManyMemberships
            );
            assert_eq!(
                UsageSubscription::check(&CHARLIE, estimate()),
                Err(FeePathReason::NoPayingGroup)
            );
        });
    }

    // REQ-PL-7 (condition 3), REQ-MI-13
    #[test]
    fn group_account_is_never_a_member() {
        new_test_ext().execute_with(|| {
            setup();
            // The group account holds the group's stock, which is no membership.
            use fc_traits_memberships::Issue;
            assert_ok!(MembershipsManager::issue(&GROUP_A, &50));
            assert_eq!(
                UsageSubscription::check(&group_account(GROUP_A), estimate()),
                Err(FeePathReason::NoPayingGroup)
            );
            // A named group the member left is ignored, never deleted (`AC-C3.2`, `REQ-PC-4`).
            assert_ok!(UsageSubscription::set_paying_group(
                RuntimeOrigin::signed(ALICE),
                Some(GROUP_A)
            ));
            use fc_traits_memberships::Manager;
            assert_ok!(MembershipsManager::release(&GROUP_A, &1));
            let before = storage_root();
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate()),
                Err(FeePathReason::NoPayingGroup)
            );
            assert_eq!(storage_root(), before);
            assert_eq!(
                PayingGroup::<Test>::get(&ALICE).and_then(|choice| choice.group),
                Some(GROUP_A)
            );
        });
    }

    // REQ-PL-7 (condition 4), REQ-GR-4
    #[test]
    fn group_not_usable() {
        new_test_ext().execute_with(|| {
            setup();
            UsableGroups::set_unusable(GROUP_A);
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate()),
                Err(FeePathReason::GroupUnusable)
            );
        });
    }

    // REQ-PL-7 (condition 5)
    #[test]
    fn no_active_paid_contract() {
        new_test_ext().execute_with(|| {
            // No contract.
            add_member(GROUP_A, 1, &ALICE);
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate()),
                Err(FeePathReason::NoContract)
            );

            // `AC-D2.1`, `REQ-CT-4`: unpaid at d, before any bookkeeping has run.
            subscribe(GROUP_A, standard());
            set_funds(GROUP_A, 0);
            Clock::set(B - 1);
            assert!(UsageSubscription::check(&ALICE, estimate()).is_ok());
            Clock::set(B);
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate()),
                Err(FeePathReason::NotPaid)
            );

            // *Suspended*.
            advance_to(B);
            assert!(matches!(
                subscription(GROUP_A).map(|s| s.state),
                Some(SubscriptionState::Suspended { .. })
            ));
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate()),
                Err(FeePathReason::NotPaid)
            );
        });
    }

    // REQ-PL-7 (condition 5)
    #[test]
    fn defaulted_contract() {
        new_test_ext().execute_with(|| {
            add_member(GROUP_A, 1, &ALICE);
            subscribe(GROUP_A, custom(GROUP_A, Some(3), None));
            set_funds(GROUP_A, 0);
            advance_to(B);
            advance_to(B + GRACE);
            assert!(matches!(
                subscription(GROUP_A).map(|s| s.state),
                Some(SubscriptionState::Defaulted { .. })
            ));
            assert_eq!(
                UsageSubscription::check(&ALICE, estimate()),
                Err(FeePathReason::NotPaid)
            );
        });
    }

    // REQ-PL-7 (condition 6), REQ-PL-12
    #[test]
    fn pool_cannot_cover_the_estimate() {
        new_test_ext().execute_with(|| {
            setup();
            // Either component alone refuses it, even though a smaller one would fit.
            for excess in [
                Weight::from_parts(ALLOWANCE.ref_time() + 1, 0),
                Weight::from_parts(0, ALLOWANCE.proof_size() + 1),
            ] {
                assert_eq!(
                    UsageSubscription::check(&ALICE, excess),
                    Err(FeePathReason::AllowanceExceeded)
                );
            }
            assert!(UsageSubscription::check(&ALICE, ALLOWANCE).is_ok());
        });
    }
}

/// Naming a paying group, and a member with no balance transacting through `CheckNonce`.
mod paying_group {
    use super::*;

    fn name(group: Option<u32>) -> RuntimeCall {
        RuntimeCall::UsageSubscription(crate::Call::set_paying_group { group })
    }

    // AC-C3.2
    #[test]
    fn named_group_pays_and_a_stale_name_is_ignored() {
        new_test_ext().execute_with(|| {
            setup();
            subscribe(GROUP_B, standard());
            add_member(GROUP_B, 10, &ALICE);
            assert_eq!(
                UsageSubscription::check(&ALICE, Weight::from_parts(10, 0)),
                Err(FeePathReason::NoPayingGroup)
            );
            clear_events();

            assert_ok!(UsageSubscription::set_paying_group(
                RuntimeOrigin::signed(ALICE),
                Some(GROUP_B)
            ));
            assert_eq!(
                events(),
                vec![crate::Event::PayingGroupSet {
                    who: ALICE,
                    group: Some(GROUP_B)
                }]
            );
            assert!(matches!(apply(&ALICE, remark()), Ok(Ok(_))));
            assert!(used(GROUP_B).any_gt(Weight::zero()));
            assert_eq!(used(GROUP_A), Weight::zero());

            // ALICE leaves group B: the name is ignored, and both groups are left, so the fee path.
            use fc_traits_memberships::Manager;
            assert_ok!(MembershipsManager::release(&GROUP_B, &10));
            assert_eq!(
                UsageSubscription::resolve_paying_group(&ALICE),
                PayingGroupResolution::Only(GROUP_A)
            );
            add_member(GROUP_C, 20, &ALICE);
            assert_eq!(
                UsageSubscription::check(&ALICE, Weight::from_parts(10, 0)),
                Err(FeePathReason::NoPayingGroup)
            );
        });
    }

    // REQ-PC-4
    #[test]
    fn naming_needs_a_valid_membership() {
        new_test_ext().execute_with(|| {
            setup();
            assert_noop!(
                UsageSubscription::set_paying_group(RuntimeOrigin::signed(ALICE), Some(GROUP_B)),
                Error::<Test>::NotAMember
            );
            assert_noop!(
                UsageSubscription::set_paying_group(
                    RuntimeOrigin::signed(group_account(GROUP_A)),
                    Some(GROUP_A)
                ),
                Error::<Test>::NotAMember
            );
            // Clearing needs none.
            assert_ok!(UsageSubscription::set_paying_group(
                RuntimeOrigin::signed(BOB),
                None
            ));
        });
    }

    /// The guide's order: `CheckNonce`, `CheckWeight`, then the payment step inside
    /// `SkipCheckIfFeeless`.
    type GuideTxExtensions = (
        frame_system::CheckNonce<Test>,
        frame_system::CheckWeight<Test>,
        pallet_skip_feeless_payment::SkipCheckIfFeeless<Test, UsageExtension>,
    );

    fn guide_ext(nonce: u32) -> GuideTxExtensions {
        (
            frame_system::CheckNonce::from(nonce),
            frame_system::CheckWeight::new(),
            usage_ext().into(),
        )
    }

    /// The fee payments `SkipCheckIfFeeless` skipped, by signer.
    fn fees_skipped() -> Vec<AccountId> {
        System::events()
            .into_iter()
            .filter_map(|record| match record.event {
                RuntimeEvent::SkipFeelessPayment(
                    pallet_skip_feeless_payment::Event::FeeSkipped {
                        origin: OriginCaller::system(frame_system::RawOrigin::Signed(who)),
                    },
                ) => Some(who),
                _ => None,
            })
            .collect()
    }

    // AC-C3.3, REQ-PC-5
    #[test]
    fn naming_is_free_within_the_rate_limit() {
        new_test_ext().execute_with(|| {
            setup();
            subscribe(GROUP_B, standard());
            add_member(GROUP_B, 10, &DAVE);
            // DAVE has no balance, and a provider for `CheckNonce`.
            System::inc_providers(&DAVE);
            let origin = RuntimeOrigin::signed(DAVE);

            // Each free change, through the guide's extensions: `SkipCheckIfFeeless` skips the
            // payment step, so it takes no fee and no pool.
            let mut nonce = 0;
            for (n, group) in [Some(GROUP_A), Some(GROUP_B), None].into_iter().enumerate() {
                let call = name(group);
                assert!(call.is_feeless(&origin), "change {n} is free");
                let result = apply_with(&DAVE, call, guide_ext(nonce));
                assert!(matches!(result, Ok(Ok(_))), "change {n}: {result:?}");
                nonce += 1;
            }
            assert_eq!(fees_skipped(), vec![DAVE; 3]);
            assert_eq!(Balances::free_balance(&DAVE), 0);
            assert_eq!(used(GROUP_A), Weight::zero());
            assert_eq!(used(GROUP_B), Weight::zero());
            assert_eq!(
                PayingGroup::<Test>::get(&DAVE),
                Some(PayingGroupChoice {
                    group: None,
                    window: 0,
                    changes: 3
                })
            );

            // Beyond the limit, an ordinary transaction: the payment step runs, and DAVE, a
            // member of two groups with none named, takes the fee path and cannot pay.
            assert!(!name(Some(GROUP_A)).is_feeless(&origin));
            assert_eq!(
                apply_with(&DAVE, name(Some(GROUP_A)), guide_ext(nonce)),
                Err(InvalidTransaction::Payment.into())
            );
            // A name the signer holds no membership of is never free.
            assert!(!name(Some(GROUP_C)).is_feeless(&origin));
            // Nor is clearing when nothing was named.
            assert!(!name(None).is_feeless(&RuntimeOrigin::signed(ALICE)));

            // The next rate window, counted from tick 0.
            Clock::set(MinUsagePeriod::get());
            assert!(name(Some(GROUP_A)).is_feeless(&origin));
            let result = apply_with(&DAVE, name(Some(GROUP_A)), guide_ext(nonce));
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");
            assert_eq!(fees_skipped().len(), 4);
        });
    }

    // REQ-PL-10, REQ-PC-5
    #[test]
    fn member_with_a_provider_and_no_balance_passes_check_nonce() {
        new_test_ext().execute_with(|| {
            setup();
            subscribe(GROUP_B, standard());
            // DAVE holds memberships and no balance at all. A provider, as `fc-pallet-pass` gives
            // its accounts, is what `CheckNonce` asks for.
            System::inc_providers(&DAVE);
            assert_eq!(Balances::free_balance(&DAVE), 0);

            // The pool path, through `CheckNonce`, `CheckWeight` and `SkipCheckIfFeeless`.
            let result = apply_with(&DAVE, remark(), guide_ext(0));
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");
            assert!(used(GROUP_A).any_gt(Weight::zero()));
            assert!(fees_skipped().is_empty());
            assert_eq!(System::account_nonce(&DAVE), 1);

            // A member of two groups names one for free: `SkipCheckIfFeeless` skips the payment
            // step, and `CheckNonce` and `CheckWeight` still run.
            add_member(GROUP_B, 10, &DAVE);
            let call = name(Some(GROUP_B));
            assert!(call.is_feeless(&RuntimeOrigin::signed(DAVE)));
            let before = (used(GROUP_A), used(GROUP_B));
            let result = apply_with(&DAVE, call, guide_ext(1));
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");
            assert_eq!(fees_skipped(), vec![DAVE]);
            assert_eq!((used(GROUP_A), used(GROUP_B)), before);
            assert_eq!(
                PayingGroup::<Test>::get(&DAVE).and_then(|choice| choice.group),
                Some(GROUP_B)
            );
            assert_eq!(System::account_nonce(&DAVE), 2);

            // The named group pays from then on.
            let before = used(GROUP_A);
            let result = apply_with(&DAVE, remark(), guide_ext(2));
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");
            assert!(used(GROUP_B).any_gt(Weight::zero()));
            assert_eq!(used(GROUP_A), before);
            assert_eq!(Balances::free_balance(&DAVE), 0);
        });
    }

    // REQ-PL-10
    #[test]
    fn check_nonce_refuses_a_member_with_no_provider_nor_sufficient() {
        new_test_ext().execute_with(|| {
            setup();
            // DAVE's memberships give it no provider: the pool would admit it, `CheckNonce` not.
            assert_eq!(System::providers(&DAVE), 0);
            assert_eq!(System::sufficients(&DAVE), 0);
            assert!(UsageSubscription::check(&DAVE, Weight::from_parts(10, 0)).is_ok());

            assert_eq!(
                apply_with(&DAVE, remark(), guide_ext(0)),
                Err(InvalidTransaction::Payment.into())
            );
            assert_eq!(used(GROUP_A), Weight::zero());
        });
    }

    // REQ-PC-3, NFR-1, DEC-7, 0009-A1: the stock a group account holds counts against the bound.
    #[test]
    fn a_signing_group_account_with_stock_at_the_bound_takes_the_fee_path() {
        new_test_ext().execute_with(|| {
            use fc_traits_memberships::Issue;
            setup();
            let account = group_account(GROUP_A);
            // GROUP_A's account is a member of GROUP_B, which has a contract.
            subscribe(GROUP_B, standard());
            add_member(GROUP_B, 10, &account);
            assert_eq!(
                UsageSubscription::resolve_paying_group(&account),
                PayingGroupResolution::Only(GROUP_B)
            );

            // Its stock and its membership together within the bound: GROUP_B is found.
            for m in 50..50 + MaxMembershipScan::get() - 1 {
                assert_ok!(MembershipsManager::issue(&GROUP_A, &m));
            }
            assert_eq!(
                UsageSubscription::resolve_paying_group(&account),
                PayingGroupResolution::Only(GROUP_B)
            );
            assert!(UsageSubscription::check(&account, Weight::from_parts(10, 0)).is_ok());

            // One more item of stock, which is no membership, and the reads exceed the bound.
            assert_ok!(MembershipsManager::issue(&GROUP_A, &60));
            assert_eq!(
                UsageSubscription::resolve_paying_group(&account),
                PayingGroupResolution::TooManyMemberships
            );
            assert_eq!(
                UsageSubscription::check(&account, Weight::from_parts(10, 0)),
                Err(FeePathReason::NoPayingGroup)
            );
        });
    }

    // REQ-PC-3, NFR-1, 0009-A1
    #[test]
    fn a_member_past_the_bound_is_resolved_from_a_bounded_number_of_reads() {
        let mut ext = new_test_ext();
        ext.execute_with(|| {
            setup();
            for m in 100..300 {
                add_member(GROUP_A, m, &CHARLIE);
            }
            assert_eq!(
                UsageSubscription::resolve_paying_group(&CHARLIE),
                PayingGroupResolution::TooManyMemberships
            );
            // The usual case, beside it: one membership, one group.
            assert_eq!(
                UsageSubscription::resolve_paying_group(&ALICE),
                PayingGroupResolution::Only(GROUP_A)
            );
        });
        ext.commit_all().unwrap();

        let (_, bounded) =
            ext.execute_and_prove(|| UsageSubscription::resolve_paying_group(&CHARLIE));
        let (_, every) = ext.execute_and_prove(|| {
            <<Test as Config>::Memberships as fc_traits_memberships::Inspect<_>>::user_memberships(
                &CHARLIE, None,
            )
            .count()
        });
        assert!(
            bounded.encoded_size() * 3 < every.encoded_size(),
            "{} vs {}",
            bounded.encoded_size(),
            every.encoded_size()
        );
    }
}

/// The fee path, the metadata and the encoding are exactly the fee extension's.
mod transparency {
    use super::*;

    // INV-6, INV-12, REQ-PL-11, CTR-FEE-5
    #[test]
    fn fee_path_is_exactly_the_inner_extensions() {
        new_test_ext().execute_with(|| {
            setup();
            // The pool path weighs more: the fee path leaves some of the declared weight unspent.
            pool_path_heavier();
            let call = remark();
            let wrapper = || UsageExtension::new(ChargeTransactionPayment::from(7));
            let info = DispatchInfo {
                extension_weight: wrapper().weight(&call),
                ..declared(Weight::from_parts(1_000, 0))
            };
            assert!(info.extension_weight.all_gt(fee_path_used(&call)));
            let post = PostDispatchInfo {
                actual_weight: Some(Weight::from_parts(400, 0)),
                pays_fee: Pays::Yes,
            };

            // The pool path takes no fee, nor the tip.
            let before = Balances::free_balance(&ALICE);
            assert_ok!(wrapper().test_run(
                RuntimeOrigin::signed(ALICE),
                &call,
                &info,
                10,
                0,
                |_| Ok(post)
            ));
            assert_eq!(Balances::free_balance(&ALICE), before);
            assert!(used(GROUP_A).any_gt(Weight::zero()));

            // ALICE's pool is exhausted: the fee path, after the full check, with a tip, through
            // the wrapper.
            exhaust(GROUP_A);
            let before = Balances::free_balance(&ALICE);
            let result = wrapper()
                .test_run(RuntimeOrigin::signed(ALICE), &call, &info, 10, 0, |_| {
                    Ok(post)
                })
                .expect("valid; qed")
                .expect("dispatched; qed");
            let wrapped = before - Balances::free_balance(&ALICE);
            // The block keeps the call's actual weight and what the fee path used.
            assert_eq!(
                result.actual_weight,
                Some(Weight::from_parts(400, 0).saturating_add(fee_path_used(&call)))
            );

            // Then through the fee extension alone, declaring what the fee path used.
            let alone_info = DispatchInfo {
                extension_weight: fee_path_used(&call),
                ..info
            };
            let before = Balances::free_balance(&ALICE);
            assert_ok!(ChargeTransactionPayment::<Test>::from(7).test_run(
                RuntimeOrigin::signed(ALICE),
                &call,
                &alone_info,
                10,
                0,
                |_| Ok(post)
            ));
            let alone = before - Balances::free_balance(&ALICE);

            // The signer pays what the fee extension alone charges, and nothing for the weight
            // the fee path left unspent.
            assert!(wrapped > 0);
            assert_eq!(wrapped, alone);
            // The pool was not charged.
            assert_eq!(used(GROUP_A), ALLOWANCE);
        });
    }

    #[test]
    fn transparent_metadata_and_encoding() {
        use pallet_transaction_payment::ChargeTransactionPayment as Inner;

        assert_eq!(
            <UsageExtension as TransactionExtension<RuntimeCall>>::IDENTIFIER,
            <Inner<Test> as TransactionExtension<RuntimeCall>>::IDENTIFIER
        );
        let outer = <TxExtensions as TransactionExtension<RuntimeCall>>::metadata();
        let inner = <InnerTxExtensions as TransactionExtension<RuntimeCall>>::metadata();
        assert_eq!(outer.len(), inner.len());
        for (outer, inner) in outer.iter().zip(inner.iter()) {
            assert_eq!(outer.identifier, inner.identifier);
            assert_eq!(outer.ty, inner.ty);
            assert_eq!(outer.implicit, inner.implicit);
        }
        assert_eq!(
            scale_info::meta_type::<UsageExtension>(),
            scale_info::meta_type::<Inner<Test>>()
        );
        // Inside `SkipCheckIfFeeless`, as the guide requires, the same holds.
        type Skipping<E> = pallet_skip_feeless_payment::SkipCheckIfFeeless<Test, E>;
        assert_eq!(
            <Skipping<UsageExtension> as TransactionExtension<RuntimeCall>>::IDENTIFIER,
            <Skipping<Inner<Test>> as TransactionExtension<RuntimeCall>>::IDENTIFIER
        );
        let outer = <Skipping<UsageExtension> as TransactionExtension<RuntimeCall>>::metadata();
        let inner = <Skipping<Inner<Test>> as TransactionExtension<RuntimeCall>>::metadata();
        assert_eq!(outer.len(), inner.len());
        for (outer, inner) in outer.iter().zip(inner.iter()) {
            assert_eq!(outer.identifier, inner.identifier);
            assert_eq!(outer.ty, inner.ty);
            assert_eq!(outer.implicit, inner.implicit);
        }
        assert_eq!(
            <Skipping<UsageExtension> as scale_info::TypeInfo>::type_info(),
            <Inner<Test> as scale_info::TypeInfo>::type_info()
        );

        // An extrinsic's bytes are the same with either.
        let signature =
            sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([7u8; 64]));
        let with_wrapper = UncheckedExtrinsic::new_signed(
            remark(),
            ALICE,
            signature.clone(),
            (frame_system::CheckWeight::new(), usage_ext()),
        )
        .encode();
        let without = sp_runtime::generic::UncheckedExtrinsic::<
            AccountId,
            RuntimeCall,
            sp_runtime::MultiSignature,
            InnerTxExtensions,
        >::new_signed(
            remark(),
            ALICE,
            signature,
            (frame_system::CheckWeight::new(), Inner::from(0)),
        )
        .encode();
        assert_eq!(with_wrapper, without);
    }
}
