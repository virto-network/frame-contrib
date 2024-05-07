use crate::DeviceId;
use impl_trait_for_tuples::impl_for_tuples;

pub enum ClaimError {
    CannotClaim,
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
    fn claim(account_name: &AccountName, claimer: &AccountId) -> Result<(), ClaimError>;
    fn claimer_pays_fees(account_name: &AccountName, claimer: &AccountId) -> bool;
}

#[impl_for_tuples(64)]
impl<AccountId, AccountName> Registrar<AccountId, AccountName> for Tuple {
    fn claim(account_name: &AccountName, claimer: &AccountId) -> Result<(), ClaimError> {
        for_tuples!(#(
            match Tuple::claim(account_name, claimer) {
                Ok(_) => return Ok(()),
                _ => ()
            }
        )*);
        Err(ClaimError::CannotClaim)
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
