use crate::ExtHelper;
use alloc::vec::Vec;
use frame_support::DefaultNoBound;
use pallet_assets::{Config, GenesisConfig};

type AssetOf<T, I = ()> = Asset<
    <T as frame_system::Config>::AccountId,
    <T as Config<I>>::AssetId,
    <T as Config<I>>::Balance,
>;

pub struct Asset<AccountId, AssetId, Balance> {
    id: AssetId,
    owner: AccountId,
    minimum_balance: Balance,
    is_sufficient: bool,
    metadata: Option<(Vec<u8>, Vec<u8>, u8)>,
    accounts: Vec<(AccountId, Balance)>,
}

impl<AccountId, AssetId, Balance> Asset<AccountId, AssetId, Balance> {
    pub fn new(
        id: AssetId,
        owner: AccountId,
        minimum_balance: Balance,
        is_sufficient: bool,
    ) -> Self {
        Self {
            id,
            owner,
            minimum_balance,
            is_sufficient,
            metadata: None,
            accounts: Vec::new(),
        }
    }

    pub fn with_metadata(mut self, name: Vec<u8>, symbol: Vec<u8>, decimals: u8) -> Self {
        self.metadata = Some((name, symbol, decimals));
        self
    }

    pub fn add_account(mut self, id: AccountId, balance: Balance) -> Self {
        self.accounts.push((id, balance));
        self
    }
}

#[derive(DefaultNoBound)]
pub struct AssetsExtBuilder<T: Config<I>, I: 'static = ()> {
    assets: Vec<AssetOf<T, I>>,
}

impl<T: Config<I>, I: 'static> AssetsExtBuilder<T, I> {
    pub fn with_asset(mut self, asset: AssetOf<T, I>) -> Self {
        self.assets.push(asset);
        self
    }
}

impl<T: Config<I>, I: 'static> ExtHelper for AssetsExtBuilder<T, I> {
    fn as_storage(&self) -> impl sp_runtime::BuildStorage {
        GenesisConfig::<T, I> {
            assets: self
                .assets
                .iter()
                .map(|a| {
                    (
                        a.id.clone(),
                        a.owner.clone(),
                        a.is_sufficient,
                        a.minimum_balance,
                    )
                })
                .collect(),
            metadata: self
                .assets
                .iter()
                .filter(|a| a.metadata.is_some())
                .map(|Asset { id, metadata, .. }| {
                    let (name, symbol, decimals) = metadata
                        .clone()
                        .expect("filtered by assets with some metadata; qed");
                    (id.clone(), name, symbol, decimals)
                })
                .collect(),
            accounts: self
                .assets
                .iter()
                .flat_map(|Asset { id, accounts, .. }| {
                    accounts
                        .clone()
                        .into_iter()
                        .map(|(who, amount)| (id.clone(), who, amount))
                })
                .collect(),
            next_asset_id: None,
            reserves: vec![],
        }
    }
}
