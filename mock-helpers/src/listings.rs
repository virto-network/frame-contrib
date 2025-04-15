use crate::ExtHelper;
use fc_pallet_listings::{pallet::Config, GenesisConfig, ItemPrice, ItemPriceOf, ItemType};
use frame_support::DefaultNoBound;
use sp_runtime::BuildStorage;

pub struct Item<SKU, Price> {
    id: ItemType<SKU>,
    name: Vec<u8>,
    price: Option<Price>,
    transferable: bool,
    for_resale: bool,
}

impl<SKU, Price> Item<SKU, Price> {
    pub fn new(id: ItemType<SKU>, name: Vec<u8>, price: Option<Price>) -> Self {
        Self {
            id,
            name,
            price,
            transferable: true,
            for_resale: true,
        }
    }

    pub fn not_transferable(mut self) -> Self {
        self.transferable = false;
        self
    }

    pub fn not_for_resale(mut self) -> Self {
        self.for_resale = false;
        self
    }
}

pub struct Inventory<AccountId, MerchantId, Id, ItemSKU, Price> {
    id: (MerchantId, Id),
    owner: AccountId,
    items: Vec<Item<ItemSKU, Price>>,
}

impl<AccountId, MerchantId, Id, ItemId, Price> Inventory<AccountId, MerchantId, Id, ItemId, Price> {
    pub fn new(id: (MerchantId, Id), owner: AccountId) -> Self {
        Self {
            id,
            owner,
            items: Vec::new(),
        }
    }

    pub fn with_item(mut self, item: Item<ItemId, Price>) -> Self {
        self.items.push(item);
        self
    }
}

pub type InventoryOf<T, I = ()> = Inventory<
    <T as frame_system::Config>::AccountId,
    <T as Config<I>>::MerchantId,
    <T as Config<I>>::InventoryId,
    <T as Config<I>>::ItemSKU,
    ItemPriceOf<T, I>,
>;

#[derive(DefaultNoBound)]
pub struct ListingsExtBuilder<T: Config<I>, I: 'static = ()> {
    inventories: Vec<InventoryOf<T, I>>,
}

impl<T: Config<I>, I: 'static> ListingsExtBuilder<T, I> {
    pub fn with_inventory(mut self, inventory: InventoryOf<T, I>) -> Self {
        self.inventories.push(inventory);
        self
    }
}

impl<T: Config<I>, I: 'static> ExtHelper for ListingsExtBuilder<T, I> {
    fn as_storage(&self) -> impl BuildStorage {
        GenesisConfig::<T, I> {
            inventories: self
                .inventories
                .iter()
                .map(
                    |Inventory {
                         id: (merchant_id, id),
                         owner,
                         ..
                     }| (*merchant_id, *id, owner.clone()),
                )
                .collect(),
            items: self
                .inventories
                .iter()
                .flat_map(
                    |Inventory {
                         id: inventory_id,
                         items,
                         ..
                     }| {
                        items.iter().map(
                            |Item {
                                 id,
                                 name,
                                 price,
                                 transferable,
                                 for_resale: not_for_resale,
                             }| {
                                (
                                    *inventory_id,
                                    *id,
                                    name.clone(),
                                    price
                                        .clone()
                                        .map(|ItemPrice { asset, amount }| (asset, amount)),
                                    *transferable,
                                    *not_for_resale,
                                )
                            },
                        )
                    },
                )
                .collect(),
        }
    }
}
