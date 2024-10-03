use core::marker::PhantomData;

use frame_support::{
    pallet_prelude::{Decode, Encode},
    traits::nonfungibles_v2,
    weights::Weight,
};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{Bounded, CheckedAdd, CheckedSub};

use crate::*;

pub const ATTR_MEMBERSHIP_GAS: &[u8] = b"membership_gas";
pub const ATTR_GAS_TX_PAY_WITH_MEMBERSHIP: &[u8] = b"mbmshp_pays_gas";

#[derive(Encode, Decode, Debug)]
pub struct MembershipWeightTank<T: frame_system::Config> {
    pub since: BlockNumberFor<T>,
    pub used: Weight,
    pub period: Option<BlockNumberFor<T>>,
    pub max_per_period: Option<Weight>,
}

impl<T> Default for MembershipWeightTank<T>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Default,
{
    fn default() -> Self {
        Self {
            since: Default::default(),
            used: Default::default(),
            period: Default::default(),
            max_per_period: Default::default(),
        }
    }
}

pub struct NonFungibleGasBurner<T, F, I>(PhantomData<(T, F, I)>);

impl<T, F, ItemConfig> GasBurner for NonFungibleGasBurner<T, F, ItemConfig>
where
    T: frame_system::Config,
    BlockNumberFor<T>: Bounded,
    F: nonfungibles_v2::Inspect<T::AccountId>
        + nonfungibles_v2::InspectEnumerable<T::AccountId>
        + nonfungibles_v2::Mutate<T::AccountId, ItemConfig>,
    ItemConfig: Default,
{
    type AccountId = T::AccountId;
    type Gas = Weight;

    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        F::owned(who).find_map(|(collection, item)| {
            let mut gas_tank: MembershipWeightTank<T> =
                F::typed_system_attribute(&collection, Some(&item), &ATTR_MEMBERSHIP_GAS)?;

            let block_number = frame_system::Pallet::<T>::block_number();
            let period = gas_tank.period.unwrap_or(BlockNumberFor::<T>::max_value());

            let Some(max_weight) = gas_tank.max_per_period else {
                return Some(Weight::MAX);
            };

            if block_number.checked_sub(&gas_tank.since)? > period {
                gas_tank.since = block_number.checked_add(&period)?;
                gas_tank.used = Weight::zero();

                F::set_typed_attribute(&collection, &item, &ATTR_MEMBERSHIP_GAS, &gas_tank).ok()?;
            };

            let remaining = max_weight.checked_sub(&gas_tank.used.checked_add(estimated)?)?;
            F::set_typed_attribute(
                &collection,
                &item,
                &ATTR_GAS_TX_PAY_WITH_MEMBERSHIP,
                &remaining,
            )
            .ok()?;

            Some(remaining)
        })
    }

    fn burn_gas(who: &Self::AccountId, expected: &Self::Gas, used: &Self::Gas) -> Self::Gas {
        let Some(mut gas_tank): Option<MembershipWeightTank<T>> =
            F::owned(who).find_map(|(collection, item)| {
                let expected_remaining: Weight = F::typed_system_attribute(
                    &collection,
                    Some(&item),
                    &ATTR_GAS_TX_PAY_WITH_MEMBERSHIP,
                )?;

                if expected.eq(&expected_remaining) {
                    Some(F::typed_system_attribute(
                        &collection,
                        Some(&item),
                        &ATTR_MEMBERSHIP_GAS,
                    )?)
                } else {
                    None
                }
            })
        else {
            return Weight::zero();
        };
        let Some(max_weight) = gas_tank.max_per_period else {
            return Weight::MAX;
        };

        gas_tank.used = gas_tank.used.add_proof_size(used.proof_size());
        gas_tank.used = gas_tank.used.add_ref_time(used.ref_time());

        max_weight.saturating_sub(gas_tank.used)
    }
}
