use codec::Encode;
use frame::deps::frame_support::dispatch::{DispatchInfo, Pays, PostDispatchInfo};
use frame::{
    deps::{
        sp_io,
        sp_runtime::{
            generic::ExtrinsicFormat,
            traits::{Applyable, DispatchTransaction, TransactionExtension},
            transaction_validity::TransactionSource,
            StateVersion,
        },
    },
    testing_prelude::{
        assert_noop, assert_ok, frame_system, parameter_types, InvalidTransaction,
        TransactionValidityError, Weight,
    },
};
use pallet_transaction_payment::ChargeTransactionPayment;

use crate::mock::{
    fc_pallet_gas_transaction_payment, new_test_ext, AccountId, Balances, CheckedExtrinsic,
    RuntimeCall, RuntimeEvent, RuntimeOrigin, System, Tank, Test, TxExtensions, BASE_EXTRINSIC,
    TANK_PROOF_SIZE,
};
use crate::{Pre, WeightInfo, PATH_MISMATCH};

const ALICE: AccountId = 1;
const BOB: AccountId = 2;

parameter_types! {
  pub ChargeTx: TxExtensions = fc_pallet_gas_transaction_payment::ChargeTransactionPayment::new(
    ChargeTransactionPayment::from(0),
  );
  pub Call: RuntimeCall = RuntimeCall::System(frame_system::Call::remark {
    remark: b"Hello world".to_vec(),
  });
}

/// The metered `ref_time` of a transaction declaring `call_weight` and no extension weight.
fn metered(call_weight: u64) -> u64 {
    call_weight + BASE_EXTRINSIC.ref_time()
}

fn info(call_weight: u64) -> DispatchInfo {
    DispatchInfo {
        call_weight: Weight::from_parts(call_weight, 0),
        ..Default::default()
    }
}

fn storage_root() -> Vec<u8> {
    sp_io::storage::root(StateVersion::V1)
}

fn test_run(
    who: AccountId,
    call: &RuntimeCall,
    call_weight: Weight,
) -> <TxExtensions as DispatchTransaction<RuntimeCall>>::Result {
    let test_di = DispatchInfo {
        call_weight,
        ..Default::default()
    };

    ChargeTx::get().test_run(
        RuntimeOrigin::signed(who),
        call,
        &test_di,
        call.encoded_size(),
        0,
        |_| Ok(().into()),
    )
}

fn gas_burned_events() -> Vec<(AccountId, Weight)> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::GasTransactionPayment(
                fc_pallet_gas_transaction_payment::Event::GasBurned { who, remaining },
            ) => Some((who, remaining)),
            _ => None,
        })
        .collect()
}

mod charge_transaction_payment_pre_dispatch {
    use super::*;

    #[test]
    fn fails_if_both_burner_and_inner_transaction_payments_fail() {
        new_test_ext(vec![(ALICE, 0)]).execute_with(|| {
            let call = Call::get();

            assert_noop!(
                test_run(ALICE, &call, Weight::from_parts(2, 0)),
                InvalidTransaction::Payment
            );
        });

        new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
            let call = Call::get();

            assert_noop!(
                test_run(BOB, &call, Weight::from_parts(2, 0)),
                InvalidTransaction::Payment
            );
        });
    }

    #[test]
    fn it_works_if_inner_transaction_payment_works() {
        let call = Call::get();

        new_test_ext(vec![(ALICE, 3)]).execute_with(|| {
            assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, 3));
            assert_ok!(test_run(BOB, &call, Weight::from_parts(3, 0)));
            assert_eq!(Balances::free_balance(BOB), 1);
        });
    }

    #[test]
    fn it_works_if_burner_works() {
        // Works for single remark
        new_test_ext(vec![(ALICE, metered(1))]).execute_with(|| {
            assert_ok!(test_run(ALICE, &Call::get(), Weight::from_parts(1, 0)));
        });

        // Works for remarks batch
        new_test_ext(vec![(ALICE, metered(1) + metered(3))]).execute_with(|| {
            assert_ok!(test_run(ALICE, &Call::get(), Weight::from_parts(1, 0)));
            assert_ok!(test_run(ALICE, &Call::get(), Weight::from_parts(3, 0)));
        });
    }
}

mod charge_transaction_payment_usage_with_checked_extrinsic {
    use super::*;

    fn assert_applied_extrinsic<const S: usize>(xt: CheckedExtrinsic) {
        let info = info(S as u64);
        let call_encoded_len = xt.function.encode().len();
        assert_ok!(xt.apply::<Test>(&info, call_encoded_len));
    }
    fn assert_failed_extrinsic<const S: usize>(
        xt: CheckedExtrinsic,
        error: TransactionValidityError,
    ) {
        let info = info(S as u64);
        let call_encoded_len = xt.function.encode().len();
        assert_noop!(xt.apply::<Test>(&info, call_encoded_len), error);
    }

    #[test]
    fn validates_single_extrinsic() {
        new_test_ext(vec![(ALICE, metered(1))]).execute_with(|| {
            assert_applied_extrinsic::<1>(CheckedExtrinsic {
                format: ExtrinsicFormat::Signed(ALICE, ChargeTx::get()),
                function: Call::get(),
            });
            assert_eq!(
                Tank::get(ALICE).map(|tank| tank.ref_time()),
                Some(0),
                "the tank paid the whole metered weight"
            );
        });
    }

    #[test]
    fn validates_multiple_extrinsics() {
        new_test_ext(vec![(ALICE, metered(1) + metered(2))]).execute_with(|| {
            assert_applied_extrinsic::<1>(CheckedExtrinsic {
                format: ExtrinsicFormat::Signed(ALICE, ChargeTx::get()),
                function: Call::get(),
            });
            assert_applied_extrinsic::<2>(CheckedExtrinsic {
                format: ExtrinsicFormat::Signed(ALICE, ChargeTx::get()),
                function: Call::get(),
            });

            assert_failed_extrinsic::<1>(
                CheckedExtrinsic {
                    format: ExtrinsicFormat::Signed(ALICE, ChargeTx::get()),
                    function: Call::get(),
                },
                InvalidTransaction::Payment.into(),
            );
        });
    }
}

/// Validation, preparation and post-dispatch agree, and charge what `CheckWeight` books.
mod charge_transaction_payment_phases {
    use super::*;

    // INV-1
    #[test]
    fn validation_writes_nothing() {
        new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
            assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, 10));
            let call = Call::get();
            let len = call.encoded_size();
            let before = storage_root();

            // The tank's path, and the fee path.
            for who in [ALICE, BOB] {
                let (_, val, _) = ChargeTx::get()
                    .validate_only(
                        RuntimeOrigin::signed(who),
                        &call,
                        &info(10),
                        len,
                        TransactionSource::External,
                        0,
                    )
                    .expect("both paths are valid; qed");
                assert_eq!(val.is_none(), who == ALICE);
            }

            assert_eq!(storage_root(), before);
        });
    }

    // INV-2, INV-15
    #[test]
    fn prepare_rejects_when_the_burner_no_longer_covers_the_transaction() {
        new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
            let call = Call::get();
            let len = call.encoded_size();
            let origin = RuntimeOrigin::signed(ALICE);

            let (_, val, origin) = ChargeTx::get()
                .validate_only(origin, &call, &info(10), len, TransactionSource::InBlock, 0)
                .expect("the tank covers it; qed");
            assert!(val.is_none(), "validation chose the tank");

            // State changes between the phases: the tank no longer covers the transaction.
            Tank::insert(ALICE, Weight::zero());

            assert_eq!(
                ChargeTx::get()
                    .prepare(val, &origin, &call, &info(10), len)
                    .map(|_| ()),
                Err(InvalidTransaction::Custom(PATH_MISMATCH).into())
            );
        });
    }

    // INV-15
    #[test]
    fn prepare_rejects_an_unsigned_origin_without_panicking() {
        new_test_ext(vec![]).execute_with(|| {
            let call = Call::get();
            for origin in [RuntimeOrigin::root(), RuntimeOrigin::none()] {
                assert_eq!(
                    ChargeTx::get()
                        .prepare(None, &origin, &call, &info(10), 0)
                        .map(|_| ()),
                    Err(InvalidTransaction::Custom(PATH_MISMATCH).into())
                );
            }
        });
    }

    // INV-2
    #[test]
    fn fee_path_never_asks_the_burner_again() {
        new_test_ext(vec![]).execute_with(|| {
            assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, 10));
            let call = Call::get();
            let len = call.encoded_size();

            let (_, val, origin) = ChargeTx::get()
                .validate_only(
                    RuntimeOrigin::signed(BOB),
                    &call,
                    &info(10),
                    len,
                    TransactionSource::InBlock,
                    0,
                )
                .expect("BOB can pay the fee; qed");
            assert!(val.is_some(), "validation chose the fee path");

            // A tank appears between the phases. Preparation still takes the fee path.
            Tank::insert(BOB, Weight::from_parts(1_000, TANK_PROOF_SIZE));
            let pre = ChargeTx::get()
                .prepare(val, &origin, &call, &info(10), len)
                .expect("the fee path was validated; qed");

            assert!(matches!(pre, Pre::Inner(_)));
            assert!(Balances::free_balance(BOB) < 10, "the fee was withdrawn");
            assert_eq!(
                Tank::get(BOB),
                Some(Weight::from_parts(1_000, TANK_PROOF_SIZE))
            );
        });
    }

    // INV-3
    #[test]
    fn charges_the_actual_weight_not_the_estimate() {
        new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
            // Estimate: 95 declared for the call + 5 base = 100. Actual: 5 + 5 = 10.
            let call = Call::get();
            let info = info(100 - BASE_EXTRINSIC.ref_time());
            assert_ok!(ChargeTx::get().test_run(
                RuntimeOrigin::signed(ALICE),
                &call,
                &info,
                0,
                0,
                |_| Ok(PostDispatchInfo {
                    actual_weight: Some(Weight::from_parts(5, 0)),
                    pays_fee: Pays::Yes,
                }),
            ));

            let charged = Weight::from_parts(1_000, TANK_PROOF_SIZE)
                .saturating_sub(Tank::get(ALICE).unwrap_or_default());
            assert_eq!(charged, Weight::from_parts(10, 0));
            assert_eq!(
                gas_burned_events(),
                vec![(ALICE, Weight::from_parts(990, TANK_PROOF_SIZE))]
            );
        });
    }

    // INV-3, REQ-PL-6
    #[test]
    fn charge_counts_length_as_proof_size_and_is_capped_at_the_estimate() {
        new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
            let call = Call::get();
            let len = call.encoded_size();
            let info = info(100);
            let estimate = Weight::from_parts(metered(100), len as u64);

            // The call reports more than it declared: the charge is still the estimate.
            assert_ok!(ChargeTx::get().test_run(
                RuntimeOrigin::signed(ALICE),
                &call,
                &info,
                len,
                0,
                |_| Ok(PostDispatchInfo {
                    actual_weight: Some(Weight::from_parts(10_000, 10_000)),
                    pays_fee: Pays::Yes,
                }),
            ));

            assert_eq!(
                Tank::get(ALICE),
                Some(Weight::from_parts(1_000, TANK_PROOF_SIZE).saturating_sub(estimate))
            );
        });
    }

    // INV-11
    #[test]
    fn call_weight_is_not_reported_as_unspent() {
        new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
            let call = Call::get();
            let info = DispatchInfo {
                call_weight: Weight::from_parts(50, 0),
                extension_weight: Weight::from_parts(7, 0),
                ..Default::default()
            };

            let post_info = ChargeTx::get()
                .test_run(RuntimeOrigin::signed(ALICE), &call, &info, 0, 0, |_| {
                    Ok(PostDispatchInfo {
                        actual_weight: Some(Weight::from_parts(20, 0)),
                        pays_fee: Pays::Yes,
                    })
                })
                .expect("the tank pays; qed")
                .expect("the call succeeds; qed");

            // The call's 20 plus the extensions' 7 stay booked; nothing is refunded.
            assert_eq!(post_info.actual_weight, Some(Weight::from_parts(27, 0)));
            assert_eq!(gas_burned_events().len(), 1);
        });
    }

    // REQ-PL-9
    #[test]
    fn pays_no_charges_nothing() {
        new_test_ext(vec![(ALICE, 1_000)]).execute_with(|| {
            let call = Call::get();

            // The call reports no fee after dispatch.
            assert_ok!(ChargeTx::get().test_run(
                RuntimeOrigin::signed(ALICE),
                &call,
                &info(10),
                0,
                0,
                |_| Ok(PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::No,
                }),
            ));
            // The call is declared as paying no fee.
            assert_ok!(ChargeTx::get().test_run(
                RuntimeOrigin::signed(ALICE),
                &call,
                &DispatchInfo {
                    pays_fee: Pays::No,
                    ..info(10)
                },
                0,
                0,
                |_| Ok(().into()),
            ));

            assert_eq!(
                Tank::get(ALICE),
                Some(Weight::from_parts(1_000, TANK_PROOF_SIZE))
            );
            assert!(gas_burned_events().is_empty());
        });
    }

    // CTR-FEE-6
    #[test]
    fn declared_weight_includes_inner_extension() {
        let call = Call::get();
        let inner = ChargeTransactionPayment::<Test>::from(0).weight(&call);

        assert_ne!(inner, Weight::zero());
        assert_eq!(
            ChargeTx::get().weight(&call),
            <() as WeightInfo>::charge_transaction_payment().saturating_add(inner)
        );
    }

    #[test]
    fn metadata_and_identifier_are_the_inner_extensions() {
        type Inner = ChargeTransactionPayment<Test>;

        assert_eq!(
            <TxExtensions as TransactionExtension<RuntimeCall>>::IDENTIFIER,
            <Inner as TransactionExtension<RuntimeCall>>::IDENTIFIER
        );

        let outer = <TxExtensions as TransactionExtension<RuntimeCall>>::metadata();
        let inner = <Inner as TransactionExtension<RuntimeCall>>::metadata();
        assert_eq!(outer.len(), inner.len());
        for (outer, inner) in outer.iter().zip(inner.iter()) {
            assert_eq!(outer.identifier, inner.identifier);
            assert_eq!(outer.ty, inner.ty);
            assert_eq!(outer.implicit, inner.implicit);
        }
        assert_eq!(
            scale_info::meta_type::<TxExtensions>(),
            scale_info::meta_type::<Inner>()
        );
        assert_eq!(ChargeTx::get().encode(), Inner::from(0).encode());
    }
}

/// `AC-G2.2`, through the real extensions, with `NonFungibleGasTank`, integrated as the crate guide
/// says.
mod non_fungible_gas_tank {
    use super::{storage_root, Encode};
    use crate::mock_tank::{
        new_test_ext, AccountId, Balances, CheckedExtrinsic, GasExtension, Memberships,
        MembershipsGasTank, RuntimeCall, RuntimeOrigin, System, Test, TxExtensions, COLLECTION,
    };
    use frame::deps::{
        frame_support::{
            dispatch::{DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo},
            storage::{storage_prefix, unhashed},
            traits::nonfungibles_v2::{Inspect, InspectEnumerable, Mutate},
            Blake2_128Concat, StorageHasher,
        },
        sp_runtime::{
            generic::ExtrinsicFormat,
            traits::{Applyable, DispatchTransaction, TransactionExtension},
            transaction_validity::{InvalidTransaction, TransactionSource},
        },
    };
    use frame::testing_prelude::Get;
    use frame::testing_prelude::{assert_ok, frame_system, Weight};
    use frame_contrib_traits::gas_tank::{DefaultMaxScan, GasBurner, MakeTank};

    const PERIOD: u64 = 10;

    fn member() -> AccountId {
        AccountId::new([1u8; 32])
    }

    fn tx_ext() -> TxExtensions {
        (
            frame_system::CheckWeight::new(),
            GasExtension::new(pallet_transaction_payment::ChargeTransactionPayment::from(
                0,
            )),
        )
    }

    fn remark() -> RuntimeCall {
        RuntimeCall::System(frame_system::Call::remark {
            remark: b"Hello world".to_vec(),
        })
    }

    fn info(call: &RuntimeCall) -> DispatchInfo {
        DispatchInfo {
            extension_weight: tx_ext().weight(call),
            ..call.get_dispatch_info()
        }
    }

    fn apply(
        call: RuntimeCall,
    ) -> frame::deps::sp_runtime::ApplyExtrinsicResultWithInfo<
        frame::deps::frame_support::dispatch::PostDispatchInfo,
    > {
        let info = info(&call);
        let len = call.encoded_size();
        CheckedExtrinsic {
            format: ExtrinsicFormat::Signed(member(), tx_ext()),
            function: call,
        }
        .apply::<Test>(&info, len)
    }

    // AC-G2.2
    #[test]
    fn admits_the_first_transaction_after_a_window_boundary() {
        new_test_ext().execute_with(|| {
            let call = remark();
            let estimate = GasExtension::estimate(&info(&call), call.encoded_size());

            // A member with no funds, whose tank covers exactly one remark per period.
            assert_ok!(Memberships::mint_into(
                &COLLECTION,
                &1,
                &member(),
                &Default::default(),
                true
            ));
            assert_ok!(MembershipsGasTank::make_tank(
                &(COLLECTION, 1),
                Some(estimate),
                Some(PERIOD)
            ));

            // Window 0: the tank pays once, then is exhausted.
            assert_ok!(apply(remark()));
            System::set_block_number(5);
            assert_eq!(apply(remark()), Err(InvalidTransaction::Payment.into()));

            // The window has just ended (it started at block 1).
            System::set_block_number(1 + PERIOD);
            let before = storage_root();
            let (_, val, _) = tx_ext()
                .validate_only(
                    RuntimeOrigin::signed(member()),
                    &call,
                    &info(&call),
                    call.encoded_size(),
                    TransactionSource::External,
                    0,
                )
                .expect("the new window admits the member; qed");
            assert!(val.1.is_none(), "validation chose the tank");
            assert_eq!(storage_root(), before, "validation wrote nothing");

            // Validated, prepared and dispatched, without a panic and without a fee.
            let result = apply(remark());
            assert!(matches!(result, Ok(Ok(_))), "{result:?}");
            assert_eq!(Balances::free_balance(member()), 0);
            assert_eq!(
                MembershipsGasTank::check_available_gas(&member(), &Weight::zero()),
                Some(Weight::zero()),
                "the new window paid for it"
            );
        });
    }

    /// The key the crate guide documents for `who`'s paying-item note.
    fn paying_item_key(who: &AccountId) -> Vec<u8> {
        let mut key = storage_prefix(b"NonFungibleGasTank", b"PayingItem").to_vec();
        key.extend(Blake2_128Concat::hash(&who.encode()));
        key
    }

    /// Gives the member a membership with an unlimited-period tank of `capacity`.
    fn tank_on(item: u32, capacity: Weight) {
        assert_ok!(Memberships::mint_into(
            &COLLECTION,
            &item,
            &member(),
            &Default::default(),
            true
        ));
        assert_ok!(MembershipsGasTank::make_tank(
            &(COLLECTION, item),
            Some(capacity),
            None
        ));
    }

    /// The membership's tank usage, read straight from its `membership_gas` attribute: (window
    /// start, used, period, capacity).
    fn used(item: u32) -> Weight {
        let (_, used, _, _): (u64, Weight, Option<u64>, Option<Weight>) =
            Memberships::typed_system_attribute(
                &COLLECTION,
                Some(&item),
                &b"membership_gas".as_slice(),
            )
            .expect("the test gave this membership a tank; qed");
        used
    }

    /// `REQ-RT-8` (`DEC-23` fix 3): a call that gives the signer items the scan reaches before its
    /// tank (a purchase, a swap, a new membership) is still charged to that tank, in full.
    // REQ-RT-8
    #[test]
    fn charges_the_noted_tank_when_dispatch_adds_earlier_items() {
        new_test_ext().execute_with(|| {
            let call = remark();
            let info = info(&call);
            let len = call.encoded_size();
            let estimate = GasExtension::estimate(&info, len);
            tank_on(100, estimate);

            let scan_bound = <DefaultMaxScan as Get<u32>>::get() as usize;
            let post_info = tx_ext()
                .1
                .test_run(
                    RuntimeOrigin::signed(member()),
                    &call,
                    &info,
                    len,
                    0,
                    |_| {
                        // Dispatch: the signer gains items until its tank is past the scan bound.
                        let mut item = 101;
                        while Memberships::owned(&member())
                            .take(scan_bound)
                            .any(|owned| owned == (COLLECTION, 100))
                        {
                            Memberships::mint_into(
                                &COLLECTION,
                                &item,
                                &member(),
                                &Default::default(),
                                true,
                            )?;
                            item += 1;
                        }
                        Ok(().into())
                    },
                )
                .expect("the tank pays; qed");
            assert_ok!(post_info);

            assert_eq!(
                used(100),
                estimate,
                "the tank paid the whole metered weight"
            );
            assert_eq!(unhashed::get_raw(&paying_item_key(&member())), None);
        });
    }

    /// The paying-item note never outlives the transaction: neither when it pays (the burn takes
    /// it) nor when it pays no fee, declared or reported (the extension cancels it).
    // REQ-RT-8
    #[test]
    fn paying_item_note_never_outlives_the_transaction() {
        new_test_ext().execute_with(|| {
            let call = remark();
            let len = call.encoded_size();
            tank_on(1, Weight::from_parts(u64::MAX / 2, u64::MAX / 2));
            let note = || unhashed::get_raw(&paying_item_key(&member()));

            // No fee, declared or reported after dispatch: nothing changes at all.
            let before = storage_root();
            for (info, post_info) in [
                (
                    DispatchInfo {
                        pays_fee: Pays::No,
                        ..info(&call)
                    },
                    PostDispatchInfo::from(()),
                ),
                (
                    info(&call),
                    PostDispatchInfo {
                        actual_weight: None,
                        pays_fee: Pays::No,
                    },
                ),
            ] {
                assert_ok!(tx_ext().1.test_run(
                    RuntimeOrigin::signed(member()),
                    &call,
                    &info,
                    len,
                    0,
                    |_| {
                        assert!(note().is_some(), "preparation noted the paying item");
                        Ok(post_info)
                    },
                ));
                assert_eq!(note(), None);
            }
            assert_eq!(storage_root(), before);
            assert_eq!(used(1), Weight::zero());

            // A fee: the tank is charged, and the note is gone.
            assert_ok!(tx_ext().1.test_run(
                RuntimeOrigin::signed(member()),
                &call,
                &info(&call),
                len,
                0,
                |_| Ok(().into()),
            ));
            assert_ne!(used(1), Weight::zero());
            assert_eq!(note(), None);
        });
    }
}
