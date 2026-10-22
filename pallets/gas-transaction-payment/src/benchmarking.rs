// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Benchmarks for Gas Transaction Payment Pallet's transaction extension

extern crate alloc;

use super::*;
use crate::Pallet;
use frame::{
    benchmarking::prelude::*,
    deps::{
        frame_support::dispatch::{DispatchInfo, PostDispatchInfo},
        sp_runtime::traits::{AsTransactionAuthorizedOrigin, DispatchTransaction, Dispatchable},
    },
};
use frame_system::RawOrigin;

fn assert_last_event<T: Config>(generic_event: T::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event);
}

#[benchmarks(where
    T: Config + Send + Sync,
	T::RuntimeOrigin: AsTransactionAuthorizedOrigin,
	T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
)]
mod benchmarks {
    use super::*;

    /// The tank's path end to end: the check in validation, `prepare_gas` in preparation, and
    /// `burn_gas` after dispatch. The fee extension's weight is added to this one in
    /// [`ChargeTransactionPayment::weight`], so the declared weight covers either path.
    ///
    /// The helper decides how hard the tank is to find: a `NonFungibleGasTank` helper should give
    /// the caller as many items as the tank's scan bound, with the tank on the last one scanned.
    /// That tank's burn reads its paying-item note and the noted tank, with no scan, so its cost
    /// does not depend on the setup; a transaction that pays no fee only drops the note, which
    /// costs less than the burn measured here.
    #[benchmark]
    fn charge_transaction_payment() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = account("caller", 0, 0);

        let ext = T::BenchmarkHelper::ext();
        let inner = frame_system::Call::remark {
            remark: alloc::vec![],
        };
        let call = T::RuntimeCall::from(inner);
        let extension_weight = ext.weight(&call);
        let info = DispatchInfo {
            call_weight: Weight::from_parts(100, 0),
            extension_weight,
            class: DispatchClass::Operational,
            pays_fee: Pays::Yes,
        };
        let len = 10;

        // Exactly the metered estimate, so the check passes at its limit.
        let estimate = ChargeTransactionPayment::<
            T,
            <T::BenchmarkHelper as BenchmarkHelper<T>>::Ext,
        >::estimate(&info, len);
        T::BenchmarkHelper::setup_account(&caller, estimate)?;
        if T::GasTank::check_available_gas(&caller, &estimate).is_none() {
            return Err(BenchmarkError::Stop(
                "the helper's tank does not cover the estimate",
            ));
        }

        let post_info = PostDispatchInfo {
            actual_weight: Some(Weight::from_parts(10, 0)),
            pays_fee: Pays::Yes,
        };

        let result;
        #[block]
        {
            result = ext.test_run(
                RawOrigin::Signed(caller.clone()).into(),
                &call,
                &info,
                len,
                0,
                |_| Ok(post_info),
            );
        }

        result
            .map_err(|_| BenchmarkError::Stop("the transaction was invalid"))?
            .map_err(|_| BenchmarkError::Stop("the transaction failed"))?;

        // The tank paid, and reports what it has left.
        let remaining = T::GasTank::check_available_gas(&caller, &Weight::zero())
            .ok_or(BenchmarkError::Stop("the tank is gone"))?;
        assert_last_event::<T>(
            Event::<T>::GasBurned {
                who: caller,
                remaining,
            }
            .into(),
        );

        Ok(())
    }

    // Runs on a real `NonFungibleGasTank`, integrated as the crate guide says.
    impl_benchmark_test_suite!(Pallet, mock_tank::new_test_ext(), mock_tank::Test);
}
