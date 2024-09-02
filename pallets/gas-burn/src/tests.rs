use codec::Encode;
use frame_support::{
    dispatch::GetDispatchInfo, pallet_prelude::InvalidTransaction, parameter_types,
};
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::{SignedExtension as _, Zero};

use crate::{
    mock::{new_test_ext, AccountId, RuntimeCall, SignedExtension, Test},
    ChargeTxBurningGas,
};

type Pre = crate::extensions::Pre<ChargeTransactionPayment<Test>>;

const ALICE: AccountId = 1;
const BOB: AccountId = 2;

parameter_types! {
  pub ChargeTx: SignedExtension = ChargeTxBurningGas::new(
    pallet_transaction_payment::ChargeTransactionPayment::from(0),
  );
}

#[test]
fn fails_if_account_does_not_have_tank() {
    new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
        let call = RuntimeCall::System(frame_system::Call::remark {
            remark: b"Hello world".to_vec(),
        });

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
fn it_works_for_single_remark() {
    new_test_ext(vec![(ALICE, 1)]).execute_with(|| {
        let call = RuntimeCall::System(frame_system::Call::remark {
            remark: b"Hello world".to_vec(),
        });

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
