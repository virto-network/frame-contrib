use crate::DeviceId;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::PalletError;
use impl_trait_for_tuples::impl_for_tuples;
use scale_info::TypeInfo;

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
    fn claim(account_name: &AccountName, claimer: &AccountId) -> Result<(), RegistrarError>;
    fn claimer_pays_fees(account_name: &AccountName, claimer: &AccountId) -> bool;
}

#[impl_for_tuples(64)]
impl<AccountId, AccountName> Registrar<AccountId, AccountName> for Tuple {
    fn claim(account_name: &AccountName, claimer: &AccountId) -> Result<(), RegistrarError> {
        for_tuples!(#(
            match Tuple::claim(account_name, claimer) {
                Ok(_) => return Ok(()),
                Err(RegistrarError::CannotClaim) => (),
                Err(e) => return Err(e),
            }
        )*);
        Err(RegistrarError::CannotClaim)
    }
    fn claimer_pays_fees(account_name: &AccountName, claimer: &AccountId) -> bool {
        for_tuples!(#(
            match Tuple::claimer_pays_fees(account_name, claimer) {
                false => return false,
                _ => ()
            }
        )*);
        true
    }
}
