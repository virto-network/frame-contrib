use super::*;
use crate::pallet::Pallet;
use fc_pallet_content::{
    pallet::{IpfsHash, ItemId},
    Nonce, RETRACTABLE, REVISIONABLE,
};
use frame_benchmarking::v2::*;
use scale_info::prelude::vec;

fn assert_last_event<T: Config>(generic_event: T::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

/// Publish a revisionable + retractable item as `caller` and return its id.
fn setup_item<T: Config>(caller: T::AccountId) -> ItemId
where
    T: fc_pallet_content::pallet::Config,
{
    let item_id =
        fc_pallet_content::pallet::Pallet::<T>::get_item_id(caller.clone(), Nonce::default());
    fc_pallet_content::pallet::Pallet::<T>::publish_item(
        frame_system::RawOrigin::Signed(caller).into(),
        Nonce::default(),
        vec![],
        REVISIONABLE | RETRACTABLE,
        vec![],
        IpfsHash::default(),
    )
    .expect("setup must succeed");
    item_id
}

#[benchmarks(
    where T: fc_pallet_content::pallet::Config
)]
mod benchmarks {
    use frame_system::RawOrigin;

    use super::*;

    #[benchmark]
    pub fn set_profile() {
        let caller: T::AccountId = whitelisted_caller();
        let item_id = setup_item::<T>(caller.clone());

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), item_id.clone());

        assert_last_event::<T>(
            Event::ProfileSet {
                account: caller,
                item_id,
            }
            .into(),
        );
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
