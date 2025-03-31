#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use codec::{Encode, MaxEncodedLen};
use frame_support::pallet_prelude::Member;
use frame_support::sp_runtime::DispatchError;
use impl_trait_for_tuples::impl_for_tuples;

pub use {Inspect as PaymentInspect, Mutate as PaymentMutate};

/// Represents a payment.
pub struct Payment<AccountId, Asset, Balance> {
    beneficiary: AccountId,
    asset: Asset,
    amount: Balance,
}

impl<AccountId, Asset, Balance: Copy> Payment<AccountId, Asset, Balance> {
    pub fn new(beneficiary: AccountId, asset: Asset, amount: Balance) -> Self {
        Self {
            beneficiary,
            asset,
            amount,
        }
    }

    pub fn beneficiary(&self) -> &AccountId {
        &self.beneficiary
    }

    pub fn asset(&self) -> &Asset {
        &self.asset
    }

    pub fn amount(&self) -> Balance {
        self.amount
    }
}

pub trait Inspect<AccountId> {
    type Id: Member + MaxEncodedLen;
    type AssetId;
    type Balance;

    /// Given an `Id`, returns the details of a payment.
    fn details(id: Self::Id) -> Option<Payment<AccountId, Self::AssetId, Self::Balance>>;
}

pub trait Mutate<AccountId>: Inspect<AccountId> {
    /// Creates a new payment.
    fn create<Details: Encode>(
        creator: &AccountId,
        asset: Self::AssetId,
        amount: Self::Balance,
        beneficiary: &AccountId,
        details: Option<Details>,
    ) -> Result<Self::Id, DispatchError>;
}

pub trait OnPaymentStatusChanged<Id, Balance: Copy> {
    /// Notifies whenever a payment is created
    fn on_payment_created(_id: &Id) {}
    /// Notifies whenever a payment charge is completed successfully.
    fn on_payment_charge_success(_id: &Id, _fees: Balance, _resulting_amount: Balance) {}
    /// Notifies whenever a payment is cancelled.
    fn on_payment_cancelled(_id: &Id) {}
    /// Notifies whenever a payment is successfully released to the beneficiary.
    fn on_payment_released(_id: &Id, _fees: Balance, _resulting_amount: Balance) {}
}

#[impl_for_tuples(64)]
impl<Id, Balance: Copy> OnPaymentStatusChanged<Id, Balance> for Tuple {
    fn on_payment_created(id: &Id) {
        for_tuples!(
            #( Tuple::on_payment_created(id); )*
        )
    }

    fn on_payment_released(id: &Id, fees: Balance, resulting_amount: Balance) {
        for_tuples!(
            #( Tuple::on_payment_released(id, fees, resulting_amount); )*
        )
    }

    fn on_payment_charge_success(id: &Id, fees: Balance, resulting_amount: Balance) {
        for_tuples!(
            #( Tuple::on_payment_charge_success(id, fees, resulting_amount); )*
        )
    }

    fn on_payment_cancelled(id: &Id) {
        for_tuples!(
            #( Tuple::on_payment_cancelled(id); )*
        )
    }
}
