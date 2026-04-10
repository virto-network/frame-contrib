use frame::deps::frame_support::{assert_noop, assert_ok, traits::fungibles::Mutate};
use frame::deps::frame_support::{dispatch::DispatchInfo, traits::tokens::Preservation};
use sp_runtime::{
    traits::DispatchTransaction, transaction_validity::InvalidTransaction, BoundedVec, Permill,
};

use crate::{
    mock::*, types::FeeConfig, ChargeFees, CommunityFees as CommunityFeesStorage, Error, Event,
    ProtocolFees, WithFees,
};

fn fee_name(s: &[u8]) -> BoundedVec<u8, MaxFeeNameLen> {
    BoundedVec::try_from(s.to_vec()).unwrap()
}

fn balance_of(asset: AssetId, who: AccountId) -> Balance {
    use frame::deps::frame_support::traits::fungibles::Inspect;
    <Assets as Inspect<_>>::balance(asset, &who)
}

/// Helper to run the ChargeFees extension lifecycle on a call.
fn run_extension(
    who: AccountId,
    call: &RuntimeCall,
) -> <ChargeFees<Test> as DispatchTransaction<RuntimeCall>>::Result {
    let ext = ChargeFees::<Test>::default();
    let info = DispatchInfo::default();
    ext.test_run(RuntimeOrigin::signed(who), call, &info, 0, 0, |_| {
        Ok(().into())
    })
}

// ============================================================================
// Extrinsic tests
// ============================================================================

mod protocol_fees {
    use super::*;

    #[test]
    fn set_protocol_fee_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"protocol_cut"),
                FeeConfig::Percentage(Permill::from_percent(5)),
                FEE_RECEIVER_PROTOCOL,
            ));

            let fees = ProtocolFees::<Test>::get();
            assert_eq!(fees.len(), 1);
            assert_eq!(fees[0].name, fee_name(b"protocol_cut"));
            assert_eq!(
                fees[0].config,
                FeeConfig::Percentage(Permill::from_percent(5))
            );
            assert_eq!(fees[0].beneficiary, FEE_RECEIVER_PROTOCOL);

            System::assert_last_event(
                Event::ProtocolFeeSet {
                    name: fee_name(b"protocol_cut"),
                }
                .into(),
            );
        });
    }

    #[test]
    fn set_protocol_fee_upserts() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"fee"),
                FeeConfig::Fixed(10),
                FEE_RECEIVER_PROTOCOL,
            ));
            // Update the same fee
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"fee"),
                FeeConfig::Fixed(20),
                FEE_RECEIVER_COMMUNITY,
            ));

            let fees = ProtocolFees::<Test>::get();
            assert_eq!(fees.len(), 1);
            assert_eq!(fees[0].config, FeeConfig::Fixed(20));
            assert_eq!(fees[0].beneficiary, FEE_RECEIVER_COMMUNITY);
        });
    }

    #[test]
    fn set_protocol_fee_requires_admin() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Fees::set_protocol_fee(
                    RuntimeOrigin::signed(MEMBER_1A),
                    fee_name(b"fee"),
                    FeeConfig::Fixed(10),
                    FEE_RECEIVER_PROTOCOL,
                ),
                sp_runtime::DispatchError::BadOrigin,
            );
        });
    }

    #[test]
    fn remove_protocol_fee_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"fee"),
                FeeConfig::Fixed(10),
                FEE_RECEIVER_PROTOCOL,
            ));
            assert_ok!(Fees::remove_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"fee"),
            ));
            assert!(ProtocolFees::<Test>::get().is_empty());
        });
    }

    #[test]
    fn remove_nonexistent_protocol_fee_fails() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Fees::remove_protocol_fee(RuntimeOrigin::root(), fee_name(b"nope")),
                Error::<Test>::FeeNotFound,
            );
        });
    }

    #[test]
    fn rejects_invalid_fee_config() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Fees::set_protocol_fee(
                    RuntimeOrigin::root(),
                    fee_name(b"bad"),
                    FeeConfig::PercentageClamped {
                        rate: Permill::from_percent(10),
                        min: 500,
                        max: 100, // min > max
                    },
                    FEE_RECEIVER_PROTOCOL,
                ),
                Error::<Test>::InvalidFeeConfig,
            );
        });
    }
}

mod community_fees {
    use super::*;

    #[test]
    fn set_community_fee_works() {
        new_test_ext().execute_with(|| {
            // Signed by community admin (account 10 => community id 10)
            assert_ok!(Fees::set_community_fee(
                RuntimeOrigin::signed(COMMUNITY_1_ADMIN),
                fee_name(b"community_cut"),
                FeeConfig::Percentage(Permill::from_percent(3)),
                FEE_RECEIVER_COMMUNITY,
            ));

            let fees = CommunityFeesStorage::<Test>::get(COMMUNITY_1_ADMIN as CommunityId);
            assert_eq!(fees.len(), 1);
            assert_eq!(fees[0].name, fee_name(b"community_cut"));
        });
    }

    #[test]
    fn remove_community_fee_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_community_fee(
                RuntimeOrigin::signed(COMMUNITY_1_ADMIN),
                fee_name(b"fee"),
                FeeConfig::Fixed(5),
                FEE_RECEIVER_COMMUNITY,
            ));
            assert_ok!(Fees::remove_community_fee(
                RuntimeOrigin::signed(COMMUNITY_1_ADMIN),
                fee_name(b"fee"),
            ));
            assert!(CommunityFeesStorage::<Test>::get(COMMUNITY_1_ADMIN as CommunityId).is_empty());
        });
    }
}

// ============================================================================
// Fee calculation tests
// ============================================================================

mod fee_config {
    use super::*;

    #[test]
    fn fixed_fee() {
        assert_eq!(FeeConfig::Fixed(42u64).calculate(1000, None), 42);
    }

    #[test]
    fn percentage_fee() {
        let config = FeeConfig::Percentage(Permill::from_percent(5));
        assert_eq!(config.calculate(1000u64, None), 50);
        assert_eq!(config.calculate(0u64, None), 0);
    }

    #[test]
    fn percentage_rounds_up() {
        // 1% of 99 = 0.99, mul_ceil rounds up to 1
        let config = FeeConfig::Percentage(Permill::from_percent(1));
        assert_eq!(config.calculate(99u64, None), 1);
    }

    #[test]
    fn percentage_clamped_min() {
        let config = FeeConfig::<u64>::PercentageClamped {
            rate: Permill::from_percent(1),
            min: 50,
            max: 500,
        };
        // 1% of 100 = 1, but min is 50
        assert_eq!(config.calculate(100, None), 50);
    }

    #[test]
    fn percentage_clamped_max() {
        let config = FeeConfig::<u64>::PercentageClamped {
            rate: Permill::from_percent(50),
            min: 10,
            max: 200,
        };
        // 50% of 1000 = 500, but max is 200
        assert_eq!(config.calculate(1000, None), 200);
    }

    #[test]
    fn percentage_clamped_within_range() {
        let config = FeeConfig::<u64>::PercentageClamped {
            rate: Permill::from_percent(10),
            min: 5,
            max: 500,
        };
        // 10% of 1000 = 100, within [5, 500]
        assert_eq!(config.calculate(1000, None), 100);
    }

    #[test]
    fn rounds_up_to_min_balance() {
        // 1% of 50 = 1 (after ceil), but min_balance is 10 → round up to 10
        let config = FeeConfig::Percentage(Permill::from_percent(1));
        assert_eq!(config.calculate(50u64, Some(10)), 10);
    }

    #[test]
    fn no_rounding_when_fee_above_min_balance() {
        // 5% of 1000 = 50, min_balance is 10 → no rounding needed
        let config = FeeConfig::Percentage(Permill::from_percent(5));
        assert_eq!(config.calculate(1000u64, Some(10)), 50);
    }

    #[test]
    fn zero_fee_stays_zero_despite_min_balance() {
        // 5% of 0 = 0, should stay 0 even with min_balance
        let config = FeeConfig::Percentage(Permill::from_percent(5));
        assert_eq!(config.calculate(0u64, Some(10)), 0);
    }

    #[test]
    fn fixed_fee_rounds_up_to_min_balance() {
        // Fixed(3) with min_balance 5 → rounds up to 5
        assert_eq!(FeeConfig::Fixed(3u64).calculate(1000, Some(5)), 5);
    }

    #[test]
    fn validate_rejects_min_greater_than_max() {
        let config = FeeConfig::<u64>::PercentageClamped {
            rate: Permill::from_percent(10),
            min: 500,
            max: 100, // min > max
        };
        assert!(!config.is_valid());
    }

    #[test]
    fn validate_accepts_valid_config() {
        assert!(FeeConfig::Fixed(42u64).is_valid());
        assert!(FeeConfig::<u64>::Percentage(Permill::from_percent(5)).is_valid());
        assert!(FeeConfig::<u64>::PercentageClamped {
            rate: Permill::from_percent(10),
            min: 10,
            max: 500,
        }
        .is_valid());
    }
}

// ============================================================================
// Adapter (WithFees) tests
// ============================================================================

mod adapter {
    use super::*;

    #[test]
    fn transfer_with_protocol_fee_charges_on_top() {
        new_test_ext().execute_with(|| {
            // Set a 5% protocol fee
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"protocol"),
                FeeConfig::Percentage(Permill::from_percent(5)),
                FEE_RECEIVER_PROTOCOL,
            ));

            // Transfer 1000 from NO_COMMUNITY (no community fees, only protocol)
            assert_ok!(<WithFees<Test> as Mutate<AccountId>>::transfer(
                ASSET_ID,
                &NO_COMMUNITY,
                &MEMBER_1A,
                1000,
                Preservation::Preserve,
            ));

            // Sender pays 1000 (transfer) + 50 (5% fee) = 1050 total deducted
            assert_eq!(balance_of(ASSET_ID, NO_COMMUNITY), INITIAL_BALANCE - 1050);
            // Receiver gets the full 1000
            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE + 1000);
            // Fee receiver gets 50
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 50);
        });
    }

    #[test]
    fn transfer_with_community_and_protocol_fees() {
        new_test_ext().execute_with(|| {
            // Set protocol fee: 5% -> account 50
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"protocol"),
                FeeConfig::Percentage(Permill::from_percent(5)),
                FEE_RECEIVER_PROTOCOL,
            ));

            // Set community 1 fee: 3% -> account 51
            // CommunityOrigin = EnsureSigned, so signed(1) => community_id = 1
            // DummyAccountCommunity maps accounts 100..=199 to community 1
            assert_ok!(Fees::set_community_fee(
                RuntimeOrigin::signed(1), // community id 1
                fee_name(b"community"),
                FeeConfig::Percentage(Permill::from_percent(3)),
                FEE_RECEIVER_COMMUNITY,
            ));

            // Transfer 1000 from MEMBER_1A (community 1)
            assert_ok!(<WithFees<Test> as Mutate<AccountId>>::transfer(
                ASSET_ID,
                &MEMBER_1A,
                &NO_COMMUNITY,
                1000,
                Preservation::Preserve,
            ));

            // Protocol fee: 5% of 1000 = 50
            // Community fee: 3% of 1000 = 30
            // Total deducted: 1000 + 50 + 30 = 1080
            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE - 1080);
            assert_eq!(balance_of(ASSET_ID, NO_COMMUNITY), INITIAL_BALANCE + 1000);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 50);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_COMMUNITY), 30);
        });
    }

    #[test]
    fn transfer_without_fees_works_normally() {
        new_test_ext().execute_with(|| {
            // No fees configured
            assert_ok!(<WithFees<Test> as Mutate<AccountId>>::transfer(
                ASSET_ID,
                &MEMBER_1A,
                &NO_COMMUNITY,
                500,
                Preservation::Preserve,
            ));

            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE - 500);
            assert_eq!(balance_of(ASSET_ID, NO_COMMUNITY), INITIAL_BALANCE + 500);
        });
    }

    #[test]
    fn transfer_fails_if_insufficient_balance() {
        new_test_ext().execute_with(|| {
            // Set a 50% fee — even after capping, transfer + fee exceeds balance
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"big_fee"),
                FeeConfig::Percentage(Permill::from_percent(50)),
                FEE_RECEIVER_PROTOCOL,
            ));

            // Try to transfer almost entire balance — 50% fee on top makes it unaffordable
            assert_noop!(
                <WithFees<Test> as Mutate<AccountId>>::transfer(
                    ASSET_ID,
                    &NO_COMMUNITY,
                    &MEMBER_1A,
                    INITIAL_BALANCE, // can't afford amount + 50% fee
                    Preservation::Preserve,
                ),
                sp_runtime::TokenError::FundsUnavailable,
            );

            // Balances unchanged (atomic rollback)
            assert_eq!(balance_of(ASSET_ID, NO_COMMUNITY), INITIAL_BALANCE);
            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE);
        });
    }

    #[test]
    fn oversized_fee_is_capped_at_transfer_amount() {
        new_test_ext().execute_with(|| {
            // Fee that would be larger than transfer amount
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"huge"),
                FeeConfig::Fixed(5000),
                FEE_RECEIVER_PROTOCOL,
            ));

            // Transfer 100 — fee gets capped from 5000 to 100
            assert_ok!(<WithFees<Test> as Mutate<AccountId>>::transfer(
                ASSET_ID,
                &NO_COMMUNITY,
                &MEMBER_1A,
                100,
                Preservation::Preserve,
            ));

            // Fee capped to transfer amount: sender pays 100 (fee) + 100 (transfer) = 200
            assert_eq!(balance_of(ASSET_ID, NO_COMMUNITY), INITIAL_BALANCE - 200);
            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE + 100);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 100);
        });
    }

    #[test]
    fn atomic_rollback_on_partial_fee_failure() {
        new_test_ext().execute_with(|| {
            // Set two fees that together with the transfer exceed the balance
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"fee1"),
                FeeConfig::Percentage(Permill::from_percent(40)),
                FEE_RECEIVER_PROTOCOL,
            ));
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"fee2"),
                FeeConfig::Percentage(Permill::from_percent(40)),
                FEE_RECEIVER_COMMUNITY,
            ));

            // Transfer amount where fees (80%) + transfer exceed balance
            // Balance = 10000, transfer = 8000, fees = 80% of 8000 = 6400 total
            // 8000 + 6400 = 14400 > 10000
            assert_noop!(
                <WithFees<Test> as Mutate<AccountId>>::transfer(
                    ASSET_ID,
                    &NO_COMMUNITY,
                    &MEMBER_1A,
                    8000,
                    Preservation::Preserve,
                ),
                sp_runtime::TokenError::FundsUnavailable,
            );

            // All balances unchanged — atomic rollback
            assert_eq!(balance_of(ASSET_ID, NO_COMMUNITY), INITIAL_BALANCE);
            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 0);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_COMMUNITY), 0);
        });
    }

    #[test]
    fn no_community_fees_for_non_member() {
        new_test_ext().execute_with(|| {
            // Set both protocol and community fees
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"protocol"),
                FeeConfig::Fixed(10),
                FEE_RECEIVER_PROTOCOL,
            ));
            assert_ok!(Fees::set_community_fee(
                RuntimeOrigin::signed(1),
                fee_name(b"community"),
                FeeConfig::Fixed(20),
                FEE_RECEIVER_COMMUNITY,
            ));

            // Transfer from NO_COMMUNITY — only protocol fee applies
            assert_ok!(<WithFees<Test> as Mutate<AccountId>>::transfer(
                ASSET_ID,
                &NO_COMMUNITY,
                &MEMBER_1A,
                500,
                Preservation::Preserve,
            ));

            assert_eq!(balance_of(ASSET_ID, NO_COMMUNITY), INITIAL_BALANCE - 510);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 10);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_COMMUNITY), 0); // no community fee
        });
    }

    #[test]
    fn fixed_fee_on_transfer() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"flat"),
                FeeConfig::Fixed(42),
                FEE_RECEIVER_PROTOCOL,
            ));

            assert_ok!(<WithFees<Test> as Mutate<AccountId>>::transfer(
                ASSET_ID,
                &MEMBER_1A,
                &MEMBER_1B,
                100,
                Preservation::Preserve,
            ));

            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE - 142);
            assert_eq!(balance_of(ASSET_ID, MEMBER_1B), INITIAL_BALANCE + 100);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 42);
        });
    }
}

// ============================================================================
// Transaction extension tests
// ============================================================================

mod extension {
    use super::*;

    fn transfer_call(dest: AccountId, amount: Balance) -> RuntimeCall {
        RuntimeCall::Assets(pallet_assets::Call::transfer {
            id: ASSET_ID,
            target: dest,
            amount,
        })
    }

    #[test]
    fn charges_protocol_fee_on_assets_transfer() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"protocol"),
                FeeConfig::Percentage(Permill::from_percent(5)),
                FEE_RECEIVER_PROTOCOL,
            ));

            let call = transfer_call(MEMBER_1B, 1000);
            assert_ok!(run_extension(NO_COMMUNITY, &call));

            // Fee was charged in prepare (before the actual call ran)
            // 5% of 1000 = 50
            assert_eq!(
                balance_of(ASSET_ID, NO_COMMUNITY),
                INITIAL_BALANCE - 50 // only fees deducted (test_run uses a noop call body)
            );
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 50);
        });
    }

    #[test]
    fn charges_community_and_protocol_fees() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"protocol"),
                FeeConfig::Fixed(10),
                FEE_RECEIVER_PROTOCOL,
            ));
            assert_ok!(Fees::set_community_fee(
                RuntimeOrigin::signed(1), // community 1
                fee_name(b"community"),
                FeeConfig::Fixed(20),
                FEE_RECEIVER_COMMUNITY,
            ));

            let call = transfer_call(MEMBER_1B, 500);
            assert_ok!(run_extension(MEMBER_1A, &call));

            // Protocol + community fees charged
            assert_eq!(
                balance_of(ASSET_ID, MEMBER_1A),
                INITIAL_BALANCE - 30 // 10 + 20
            );
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 10);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_COMMUNITY), 20);
        });
    }

    #[test]
    fn no_fees_on_non_asset_calls() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"protocol"),
                FeeConfig::Fixed(100),
                FEE_RECEIVER_PROTOCOL,
            ));

            // A non-asset call (system remark)
            let call = RuntimeCall::System(frame::deps::frame_system::Call::remark {
                remark: b"hello".to_vec(),
            });
            assert_ok!(run_extension(MEMBER_1A, &call));

            // No fees charged
            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 0);
        });
    }

    #[test]
    fn rejects_if_insufficient_balance_for_fees() {
        new_test_ext().execute_with(|| {
            // 50% fee + transfer of almost full balance = unaffordable
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"big_fee"),
                FeeConfig::Percentage(Permill::from_percent(50)),
                FEE_RECEIVER_PROTOCOL,
            ));

            // Transfer that leaves no room for the 50% fee
            let call = transfer_call(MEMBER_1B, INITIAL_BALANCE);
            assert_noop!(run_extension(MEMBER_1A, &call), InvalidTransaction::Payment,);

            // Nothing changed
            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE);
        });
    }

    #[test]
    fn refunds_fees_on_dispatch_failure() {
        new_test_ext().execute_with(|| {
            assert_ok!(Fees::set_protocol_fee(
                RuntimeOrigin::root(),
                fee_name(b"protocol"),
                FeeConfig::Fixed(50),
                FEE_RECEIVER_PROTOCOL,
            ));

            let ext = ChargeFees::<Test>::default();
            let info = DispatchInfo::default();
            let call = transfer_call(MEMBER_1B, 1000);

            // Simulate a failing dispatch
            let result = ext.test_run(
                RuntimeOrigin::signed(MEMBER_1A),
                &call,
                &info,
                0,
                0,
                |_| Err(sp_runtime::DispatchError::Other("simulated failure").into()),
            );

            // The dispatch result is the inner error
            assert!(result.unwrap().is_err());

            // Fees should have been refunded
            assert_eq!(balance_of(ASSET_ID, MEMBER_1A), INITIAL_BALANCE);
            assert_eq!(balance_of(ASSET_ID, FEE_RECEIVER_PROTOCOL), 0);
        });
    }
}
