use crate::DeviceId;

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
    fn claim(account_name: AccountName, claimer: AccountId) -> Result<(), ClaimError>;
    fn claimer_pays_fees(account_name: AccountName, claimer: AccountId) -> bool;
}

// impl<AccountId, AccountName> Registrar<AccountId, AccountName> for () {
//     fn claim(_account_name: AccountName, _claimer: AccountId) -> Result<(), ClaimError> {
//         Err(ClaimError::CannotClaim)
//     }
//     fn claimer_pays_fees(_account_name: AccountName, _claimer: AccountId) -> bool {
//         true
//     }
// }
