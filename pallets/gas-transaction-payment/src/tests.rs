use codec::Encode;
use frame::deps::frame_support::dispatch::DispatchInfo;
use frame::{
    deps::sp_runtime::{
        generic::ExtrinsicFormat,
        traits::{Applyable, DispatchTransaction},
    },
    testing_prelude::{
        assert_noop, assert_ok, frame_system, parameter_types, InvalidTransaction,
        TransactionValidityError, Weight,
    },
};
use pallet_transaction_payment::ChargeTransactionPayment;

use crate::mock::{
    fc_pallet_gas_transaction_payment, new_test_ext, AccountId, Balances, CheckedExtrinsic,
    RuntimeCall, RuntimeOrigin, Tank, Test, TxExtensions,
};

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
        &call,
        &test_di,
        call.encoded_size(),
        0,
        |_| Ok(().into()),
    )
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
        // // Works for single remark
        new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
            assert_ok!(test_run(ALICE, &Call::get(), Weight::from_parts(1, 0)));
        });

        // Works for remarks batch
        new_test_ext(vec![(ALICE, 4)]).execute_with(|| {
            assert_ok!(test_run(ALICE, &Call::get(), Weight::from_parts(1, 0)));
            assert_ok!(test_run(ALICE, &Call::get(), Weight::from_parts(3, 0)));
        });
    }
}

mod charge_transaction_payment_usage_with_checked_extrinsic {
    use super::*;

    fn assert_applied_extrinsic<const S: usize>(xt: CheckedExtrinsic) {
        let info = DispatchInfo {
            call_weight: Weight::from_parts(S as u64, 0),
            ..Default::default()
        };
        let call_encoded_len = xt.function.encode().len();
        assert_ok!(xt.apply::<Test>(&info, call_encoded_len));
    }
    fn assert_failed_extrinsic<const S: usize>(
        xt: CheckedExtrinsic,
        error: TransactionValidityError,
    ) {
        let info = DispatchInfo {
            call_weight: Weight::from_parts(S as u64, 0),
            ..Default::default()
        };
        let call_encoded_len = xt.function.encode().len();
        assert_noop!(xt.apply::<Test>(&info, call_encoded_len), error);
    }

    #[test]
    fn validates_single_extrinsic() {
        new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
            assert_applied_extrinsic::<1>(CheckedExtrinsic {
                format: ExtrinsicFormat::Signed(ALICE, ChargeTx::get()),
                function: Call::get(),
            });
            assert_eq!(Tank::get(ALICE), Some(Weight::zero()));
        });
    }

    #[test]
    fn validates_multiple_extrinsics() {
        new_test_ext(vec![(ALICE, 3)]).execute_with(|| {
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
