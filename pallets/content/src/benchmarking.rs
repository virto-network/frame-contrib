use super::*;
use crate::Pallet;
use frame_benchmarking::v2::*;
use scale_info::prelude::vec;

fn assert_last_event<T: Config>(generic_event: T::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

#[benchmarks]
mod benchmarks {
    use frame_system::RawOrigin;

    use super::*;

    #[benchmark]
    pub fn publish_item() {
        let caller: T::AccountId = whitelisted_caller();
        let item_id = Pallet::<T>::get_item_id(caller.clone(), Nonce::default());

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller.clone()),
            Nonce::default(),
            vec![],
            0,
            vec![],
            IpfsHash::default(),
        );

        // Verification code
        assert_last_event::<T>(
            Event::PublishRevision {
                item_id,
                owner: caller,
                revision_id: 0,
                links: vec![],
                ipfs_hash: IpfsHash::default(),
            }
            .into(),
        );
    }

    #[benchmark]
    pub fn publish_revision() {
        // Setup code
        let caller: T::AccountId = whitelisted_caller();
        let item_id = Pallet::<T>::get_item_id(caller.clone(), Nonce::default());

        Pallet::<T>::publish_item(
            RawOrigin::Signed(caller.clone()).into(),
            Nonce::default(),
            vec![],
            REVISIONABLE | RETRACTABLE,
            vec![],
            IpfsHash::default(),
        )
        .expect("setup must succeed");

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller.clone()),
            item_id.clone(),
            vec![],
            IpfsHash::default(),
        );

        // Verification code
        assert_last_event::<T>(
            Event::PublishRevision {
                item_id,
                owner: caller,
                revision_id: 1,
                links: vec![],
                ipfs_hash: IpfsHash::default(),
            }
            .into(),
        );
    }

    #[benchmark]
    pub fn retract_item() {
        // Setup code
        let caller: T::AccountId = whitelisted_caller();
        let item_id = Pallet::<T>::get_item_id(caller.clone(), Nonce::default());

        Pallet::<T>::publish_item(
            RawOrigin::Signed(caller.clone()).into(),
            Nonce::default(),
            vec![],
            REVISIONABLE | RETRACTABLE,
            vec![],
            IpfsHash::default(),
        )
        .expect("setup must succeed");

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), item_id.clone());

        // Verification code
        assert_last_event::<T>(
            Event::RetractItem {
                item_id,
                owner: caller,
            }
            .into(),
        );
    }

    #[benchmark]
    pub fn set_not_revisionable() {
        // Setup code
        let caller: T::AccountId = whitelisted_caller();
        let item_id = Pallet::<T>::get_item_id(caller.clone(), Nonce::default());

        Pallet::<T>::publish_item(
            RawOrigin::Signed(caller.clone()).into(),
            Nonce::default(),
            vec![],
            REVISIONABLE | RETRACTABLE,
            vec![],
            IpfsHash::default(),
        )
        .expect("setup must succeed");

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), item_id.clone());

        // Verification code
        assert_last_event::<T>(
            Event::SetNotRevsionable {
                item_id,
                owner: caller,
            }
            .into(),
        );
    }

    #[benchmark]
    pub fn set_not_retractable() {
        // Setup code
        let caller: T::AccountId = whitelisted_caller();
        let item_id = Pallet::<T>::get_item_id(caller.clone(), Nonce::default());

        Pallet::<T>::publish_item(
            RawOrigin::Signed(caller.clone()).into(),
            Nonce::default(),
            vec![],
            REVISIONABLE | RETRACTABLE,
            vec![],
            IpfsHash::default(),
        )
        .expect("setup must succeed");

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), item_id.clone());

        // Verification code
        assert_last_event::<T>(
            Event::SetNotRetractable {
                item_id,
                owner: caller,
            }
            .into(),
        );
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
