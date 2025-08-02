use super::*;
use pallet_revive::{AddressMapper, Config as ReviveConfig};

pub struct AddressMapperWithBlackHole<T, M>(PhantomData<(T, M)>);

impl<T: Config + ReviveConfig, M: AddressMapper<T>> AddressMapper<T>
    for AddressMapperWithBlackHole<T, M>
{
    fn to_address(account_id: &T::AccountId) -> H160 {
        if account_id.eq(&Pallet::<T>::event_horizon()) {
            return H160([0; 20]);
        }
        M::to_address(account_id)
    }

    fn to_account_id(address: &H160) -> T::AccountId {
        if address.is_zero() {
            return Pallet::<T>::event_horizon();
        }
        M::to_account_id(&address)
    }

    fn to_fallback_account_id(address: &H160) -> T::AccountId {
        if address.is_zero() {
            return Pallet::<T>::event_horizon();
        }
        M::to_fallback_account_id(&address)
    }

    fn map(account_id: &T::AccountId) -> frame::deps::sp_runtime::DispatchResult {
        M::map(account_id)
    }

    fn unmap(account_id: &T::AccountId) -> frame::deps::sp_runtime::DispatchResult {
        M::unmap(account_id)
    }

    fn is_mapped(account_id: &T::AccountId) -> bool {
        account_id.eq(&Pallet::<T>::event_horizon()) || M::is_mapped(account_id)
    }
}
