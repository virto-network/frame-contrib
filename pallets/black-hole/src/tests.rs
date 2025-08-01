use crate::mock::*;
use crate::BlackHoleMass;
use frame::prelude::DispatchError;
use frame::testing_prelude::{
    assert_noop, assert_ok, frame_system,
    fungible::{Inspect, Mutate},
};
use frame::token::Preservation::Preserve;

const ALICE: AccountId = AccountId::new([1u8; 32]);

#[test]
fn burning_works() {
    new_test_ext().execute_with(|| {
        // ALICE has funds
        assert_ok!(Balances::mint_into(&ALICE, 10));
        assert_eq!(Balances::total_balance(&ALICE), 10);
        assert_eq!(Balances::total_issuance(), 10);

        run_to_block(10);
        // If the pallet receives some funds to burn…
        assert_ok!(Balances::transfer(
            &ALICE,
            &BlackHole::event_horizon(),
            5,
            Preserve
        ));
        assert_eq!(Balances::total_balance(&ALICE), 5);
        assert_eq!(Balances::total_balance(&BlackHole::event_horizon()), 5);
        assert_eq!(Balances::total_issuance(), 10);

        // Assert that the pallet still has the funds before running `on_idle`.
        run_to_block(11);
        assert_eq!(Balances::total_balance(&BlackHole::event_horizon()), 5);

        run_to_block(12);
        // Poof! Funds are now burned.
        assert_eq!(Balances::total_balance(&ALICE), 5);
        assert_eq!(Balances::total_balance(&BlackHole::event_horizon()), 0);
        assert_eq!(Balances::total_issuance(), 5);

        System::assert_has_event(
            pallet_balances::Event::<Test>::Burned {
                who: BlackHole::event_horizon(),
                amount: 5,
            }
            .into(),
        );
        System::assert_last_event(fc_pallet_black_hole::Event::<Test>::BalanceBurned.into());
    })
}

#[test]
fn counts_the_burned_mass() {
    new_test_ext().execute_with(|| {
        // ALICE has funds
        assert_ok!(Balances::mint_into(&ALICE, 21));

        run_to_block(2);
        assert_ok!(Balances::transfer(
            &ALICE,
            &BlackHole::event_horizon(),
            5,
            Preserve
        ));

        run_to_block(12);
        assert_ok!(Balances::transfer(
            &ALICE,
            &BlackHole::event_horizon(),
            5,
            Preserve
        ));

        run_to_block(22);
        assert_ok!(Balances::transfer(
            &ALICE,
            &BlackHole::event_horizon(),
            5,
            Preserve
        ));

        run_to_block(32);
        assert_ok!(Balances::transfer(
            &ALICE,
            &BlackHole::event_horizon(),
            5,
            Preserve
        ));

        assert_eq!(Balances::total_issuance(), 6);
        assert_eq!(Balances::total_balance(&ALICE), 1);
        assert_eq!(Balances::total_balance(&BlackHole::event_horizon()), 5);

        assert_eq!(BlackHoleMass::<Test>::get(), 15);
    })
}

#[test]
fn dispatch_as_event_horizon_fails_if_bad_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            BlackHole::dispatch_as_event_horizon(
                RuntimeOrigin::signed(ALICE),
                Box::new(frame_system::Call::remark_with_event { remark: vec![] }.into()),
            ),
            DispatchError::BadOrigin
        );
    })
}

#[test]
fn dispatch_as_event_horizon_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Balances::mint_into(&BlackHole::event_horizon(), 100));
        assert_ok!(BlackHole::dispatch_as_event_horizon(
            RuntimeOrigin::root(),
            Box::new(
                pallet_balances::Call::transfer_keep_alive {
                    dest: ALICE,
                    value: 50
                }
                .into()
            ),
        ));

        assert_eq!(Balances::total_balance(&ALICE), 50);
    })
}
