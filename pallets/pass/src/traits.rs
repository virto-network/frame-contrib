use crate::DeviceId;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::PalletError;
use scale_info::TypeInfo;
use crate::types;


#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum RegistrarError {
    CannotClaim,
    CannotInitialize,
    AlreadyRegistered,
}

impl PalletError for RegistrarError {
    const MAX_ENCODED_SIZE: usize = 1;
}

pub enum AuthenticateError {
    ChallengeFailed,
}

pub trait Authenticator {
    fn get_device_id(&self, device: Vec<u8>) -> Option<DeviceId>;
    fn authenticate(
        &self,
        device: Vec<u8>,
        challenge: &[u8],
        payload: &[u8],
    ) -> Result<(), AuthenticateError>;
}

pub trait Registrar<AccountId, AccountName> {
    fn is_claimable(account_name: &AccountName, claimer: &AccountId) -> types::RegistrarResult;
    fn claim(account_name: &AccountName, claimer: &AccountId) -> types::RegistrarResult;
    fn claimer_pays_fees(account_name: &AccountName, claimer: &AccountId) -> bool;
}
