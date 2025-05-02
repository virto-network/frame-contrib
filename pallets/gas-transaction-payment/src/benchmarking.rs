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
use frame_benchmarking::v2::*;
use frame_support::dispatch::{DispatchInfo, PostDispatchInfo};
use frame_system::RawOrigin;
use sp_runtime::traits::{AsTransactionAuthorizedOrigin, DispatchTransaction, Dispatchable};

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

#[benchmarks(where
    T: Config + Send + Sync,
	T::RuntimeOrigin: AsTransactionAuthorizedOrigin,
	T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    <T::GasTank as GasFueler>::AccountId: From<T::AccountId>,
)]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn charge_transaction_payment() {
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

        T::GasTank::refuel_gas_to_account(&caller.clone().into(), &info.call_weight);
        let remaining = T::GasTank::check_available_gas(&caller, &info.call_weight)
            .expect("at least some remaining gas should be available after refueling; qed");

        let post_info = PostDispatchInfo {
            actual_weight: Some(Weight::from_parts(10, 0)),
            pays_fee: Pays::Yes,
        };

        #[block]
        {
            assert!(ext
                .test_run(
                    RawOrigin::Signed(caller.clone()).into(),
                    &call,
                    &info,
                    10,
                    0,
                    |_| Ok(post_info)
                )
                .unwrap()
                .is_ok());
        }

        assert_last_event::<T>(
            Event::<T>::GasBurned {
                who: caller,
                remaining,
            }
            .into(),
        );
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), mock::Test);
}
