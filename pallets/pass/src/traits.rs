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
    fn is_claimable(account_name: &AccountName, claimer: &AccountId) -> bool;
    fn claimer_pays_fees(account_name: &AccountName, claimer: &AccountId) -> bool;

    fn register_claim(
        account_name: &AccountName,
        claimer: &AccountId,
    ) -> Result<(), RegistrarError>;
    fn initialize_account(account_name: &AccountName) -> Result<(), RegistrarError>;

    fn claim(account_name: &AccountName, claimer: &AccountId) -> Result<(), RegistrarError> {
        if !Self::is_claimable(account_name, claimer) {
            return Err(RegistrarError::CannotClaim);
        }

        Self::register_claim(account_name, claimer)?;
        Self::initialize_account(account_name)?;
        Ok(())
    }
}

#[impl_for_tuples(64)]
impl<AccountId, AccountName> Registrar<AccountId, AccountName> for Tuple {
    fn is_claimable(_: &AccountName, _: &AccountId) -> bool {
        unimplemented!("This implementation is bound to each of the types within the tuple implementing the trait");
    }

    fn claimer_pays_fees(account_name: &AccountName, claimer: &AccountId) -> bool {
        unimplemented!("This implementation is bound to each of the types within the tuple implementing the trait");
    }

    fn register_claim(
        account_name: &AccountName,
        claimer: &AccountId,
    ) -> Result<(), RegistrarError> {
        unimplemented!("This implementation is bound to each of the types within the tuple implementing the trait");
    }

    fn initialize_account(account_name: &AccountName) -> Result<(), RegistrarError> {
        unimplemented!("This implementation is bound to each of the types within the tuple implementing the trait");
    }

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
}
