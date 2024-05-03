use crate::Config;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::BoundedVec;
use frame_system::Config as SystemConfig;
use scale_info::TypeInfo;

pub type AccountIdOf<T> = <T as SystemConfig>::AccountId;
pub type AccountName<T, I> = BoundedVec<u8, <T as Config<I>>::MaxAccountNameLen>;
pub type DeviceId = [u8; 32];
pub type DeviceDescriptor<T, I> = BoundedVec<u8, <T as Config<I>>::MaxDeviceDescriptorLen>;

pub type AccountOf<T> = Account<AccountIdOf<T>>;
#[derive(Clone, Copy, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct Account<AccountId> {
    account_id: AccountId,
    status: AccountStatus,
}

impl<AccountId> Account<AccountId> {
    pub fn new(account_id: AccountId, status: AccountStatus) -> Self {
        Self { account_id, status }
    }
}

#[derive(Clone, Copy, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum AccountStatus {
    Uninitialized,
    Active,
}
