//! Benchmarks for `fc-pallet-spaces`.
//!
//! TODO: they run against the runtime's `Config::Verifier`, through `Config::BenchmarkHelper`.
//! With a real STARK backend, `anchor` must be benchmarked with the worst proof the runtime admits
//! (`MaxProofLen`, the pinned proof policy and log-size cap); the verifier's cost is reported by
//! `ProofVerifier::weight`, not by these benchmarks.

use super::*;
use crate::Pallet;
use alloc::vec;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

fn assert_last_event<T: Config>(event: Event<T>) {
    frame_system::Pallet::<T>::assert_last_event(event.into());
}

fn register_space<T: Config>(owner: &T::AccountId) -> T::SpaceId {
    let space = T::BenchmarkHelper::space(0);
    Spaces::<T>::insert(
        space,
        SpaceInfo {
            owner: owner.clone(),
            program: T::BenchmarkHelper::program(),
            binds: 0,
        },
    );
    Heads::<T>::insert(
        space,
        Head {
            epoch: 0,
            number: 0,
            root: [0u8; 32],
            at: frame_system::Pallet::<T>::block_number(),
        },
    );
    space
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn register() -> Result<(), BenchmarkError> {
        let origin =
            T::RegisterOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let space = T::BenchmarkHelper::space(0);
        let program = T::BenchmarkHelper::program();

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, space, program, [0u8; 32]);

        assert!(Spaces::<T>::contains_key(space));
        Ok(())
    }

    #[benchmark]
    fn anchor(q: Linear<0, { T::MaxPublicLen::get() }>) -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let space = register_space::<T>(&caller);
        let head = Heads::<T>::get(space).expect("registered; qed");
        let root = [1u8; 32];
        let public = vec![0u8; q as usize];
        let statement = Pallet::<T>::anchor_statement(space, &head, root);
        let proof =
            T::BenchmarkHelper::proof(&T::BenchmarkHelper::program(), &public, &statement.encode());
        let proof: BoundedVec<u8, T::MaxProofLen> =
            proof.try_into().map_err(|_| BenchmarkError::Weightless)?;
        let public: BoundedVec<u8, T::MaxPublicLen> =
            public.try_into().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), space, 1, root, proof, public);

        assert_eq!(Heads::<T>::get(space).map(|h| h.number), Some(1));
        Ok(())
    }

    #[benchmark]
    fn anchor_replay() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let space = register_space::<T>(&caller);
        let root = [1u8; 32];
        Anchors::<T>::insert(
            space,
            1,
            AnchorRecord {
                root,
                epoch: 0,
                at: frame_system::Pallet::<T>::block_number(),
            },
        );
        let proof: BoundedVec<u8, T::MaxProofLen> = Default::default();
        let public: BoundedVec<u8, T::MaxPublicLen> = Default::default();

        #[block]
        {
            Pallet::<T>::anchor(
                RawOrigin::Signed(caller).into(),
                space,
                1,
                root,
                proof,
                public,
            )
            .map_err(|e| e.error)?;
        }
        Ok(())
    }

    #[benchmark]
    fn refound() {
        let caller: T::AccountId = whitelisted_caller();
        let space = register_space::<T>(&caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), space, [2u8; 32]);

        assert_eq!(Heads::<T>::get(space).map(|h| h.epoch), Some(1));
    }

    #[benchmark]
    fn set_current_head() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let space = register_space::<T>(&caller);
        let origin =
            T::ResetOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, space, [3u8; 32]);

        assert_last_event::<T>(Event::HeadSet {
            space,
            epoch: 1,
            base: 0,
            previous: [0u8; 32],
            root: [3u8; 32],
            binds: 0,
        });
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
