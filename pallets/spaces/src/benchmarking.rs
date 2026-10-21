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

/// `who`'s origin, and the authority it is.
fn authority_of<T: Config>(who: T::AccountId) -> (T::RuntimeOrigin, PalletsOriginOf<T>) {
    let origin: T::RuntimeOrigin = RawOrigin::Signed(who).into();
    let caller = origin.caller().clone();
    (origin, caller)
}

/// A Space governed by `authority`, registered through the call.
fn register_space<T: Config>(authority: PalletsOriginOf<T>) -> Result<T::SpaceId, BenchmarkError> {
    let origin =
        T::CreateOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
    let space = NextSpaceId::<T>::get()
        .or_else(T::SpaceId::initial_value)
        .ok_or(BenchmarkError::Weightless)?;
    Pallet::<T>::register(origin, authority, T::BenchmarkHelper::program(), [0u8; 32])
        .map_err(|_| BenchmarkError::Weightless)?;
    Ok(space)
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn register() -> Result<(), BenchmarkError> {
        let origin =
            T::CreateOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
        let (_, authority) = authority_of::<T>(whitelisted_caller());
        let program = T::BenchmarkHelper::program();

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, authority, program, [0u8; 32]);

        assert!(NextSpaceId::<T>::get().is_some());
        Ok(())
    }

    #[benchmark]
    fn anchor(q: Linear<0, { T::MaxPublicLen::get() }>) -> Result<(), BenchmarkError> {
        let (_, authority) = authority_of::<T>(whitelisted_caller());
        let space = register_space::<T>(authority)?;
        let head = Heads::<T>::get(space).ok_or(BenchmarkError::Weightless)?;
        let root = [1u8; 32];
        let public = vec![0u8; q as usize];
        let statement = Pallet::<T>::anchor_statement(space, &head, root);
        let proof =
            T::BenchmarkHelper::proof(&T::BenchmarkHelper::program(), &public, &statement.encode());
        let proof: BoundedVec<u8, T::MaxProofLen> =
            proof.try_into().map_err(|_| BenchmarkError::Weightless)?;
        let public: BoundedVec<u8, T::MaxPublicLen> =
            public.try_into().map_err(|_| BenchmarkError::Weightless)?;
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), space, 1, root, proof, public);

        assert_eq!(Heads::<T>::get(space).map(|h| h.number), Some(1));
        Ok(())
    }

    #[benchmark]
    fn anchor_replay() -> Result<(), BenchmarkError> {
        let (_, authority) = authority_of::<T>(whitelisted_caller());
        let space = register_space::<T>(authority)?;
        let root = [1u8; 32];
        Anchors::<T>::insert(
            space,
            1,
            AnchorRecord {
                root,
                epoch: 0,
                program_version: 0,
                at: frame_system::Pallet::<T>::block_number(),
            },
        );
        let proof: BoundedVec<u8, T::MaxProofLen> = Default::default();
        let public: BoundedVec<u8, T::MaxPublicLen> = Default::default();
        let caller: T::AccountId = whitelisted_caller();

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
    fn refound() -> Result<(), BenchmarkError> {
        let (origin, authority) = authority_of::<T>(whitelisted_caller());
        let space = register_space::<T>(authority)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, space, [2u8; 32]);

        assert_eq!(Heads::<T>::get(space).map(|h| h.epoch), Some(1));
        Ok(())
    }

    #[benchmark]
    fn set_current_head() -> Result<(), BenchmarkError> {
        let (_, authority) = authority_of::<T>(whitelisted_caller());
        let space = register_space::<T>(authority)?;
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

    #[benchmark]
    fn set_program() -> Result<(), BenchmarkError> {
        let (origin, authority) = authority_of::<T>(whitelisted_caller());
        let space = register_space::<T>(authority)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, space, [4u8; 32]);

        assert!(Programs::<T>::contains_key(space, 1));
        Ok(())
    }

    #[benchmark]
    fn set_authority() -> Result<(), BenchmarkError> {
        let (origin, authority) = authority_of::<T>(whitelisted_caller());
        let space = register_space::<T>(authority)?;
        let (_, next) = authority_of::<T>(account("next", 0, 0));

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, space, next.clone());

        assert_last_event::<T>(Event::AuthoritySet {
            space,
            authority: next,
        });
        Ok(())
    }

    #[benchmark]
    fn dispatch_as_space() -> Result<(), BenchmarkError> {
        let (origin, authority) = authority_of::<T>(whitelisted_caller());
        let space = register_space::<T>(authority)?;
        let call: <T as frame_system::Config>::RuntimeCall =
            frame_system::Call::<T>::remark { remark: vec![] }.into();

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, space, Box::new(call));

        Ok(())
    }

    #[benchmark]
    fn dispatch_as_account() -> Result<(), BenchmarkError> {
        let (origin, authority) = authority_of::<T>(whitelisted_caller());
        let space = register_space::<T>(authority)?;
        let call: <T as frame_system::Config>::RuntimeCall =
            frame_system::Call::<T>::remark { remark: vec![] }.into();

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, space, Box::new(call));

        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
