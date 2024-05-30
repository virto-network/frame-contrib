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
#[derive(Clone, Copy, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Debug)]
pub struct Account<AccountId> {
    pub account_id: AccountId,
    pub status: AccountStatus,
}

impl<AccountId> Account<AccountId> {
    pub fn new(account_id: AccountId, status: AccountStatus) -> Self {
        Self { account_id, status }
    }

    pub fn is_unitialized(&self) -> bool {
        matches!(self.status, AccountStatus::Uninitialized)
    }
}

#[derive(Clone, Copy, Encode, Decode, MaxEncodedLen, TypeInfo, Debug, PartialEq)]
pub enum AccountStatus {
    Uninitialized,
    Active,
}
