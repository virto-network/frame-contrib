#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use frame_support::sp_runtime::DispatchError;
use frame_support::weights::Weight;
pub use Mutate as PaymentMutate;

/// Represents a payment.
pub struct Payment<AccountId, Asset, Balance> {
    beneficiary: AccountId,
    asset: Asset,
    amount: Balance,
    remark: Option<Vec<u8>>,
    charge_type: ChargeType<Balance>,
}

pub enum ChargeType<Balance> {
    /// The payment is a single charge
    Single,
    /// The payment is a recurring charge. A maximum amount is authorized.
    Recurring { max: Balance },
}

impl<AccountId, Asset, Balance: Copy> Payment<AccountId, Asset, Balance> {
    pub fn new(
        beneficiary: AccountId,
        asset: Asset,
        amount: Balance,
        remark: Option<Vec<u8>>,
        charge_type: ChargeType<Balance>,
    ) -> Self {
        Self {
            beneficiary,
            asset,
            amount,
            remark,
            charge_type,
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

    pub fn remark(&self) -> Option<Vec<u8>> {
        self.remark.clone()
    }

    pub fn status(&self) -> Option<Vec<u8>> {
        self.remark.clone()
    }

    pub fn charge_type(&self) -> &ChargeType<Balance> {
        &self.charge_type
    }
}

pub trait Inspect<AccountId> {
    type Id;
    type AssetId;
    type Balance;

    /// Given an `Id`, returns the details of a payment.
    fn details(id: Self::AssetId) -> Payment<AccountId, Self::AssetId, Self::Balance>;
}

pub trait Mutate<AccountId>: Inspect<AccountId> {
    /// Creates a new payment.
    fn create(
        asset: Self::AssetId,
        amount: Self::Balance,
        remark: Option<Vec<u8>>,
        beneficiary: &AccountId,
    ) -> Result<Self::Id, DispatchError>;

    /// Creates a new recurring payment.
    fn create_recurring(
        asset: Self::AssetId,
        max: Self::Balance,
        remark: Option<Vec<u8>>,
        beneficiary: &AccountId,
    ) -> Result<Self::Id, DispatchError>;

    /// Charges a recurring payment. Cannot exceed initially stated max.
    fn charge_recurring_payment(
        id: Self::Id,
        amount: Self::Balance,
    ) -> Result<Self::Id, DispatchError>;
}

pub trait OnPaymentStatusChanged<Id, Balance> {
    /// Executes an action when a payment is created
    fn on_payment_created(_id: Id) -> Weight {
        Weight::default()
    }
    /// Executes an action when a payment is successfully completed.
    fn on_payment_success(_id: Id, _fees: Balance, _resulting_amount: Balance) -> Weight {
        Weight::default()
    }
    /// Executes an action when a payment charge is successfully completed.
    fn on_payment_charge_success(_id: Id, _fees: Balance, _resulting_amount: Balance) -> Weight {
        Weight::default()
    }
    /// Executes an action when a payment is cancelled.
    fn on_payment_aborted(_id: Id) -> Weight {
        Weight::default()
    }
    /// Executes an action when a payment is cancelled.
    fn on_payment_cancelled(_id: Id) -> Weight {
        Weight::default()
    }
}
