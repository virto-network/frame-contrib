//! Tests for pass pallet.

use super::{Error, Event, Pallet as Pass};
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};

mod success {
    use super::*;

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::<Test>::success(RuntimeOrigin::signed(1)));
            System::assert_last_event(Event::<Test>::Success.into());
        });
    }
}

mod error {
    use super::*;

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Pass::<Test>::error(RuntimeOrigin::signed(1)),
                Error::<Test>::Error
            );
        });
    }
}
