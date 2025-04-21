use crate::ExtHelper;
use frame_support::DefaultNoBound;
use pallet_balances::{Config, GenesisConfig};
use sp_runtime::BuildStorage;

#[derive(DefaultNoBound)]
pub struct BalancesExtBuilder<T: Config> {
    balances: Vec<(T::AccountId, T::Balance)>,
}

impl<T: Config> BalancesExtBuilder<T> {
    pub fn with_account(mut self, account: T::AccountId, balance: T::Balance) -> Self {
        self.balances.push((account, balance));
        self
    }
}

impl<T: Config> ExtHelper for BalancesExtBuilder<T> {
    fn as_storage(&self) -> impl BuildStorage {
        GenesisConfig::<T> {
            balances: self.balances.clone(),
        }
    }
}
