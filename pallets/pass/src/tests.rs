//! Tests for referenda pass pallet.

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

mod add_device {
    use super::*;
    use sp_core::ConstU32;
    use sp_runtime::BoundedVec;

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            // Mock device info and challenge payload
            let device: Vec<u8> = vec![1, 2, 3, 4];
            let challenge: Vec<u8> = vec![5, 6, 7, 8];
            let challenge_payload: frame_support::BoundedVec<u8, ConstU32<1024>>  = BoundedVec::try_from(challenge).expect("valid vector");
    
            // Call the add_device function
            assert_ok!(Pass::<Test>::add_device(RuntimeOrigin::signed(1), device, challenge_payload));
    
            // Expect an event to be emitted
            let expected_event = Event::<Test>::AddedDevice {
                account: 1,
                device_id: [1;32],
            };
    
            // Check the last event
            frame_system::Pallet::<Test>::assert_has_event(expected_event.into());
        });
    }
}