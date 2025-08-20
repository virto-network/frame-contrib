#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub use codec::{Decode, Encode, MaxEncodedLen};
use sp_io::hashing::blake2_256;

use alloc::vec::Vec;
use fc_traits_payments::{OnPaymentStatusChanged, PaymentMutate};
use frame_support::{
    ensure, fail,
    pallet_prelude::*,
    traits::{
        fungibles::{
            hold::Mutate as FunHoldMutate, Balanced as FunBalanced, Inspect as FunInspect,
            Mutate as FunMutate,
        },
        schedule::{v3::Named as ScheduleNamed, DispatchTime},
        tokens::{
            fungibles::Inspect as FunsInspect,
            Fortitude::Polite,
            Precision::Exact,
            Preservation::{Expendable, Preserve},
        },
        Bounded, CallerTrait, QueryPreimage, StorePreimage,
    },
    PalletId,
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
    traits::{BlockNumberProvider, CheckedAdd, CheckedSub, Get, StaticLookup},
    ArithmeticError, DispatchError, Percent, Saturating,
};
use types::BlockNumberFor;

pub mod weights;
pub use weights::*;

mod impls;
pub mod types;

pub use types::*;

pub trait GeneratePaymentId<AccountId> {
    type PaymentId;
    fn generate(sender: &AccountId, beneficiary: &AccountId) -> Option<Self::PaymentId>;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[cfg(feature = "runtime-benchmarks")]
    use frame_support::traits::fungibles::Create as FunCreate;

    #[pallet::config]
    pub trait Config:
        frame_system::Config<
        RuntimeEvent: From<Event<Self>> + TryInto<Event<Self>>,
        RuntimeCall: From<Call<Self>>,
    >
    {
        // Primitives: Some overarching types that come from the system (or the system depends on).

        /// The caller origin, overarching type of all pallets origins.
        type PalletsOrigin: From<frame_system::RawOrigin<Self::AccountId>>
            + CallerTrait<Self::AccountId>
            + MaxEncodedLen;
        /// The overarching hold reason.
        type RuntimeHoldReason: From<HoldReason>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        // Origins: Types that manage authorization rules to allow or deny some caller origins to
        // execute a method.
        type SenderOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;
        type BeneficiaryOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;
        type DisputeResolver: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        // Types: A set of parameter types that the pallet uses to handle information.

        type PaymentId: Parameter + Copy + MaxEncodedLen;

        // Dependencies: The external components this pallet depends on.

        #[cfg(not(feature = "runtime-benchmarks"))]
        /// Currency type that this works on.
        type Assets: FunInspect<Self::AccountId>
            + FunMutate<Self::AccountId>
            + FunBalanced<Self::AccountId>
            + FunsInspect<Self::AccountId>;
        #[cfg(feature = "runtime-benchmarks")]
        /// Currency type that this works on.
        type Assets: FunInspect<Self::AccountId>
            + FunCreate<Self::AccountId>
            + FunMutate<Self::AccountId>
            + FunBalanced<Self::AccountId>
            + FunsInspect<Self::AccountId>;
        type AssetsHold: FunHoldMutate<
            Self::AccountId,
            AssetId = AssetIdOf<Self>,
            Balance = BalanceOf<Self>,
            Reason = Self::RuntimeHoldReason,
        >;
        type BlockNumberProvider: BlockNumberProvider;
        type FeeHandler: FeeHandler<Self>;
        type Scheduler: ScheduleNamed<
            BlockNumberFor<Self>,
            CallOf<Self>,
            Self::PalletsOrigin,
            Hasher = Self::Hashing,
        >;
        /// The preimage provider used to look up call hashes to get the call.
        type Preimages: QueryPreimage<H = Self::Hashing> + StorePreimage;
        /// A hook that processes when the status of a payment changes.
        type OnPaymentStatusChanged: OnPaymentStatusChanged<Self::PaymentId, BalanceOf<Self>>;
        /// A provider that generates the `PaymentId` type given the parties.
        type GeneratePaymentId: GeneratePaymentId<Self::AccountId, PaymentId = Self::PaymentId>;

        // Parameters: A set of constant parameters to configure limits.

        #[pallet::constant]
        type PalletId: Get<PalletId>;
        #[pallet::constant]
        type IncentivePercentage: Get<Percent>;
        #[pallet::constant]
        type MaxRemarkLength: Get<u32>;
        #[pallet::constant]
        type MaxFees: Get<u32>;
        #[pallet::constant]
        type MaxDiscounts: Get<u32>;
        /// Buffer period - number of blocks to wait before user can claim
        /// canceled payment
        #[pallet::constant]
        type CancelBufferBlockLength: Get<BlockNumberFor<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    /// Payments created by a user, this method of storageDoubleMap is chosen
    /// since there is no usecase for listing payments by provider/currency. The
    /// payment will only be referenced by the creator in any transaction of
    /// interest. The storage map keys are the creator and the recipient, this
    /// also ensures that for any (sender,recipient) combo, only a single
    /// payment is active. The history of payment is not stored.
    pub type Payment<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        // Sender
        T::AccountId,
        Twox64Concat,
        T::PaymentId,
        PaymentDetail<T>,
        ResultQuery<Error<T>::NonExistentStorageValue>,
    >;

    #[pallet::storage]
    pub type PaymentParties<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::PaymentId,
        // Sender, Beneficiary pair
        (T::AccountId, T::AccountId),
        ResultQuery<Error<T>::NonExistentStorageValue>,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new payment has been created
        PaymentCreated {
            payment_id: T::PaymentId,
            asset: AssetIdOf<T>,
            amount: BalanceOf<T>,
            remark: Option<BoundedDataOf<T>>,
        },
        /// Payment amount released to the recipient
        PaymentReleased { payment_id: T::PaymentId },
        /// Payment has been cancelled by the creator
        PaymentCancelled { payment_id: T::PaymentId },
        /// A payment that NeedsReview has been resolved by Judge
        PaymentResolved {
            payment_id: T::PaymentId,
            recipient_share: Percent,
        },
        /// the payment creator has created a refund request
        PaymentCreatorRequestedRefund {
            payment_id: T::PaymentId,
            expiry: BlockNumberFor<T>,
        },
        /// the payment was refunded
        PaymentRefunded { payment_id: T::PaymentId },
        /// the refund request from creator was disputed by recipient
        PaymentRefundDisputed { payment_id: T::PaymentId },
        /// Payment request was created by recipient
        PaymentRequestCreated { payment_id: T::PaymentId },
        /// Payment request was completed by sender
        PaymentRequestCompleted { payment_id: T::PaymentId },
        /// Payment disputed resolved
        PaymentDisputeResolved { payment_id: T::PaymentId },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The selected payment does not exist
        InvalidPayment,
        /// The selected payment cannot be released
        PaymentAlreadyReleased,
        /// The selected payment already exists and is in process
        PaymentAlreadyInProcess,
        /// Action permitted only for whitelisted users
        InvalidAction,
        /// Payment is in review state and cannot be modified
        PaymentNeedsReview,
        /// Unexpeted math error
        MathError,
        /// Payment request has not been created
        RefundNotRequested,
        /// Dispute period has not passed
        DisputePeriodNotPassed,
        /// The automatic cancelation queue cannot accept
        RefundQueueFull,
        /// Release was not possible
        ReleaseFailed,
        /// Transfer failed
        TransferFailed,
        /// Storage Value does not exist
        NonExistentStorageValue,
        /// Unable to issue a payment id
        NoPaymentIdAvailable,
        /// Call from wrong beneficiary
        InvalidBeneficiary,
    }

    #[pallet::composite_enum]
    pub enum HoldReason {
        #[codec(index = 0)]
        TransferPayment,
    }

    #[pallet::call(weight(<T as Config>::WeightInfo))]
    impl<T: Config> Pallet<T> {
        /// This allows any user to create a new payment, that releases only to
        /// specified recipient. The only action is to store the details of this
        /// payment in storage and reserve the specified amount. User also has
        /// the option to add a remark, this remark can then be used to run
        /// custom logic and trigger alternate payment flows.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::pay(remark.as_ref().map(|x| x.len() as u32).unwrap_or(0))
		)]
        pub fn pay(
            origin: OriginFor<T>,
            beneficiary: AccountIdLookupOf<T>,
            asset: AssetIdOf<T>,
            #[pallet::compact] amount: BalanceOf<T>,
            remark: Option<BoundedDataOf<T>>,
        ) -> DispatchResult {
            let sender = T::SenderOrigin::ensure_origin(origin)?;
            let beneficiary = T::Lookup::lookup(beneficiary)?;

            Self::create(&sender, asset, amount, &beneficiary, remark)?;

            Ok(())
        }

        /// Release any created payment, this will transfer the reserved amount
        /// from the creator of the payment to the assigned recipient
        #[pallet::call_index(1)]
        pub fn release(origin: OriginFor<T>, payment_id: T::PaymentId) -> DispatchResult {
            let sender = T::SenderOrigin::ensure_origin(origin)?;

            // ensure the payment is in Created state
            let payment =
                Payment::<T>::get(&sender, &payment_id).map_err(|_| Error::<T>::InvalidPayment)?;
            ensure!(
                payment.state == PaymentState::Created,
                Error::<T>::InvalidAction
            );
            Self::settle_payment(&sender, &payment.beneficiary, &payment_id, None)?;

            let (_, total_beneficiary_fee_amount_mandatory, total_beneficiary_fee_amount_optional) =
                payment.fees.summary_for(Role::Beneficiary, false)?;

            let fees = total_beneficiary_fee_amount_mandatory
                .checked_add(&total_beneficiary_fee_amount_optional)
                .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

            let beneficiary_amount = payment
                .amount
                .checked_sub(&fees)
                .ok_or(DispatchError::Arithmetic(ArithmeticError::Underflow))?;

            T::OnPaymentStatusChanged::on_payment_released(&payment_id, fees, beneficiary_amount);

            Self::deposit_event(Event::PaymentReleased { payment_id });
            Ok(())
        }

        /// Allow the creator of a payment to initiate a refund that will return
        /// the funds after a configured amount of time that the receiver has to
        /// react and opose the request
        #[pallet::call_index(2)]
        pub fn request_refund(origin: OriginFor<T>, payment_id: T::PaymentId) -> DispatchResult {
            let sender = T::SenderOrigin::ensure_origin(origin)?;

            let expiry = Payment::<T>::try_mutate(
                &sender,
                &payment_id,
                |maybe_payment| -> Result<_, DispatchError> {
                    // ensure the payment exists
                    let payment = maybe_payment
                        .as_mut()
                        .map_err(|_| Error::<T>::InvalidPayment)?;
                    // refunds only possible for payments in created state
                    ensure!(
                        payment.state == PaymentState::Created,
                        Error::<T>::InvalidAction
                    );

                    // set the payment to requested refund
                    let current_block = T::BlockNumberProvider::current_block_number();
                    let cancel_block = current_block
                        .checked_add(&T::CancelBufferBlockLength::get())
                        .ok_or(Error::<T>::MathError)?;
                    let cancel_call =
                        T::RuntimeCall::from(pallet::Call::<T>::cancel { payment_id });

                    T::Scheduler::schedule_named(
                        ("payment", payment_id).using_encoded(blake2_256),
                        DispatchTime::At(cancel_block),
                        None,
                        63,
                        frame_system::RawOrigin::Signed(payment.beneficiary.clone()).into(),
                        T::Preimages::bound(cancel_call)?,
                    )?;

                    payment.state = PaymentState::RefundRequested { cancel_block };

                    Ok(cancel_block)
                },
            )?;

            Self::deposit_event(Event::PaymentCreatorRequestedRefund { payment_id, expiry });

            Ok(())
        }

        #[pallet::call_index(3)]
        pub fn accept_and_pay(origin: OriginFor<T>, payment_id: T::PaymentId) -> DispatchResult {
            let sender = T::SenderOrigin::ensure_origin(origin)?;
            let (_, beneficiary) = PaymentParties::<T>::get(&payment_id)?;

            Payment::<T>::try_mutate(
                &sender,
                payment_id,
                |maybe_payment| -> Result<_, DispatchError> {
                    let payment = maybe_payment
                        .as_mut()
                        .map_err(|_| Error::<T>::InvalidPayment)?;
                    const IS_DISPUTE: bool = false;

                    // Release sender fees recipients
                    let (
                        fee_sender_recipients,
                        _total_sender_fee_amount_mandatory,
                        _total_sender_fee_amount_optional,
                    ) = payment.fees.summary_for(Role::Sender, IS_DISPUTE)?;

                    let (
                        fee_beneficiary_recipients,
                        total_beneficiary_fee_amount_mandatory,
                        total_beneficiary_fee_amount_optional,
                    ) = payment.fees.summary_for(Role::Beneficiary, IS_DISPUTE)?;

                    Self::try_transfer_fees(&sender, payment, fee_sender_recipients, IS_DISPUTE)?;

                    T::Assets::transfer(
                        payment.asset.clone(),
                        &sender,
                        &beneficiary,
                        payment.amount,
                        Expendable,
                    )
                    .map_err(|_| Error::<T>::TransferFailed)?;

                    Self::try_transfer_fees(
                        &beneficiary,
                        payment,
                        fee_beneficiary_recipients,
                        IS_DISPUTE,
                    )?;

                    payment.state = PaymentState::Finished;

                    let fees = total_beneficiary_fee_amount_mandatory
                        .checked_add(&total_beneficiary_fee_amount_optional)
                        .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

                    let beneficiary_amount = payment
                        .amount
                        .checked_sub(&fees)
                        .ok_or(DispatchError::Arithmetic(ArithmeticError::Underflow))?;

                    // notify external systems about payment success
                    T::OnPaymentStatusChanged::on_payment_charge_success(
                        &payment_id,
                        fees,
                        beneficiary_amount,
                    );
                    T::OnPaymentStatusChanged::on_payment_released(
                        &payment_id,
                        fees,
                        beneficiary_amount,
                    );

                    Ok(())
                },
            )?;

            Self::deposit_event(Event::PaymentRequestCompleted { payment_id });
            Ok(())
        }

        /// Cancel a payment in created state, this will release the reserved
        /// back to creator of the payment. This extrinsic can only be called by
        /// the recipient of the payment
        #[pallet::call_index(10)]
        pub fn cancel(origin: OriginFor<T>, payment_id: T::PaymentId) -> DispatchResult {
            let beneficiary = T::BeneficiaryOrigin::ensure_origin(origin)?;
            let (sender, b) = PaymentParties::<T>::get(&payment_id)?;
            ensure!(beneficiary == b, Error::<T>::InvalidBeneficiary);

            let payment =
                Payment::<T>::get(&sender, &payment_id).map_err(|_| Error::<T>::InvalidPayment)?;

            match payment.state {
                PaymentState::Created => {
                    Self::cancel_payment(&sender, payment)?;
                    Self::deposit_event(Event::PaymentCancelled { payment_id });
                }
                PaymentState::RefundRequested { cancel_block: _ } => {
                    Self::cancel_payment(&sender, payment)?;
                    Self::deposit_event(Event::PaymentRefunded { payment_id });
                }
                _ => fail!(Error::<T>::InvalidAction),
            }

            T::OnPaymentStatusChanged::on_payment_cancelled(&payment_id);

            Payment::<T>::remove(&sender, &payment_id);
            PaymentParties::<T>::remove(payment_id);

            Ok(())
        }

        /// Allow payment beneficiary to dispute the refund request from the
        /// payment creator This does not cancel the request, instead sends the
        /// payment to a NeedsReview state The assigned resolver account can
        /// then change the state of the payment after review.
        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::dispute_refund())]
        pub fn dispute_refund(origin: OriginFor<T>, payment_id: T::PaymentId) -> DispatchResult {
            let beneficiary = T::BeneficiaryOrigin::ensure_origin(origin)?;
            let (sender, b) = PaymentParties::<T>::get(&payment_id)?;
            ensure!(beneficiary == b, Error::<T>::InvalidBeneficiary);

            Payment::<T>::try_mutate(
                &sender,
                &payment_id,
                |maybe_payment| -> Result<_, DispatchError> {
                    // ensure the payment exists
                    let payment = maybe_payment
                        .as_mut()
                        .map_err(|_| Error::<T>::InvalidPayment)?;

                    // ensure the payment is in Requested Refund state
                    let PaymentState::RefundRequested { cancel_block } = payment.state else {
                        fail!(Error::<T>::InvalidAction);
                    };
                    ensure!(
                        cancel_block > T::BlockNumberProvider::current_block_number(),
                        Error::<T>::InvalidAction
                    );

                    // Hold beneficiary incentive amount to balance the incentives at the time to
                    // resolve the dispute
                    let reason = &HoldReason::TransferPayment.into();
                    T::AssetsHold::hold(
                        payment.asset.clone(),
                        reason,
                        &beneficiary,
                        payment.incentive_amount,
                    )?;

                    payment.state = PaymentState::NeedsReview;

                    T::Scheduler::cancel_named(("payment", payment_id).using_encoded(blake2_256))
                },
            )?;

            Self::deposit_event(Event::PaymentRefundDisputed { payment_id });
            Ok(())
        }

        // Creates a new payment with the given details. This can be called by the
        // recipient of the payment to create a payment and then completed by the sender
        // using the `accept_and_pay` extrinsic.  The payment will be in
        // PaymentRequested State and can only be modified by the `accept_and_pay`
        // extrinsic.
        #[pallet::call_index(12)]
        pub fn request_payment(
            origin: OriginFor<T>,
            sender: AccountIdLookupOf<T>,
            asset: AssetIdOf<T>,
            #[pallet::compact] amount: BalanceOf<T>,
        ) -> DispatchResult {
            let beneficiary = T::BeneficiaryOrigin::ensure_origin(origin)?;
            let sender = T::Lookup::lookup(sender)?;
            // create PaymentDetail and add to storage
            let (payment_id, _) = Self::do_create_payment(
                &sender,
                beneficiary,
                asset,
                amount,
                PaymentState::PaymentRequested,
                T::IncentivePercentage::get(),
                None,
            )?;

            Self::deposit_event(Event::PaymentRequestCreated { payment_id });

            Ok(())
        }

        #[pallet::call_index(20)]
        pub fn resolve_dispute(
            origin: OriginFor<T>,
            payment_id: T::PaymentId,
            dispute_result: DisputeResult,
        ) -> DispatchResult {
            let dispute_resolver = T::DisputeResolver::ensure_origin(origin)?;
            let (sender, beneficiary) = PaymentParties::<T>::get(&payment_id)?;

            let payment =
                Payment::<T>::get(&sender, &payment_id).map_err(|_| Error::<T>::InvalidPayment)?;
            ensure!(
                payment.state == PaymentState::NeedsReview,
                Error::<T>::InvalidAction
            );

            let dispute = Some((dispute_result, dispute_resolver));
            Self::settle_payment(&sender, &beneficiary, &payment_id, dispute)?;

            Self::deposit_event(Event::PaymentDisputeResolved { payment_id });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The function will create a new payment. The fee and incentive
    /// amounts will be calculated and the `PaymentDetail` will be added to
    /// storage.
    #[allow(clippy::too_many_arguments)]
    fn do_create_payment(
        sender: &T::AccountId,
        beneficiary: T::AccountId,
        asset: AssetIdOf<T>,
        amount: BalanceOf<T>,
        payment_state: PaymentState<BlockNumberFor<T>>,
        incentive_percentage: Percent,
        remark: Option<&[u8]>,
    ) -> Result<(T::PaymentId, PaymentDetail<T>), DispatchError> {
        let payment_id = T::GeneratePaymentId::generate(sender, &beneficiary)
            .ok_or(Error::<T>::NoPaymentIdAvailable)?;
        Payment::<T>::try_mutate(
            sender,
            payment_id,
            |maybe_payment| -> Result<_, DispatchError> {
                if let Ok(payment) = maybe_payment {
                    ensure!(
                        payment.state == PaymentState::PaymentRequested,
                        Error::<T>::PaymentAlreadyInProcess
                    );
                }

                let incentive_amount = incentive_percentage.mul_floor(amount);

                let fees_details: Fees<T> =
                    T::FeeHandler::apply_fees(&asset, sender, &beneficiary, &amount, remark);

                let new_payment = PaymentDetail::<T> {
                    asset,
                    amount,
                    beneficiary: beneficiary.clone(),
                    incentive_amount,
                    state: payment_state,
                    fees: fees_details,
                };
                *maybe_payment = Ok(new_payment.clone());
                PaymentParties::<T>::insert(payment_id, (sender, beneficiary));

                T::OnPaymentStatusChanged::on_payment_created(&payment_id);

                Ok(new_payment)
            },
        )
        .map(|payment| (payment_id, payment))
    }

    fn reserve_payment_amount(sender: &T::AccountId, payment: &PaymentDetail<T>) -> DispatchResult {
        let (_fee_recipients, total_fee_from_sender_mandatory, total_fee_from_sender_optional) =
            payment.fees.summary_for(Role::Sender, false)?;

        let total_hold_amount = total_fee_from_sender_mandatory
            .saturating_add(payment.incentive_amount)
            .saturating_add(total_fee_from_sender_optional);
        let reason = &HoldReason::TransferPayment.into();
        T::AssetsHold::hold(payment.asset.clone(), reason, sender, total_hold_amount)?;

        T::AssetsHold::transfer_and_hold(
            payment.asset.clone(),
            reason,
            sender,
            &payment.beneficiary,
            payment.amount,
            Exact,
            Preserve,
            Polite,
        )?;

        Ok(())
    }

    fn cancel_payment(sender: &T::AccountId, payment: PaymentDetail<T>) -> DispatchResult {
        let (_fee_recipients, total_fee_from_sender_mandatory, total_fee_from_sender_optional) =
            payment.fees.summary_for(Role::Sender, false)?;

        let total_hold_amount = total_fee_from_sender_mandatory
            .saturating_add(payment.incentive_amount)
            .saturating_add(total_fee_from_sender_optional);
        let reason = &HoldReason::TransferPayment.into();

        T::AssetsHold::release(
            payment.asset.clone(),
            reason,
            sender,
            total_hold_amount,
            Exact,
        )
        .map_err(|_| Error::<T>::ReleaseFailed)?;

        let beneficiary = &payment.beneficiary;
        T::AssetsHold::release(
            payment.asset.clone(),
            reason,
            beneficiary,
            payment.amount,
            Exact,
        )
        .map_err(|_| Error::<T>::ReleaseFailed)?;

        T::Assets::transfer(
            payment.asset,
            beneficiary,
            sender,
            payment.amount,
            Expendable,
        )
        .map_err(|_| Error::<T>::TransferFailed)?;

        Ok(())
    }

    fn settle_payment(
        sender: &T::AccountId,
        beneficiary: &T::AccountId,
        payment_id: &T::PaymentId,
        maybe_dispute: Option<(DisputeResult, T::AccountId)>,
    ) -> DispatchResult {
        Payment::<T>::try_mutate(sender, payment_id, |maybe_payment| -> DispatchResult {
            let payment = maybe_payment
                .as_mut()
                .map_err(|_| Error::<T>::InvalidPayment)?;

            let reason = &HoldReason::TransferPayment.into();
            let is_dispute = maybe_dispute.is_some();

            // Release sender fees recipients
            let (
                fee_sender_recipients,
                total_sender_fee_amount_mandatory,
                total_sender_fee_amount_optional,
            ) = payment.fees.summary_for(Role::Sender, is_dispute)?;

            let total_sender_release = total_sender_fee_amount_mandatory
                .saturating_add(payment.incentive_amount)
                .saturating_add(total_sender_fee_amount_optional);

            T::AssetsHold::release(
                payment.asset.clone(),
                reason,
                sender,
                total_sender_release,
                Exact,
            )
            .map_err(|_| Error::<T>::ReleaseFailed)?;

            let (
                fee_beneficiary_recipients,
                total_beneficiary_fee_amount_mandatory,
                total_beneficiary_fee_amount_optional,
            ) = payment.fees.summary_for(Role::Beneficiary, is_dispute)?;

            let mut beneficiary_release_amount = payment.amount;

            if is_dispute {
                beneficiary_release_amount =
                    beneficiary_release_amount.saturating_add(payment.incentive_amount);
            }

            T::AssetsHold::release(
                payment.asset.clone(),
                reason,
                beneficiary,
                beneficiary_release_amount,
                Exact,
            )
            .map_err(|_| Error::<T>::ReleaseFailed)?;

            Self::try_transfer_fees(sender, payment, fee_sender_recipients, is_dispute)?;

            Self::try_transfer_fees(beneficiary, payment, fee_beneficiary_recipients, is_dispute)?;

            if let Some((dispute_result, resolver)) = maybe_dispute {
                match dispute_result.in_favor_of {
                    Role::Sender => {
                        let amount_to_sender =
                            dispute_result.percent_beneficiary.mul_floor(payment.amount);

                        // Beneficiary looses the dispute and has to transfer the incentive_amount to
                        // the dispute_resolver.
                        T::Assets::transfer(
                            payment.asset.clone(),
                            beneficiary,
                            &resolver,
                            payment.incentive_amount,
                            Expendable,
                        )
                        .map_err(|_| Error::<T>::TransferFailed)?;

                        T::Assets::transfer(
                            payment.asset.clone(),
                            beneficiary,
                            sender,
                            amount_to_sender,
                            Expendable,
                        )
                        .map_err(|_| Error::<T>::TransferFailed)?;
                    }
                    Role::Beneficiary => {
                        let amount_to_beneficiary =
                            dispute_result.percent_beneficiary.mul_floor(payment.amount);
                        let amount_to_sender = payment.amount.saturating_sub(amount_to_beneficiary);

                        T::Assets::transfer(
                            payment.asset.clone(),
                            sender,
                            &resolver,
                            payment.incentive_amount,
                            Expendable,
                        )
                        .map_err(|_| Error::<T>::TransferFailed)?;

                        T::Assets::transfer(
                            payment.asset.clone(),
                            beneficiary,
                            sender,
                            amount_to_sender,
                            Expendable,
                        )
                        .map_err(|_| Error::<T>::TransferFailed)?;

                        let fees = total_beneficiary_fee_amount_mandatory
                            .checked_add(&total_beneficiary_fee_amount_optional)
                            .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

                        T::OnPaymentStatusChanged::on_payment_released(
                            payment_id,
                            fees,
                            amount_to_beneficiary
                                .checked_add(&payment.incentive_amount)
                                .ok_or(DispatchError::Arithmetic(ArithmeticError::Underflow))?
                                .checked_sub(&total_beneficiary_fee_amount_mandatory)
                                .ok_or(DispatchError::Arithmetic(ArithmeticError::Underflow))?,
                        );
                    }
                }
            }

            payment.state = PaymentState::Finished;
            Ok(())
        })
    }

    fn try_transfer_fees(
        account: &T::AccountId,
        payment: &PaymentDetail<T>,
        fee_recipients: Vec<Fee<T>>,
        is_dispute: bool,
    ) -> DispatchResult {
        for (recipient_account, fee_amount, mandatory) in fee_recipients.iter() {
            if !is_dispute || *mandatory {
                T::Assets::transfer(
                    payment.asset.clone(),
                    account,
                    recipient_account,
                    *fee_amount,
                    Preserve,
                )
                .map_err(|_| Error::<T>::TransferFailed)?;
            }
        }
        Ok(())
    }
}
