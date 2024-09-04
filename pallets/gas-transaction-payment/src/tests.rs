use codec::Encode;
use frame_support::{
    assert_noop, assert_ok,
    dispatch::GetDispatchInfo,
    pallet_prelude::{InvalidTransaction, TransactionValidityError},
    parameter_types,
};
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::{Applyable, SignedExtension, Zero};

use crate::mock::{
    fc_pallet_gas_transaction_payment, new_test_ext, AccountId, Balances, CheckedExtrinsic,
    RemarkUnit, RuntimeCall, RuntimeOrigin, SignedExtra, Tank, Test,
};

type Pre = crate::extensions::Pre<ChargeTransactionPayment<Test>>;

const ALICE: AccountId = 1;
const BOB: AccountId = 2;

parameter_types! {
  pub ChargeTx: SignedExtra = fc_pallet_gas_transaction_payment::ChargeTransactionPayment::new(
    pallet_transaction_payment::ChargeTransactionPayment::from(0),
  );
  pub SingleCall: RuntimeCall = RuntimeCall::System(frame_system::Call::remark {
    remark: b"Hello world".to_vec(),
  });
}

fn batch_calls<const S: usize>() -> RuntimeCall {
    RuntimeCall::Utility(pallet_utility::Call::batch {
        calls: vec![SingleCall::get(); S],
    })
}

mod charge_transaction_payment_pre_dispatch {
    use super::*;

    #[test]
    fn fails_if_both_burner_and_inner_transaction_payments_fail() {
        new_test_ext(vec![(ALICE, 0)]).execute_with(|| {
            let call = SingleCall::get();

            assert_eq!(
                ChargeTx::get().pre_dispatch(
                    &ALICE,
                    &call,
                    &call.get_dispatch_info(),
                    call.encoded_size()
                ),
                Err(InvalidTransaction::Payment.into())
            );
        });

        new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
            let call = SingleCall::get();

            assert_eq!(
                ChargeTx::get().pre_dispatch(
                    &BOB,
                    &call,
                    &call.get_dispatch_info(),
                    call.encoded_size()
                ),
                Err(InvalidTransaction::Payment.into())
            );
        });
    }

    #[test]
    fn it_works_if_inner_transaction_payment_works() {
        let call = SingleCall::get();

        new_test_ext(vec![(ALICE, 3)]).execute_with(|| {
            assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, 3));
            assert_ok!(ChargeTx::get().pre_dispatch(
                &BOB,
                &call,
                &call.get_dispatch_info(),
                call.encoded_size()
            ),);
            assert_eq!(Balances::free_balance(BOB), 1);
        });
    }

    #[test]
    fn it_works_if_burner_works() {
        // Works for single remark
        new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
            let call = SingleCall::get();

            assert_eq!(
                ChargeTx::get().pre_dispatch(
                    &ALICE,
                    &call,
                    &call.get_dispatch_info(),
                    call.encoded_size()
                ),
                Ok((ALICE, Pre::Burner(Zero::zero())))
            );
        });

        // Works for remarks batch
        new_test_ext(vec![(ALICE, 4)]).execute_with(|| {
            let call = batch_calls::<1>();
            assert_eq!(
                ChargeTx::get().pre_dispatch(
                    &ALICE,
                    &call,
                    &call.get_dispatch_info(),
                    call.encoded_size()
                ),
                Ok((ALICE, Pre::Burner(RemarkUnit::get() * 2)))
            );

            let call = batch_calls::<3>();
            assert_eq!(
                ChargeTx::get().pre_dispatch(
                    &ALICE,
                    &call,
                    &call.get_dispatch_info(),
                    call.encoded_size()
                ),
                Ok((ALICE, Pre::Burner(Zero::zero())))
            );
        });
    }
}

mod charge_transaction_payment_usage_with_checked_extrinsic {
    use super::*;

    fn assert_applied_extrinsic(xt: CheckedExtrinsic) {
        let info = xt.get_dispatch_info();
        let call_encoded_len = xt.function.encode().len();
        assert_ok!(xt.apply::<Test>(&info, call_encoded_len));
    }
    fn assert_failed_extrinsic(xt: CheckedExtrinsic, error: TransactionValidityError) {
        let info = xt.get_dispatch_info();
        let call_encoded_len = xt.function.encode().len();
        assert_noop!(xt.apply::<Test>(&info, call_encoded_len), error);
    }

    #[test]
    fn validates_single_extrinsic() {
        new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
            assert_applied_extrinsic(CheckedExtrinsic {
                signed: Some((ALICE, ChargeTx::get())),
                function: SingleCall::get(),
            });
            assert_eq!(Tank::get(ALICE), Some(0));
        });

        new_test_ext(vec![(ALICE, 10)]).execute_with(|| {
            assert_applied_extrinsic(CheckedExtrinsic {
                signed: Some((ALICE, ChargeTx::get())),
                function: batch_calls::<9>(),
            });
            assert_eq!(Tank::get(ALICE), Some(0));
        });
    }

    #[test]
    fn validates_multiple_extrinsics() {
        new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
            assert_applied_extrinsic(CheckedExtrinsic {
                signed: Some((ALICE, ChargeTx::get())),
                function: SingleCall::get(),
            });

            assert_failed_extrinsic(
                CheckedExtrinsic {
                    signed: Some((ALICE, ChargeTx::get())),
                    function: SingleCall::get(),
                },
                InvalidTransaction::Payment.into(),
            );
        });
    }
}
