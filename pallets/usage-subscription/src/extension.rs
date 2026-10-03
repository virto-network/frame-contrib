//! The payment step of usage subscriptions: [`ChargeUsageSubscription`] (`DEC-24`).

use super::*;
use codec::{Decode, DecodeWithMemTracking, Encode};
use core::{fmt, marker::PhantomData};
use frame_support::dispatch::{DispatchInfo, PostDispatchInfo};
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::{
    traits::{
        AsSystemOriginSigner, DispatchInfoOf, DispatchOriginOf, Dispatchable, Implication,
        PostDispatchInfoOf, TransactionExtension, TransactionExtensionMetadata, ValidateResult,
    },
    transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
    },
};

/// The custom [`InvalidTransaction`] code [`ChargeUsageSubscription`] rejects a transaction with
/// when preparation disagrees with validation (`ERR-PathMismatch`, `CTR-FEE-4`): validation chose
/// the pool path, and in preparation the check no longer gives the same ticket.
///
/// The transaction is then invalid, as if it had failed validation. It is never prepared on the
/// fee path, which validation did not check, and nothing panics (`INV-15`).
pub const PATH_MISMATCH: u8 = 1;

/// Which path a transaction takes, carried from validation to preparation (`Val`) and from
/// preparation to post-dispatch (`Pre`).
pub enum Path<P, F> {
    /// The pool pays: no fee, and the actual metered weight is charged to the pool.
    Pool(P),
    /// The fee path: the inner extension's own value.
    Fee(F),
}

impl<P, F> fmt::Debug for Path<P, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Path::Pool(_) => write!(f, "Path::Pool"),
            Path::Fee(_) => write!(f, "Path::Fee"),
        }
    }
}

/// A transaction extension that lets a member's group pool pay for a signed transaction when
/// admission holds (`REQ-PL-7`), and otherwise defers to the fee extension `S`, unchanged.
///
/// It is invisible in metadata: its identifier, implicit data, type information and encoding are
/// `S`'s. See the [crate documentation](crate) for how to integrate it.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq)]
pub struct ChargeUsageSubscription<T, S>(pub S, PhantomData<T>);

impl<T, S: StaticTypeInfo> TypeInfo for ChargeUsageSubscription<T, S> {
    type Identity = S;
    fn type_info() -> scale_info::Type {
        S::type_info()
    }
}

impl<T, S: Encode> fmt::Debug for ChargeUsageSubscription<T, S> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ChargeUsageSubscription<{:?}>", self.0.encode())
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

impl<T, S> ChargeUsageSubscription<T, S> {
    /// Wraps the fee extension `s`.
    pub fn new(s: S) -> Self {
        Self(s, PhantomData)
    }
}

impl<T, S> From<S> for ChargeUsageSubscription<T, S> {
    fn from(s: S) -> Self {
        Self::new(s)
    }
}

impl<T: Config, S> ChargeUsageSubscription<T, S>
where
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    /// The metered weight `CheckWeight` books for the transaction before dispatch: its declared
    /// call and extension weights, plus the base extrinsic weight of its class, plus its length
    /// as proof size (`REQ-PL-6`).
    pub fn estimate(info: &DispatchInfo, len: usize) -> Weight {
        frame_system::calculate_consumed_extrinsic_weight::<T::RuntimeCall>(
            &<T as frame_system::Config>::BlockWeights::get(),
            info,
            len,
        )
    }

    /// The same parts after dispatch: the call and extension weights left after refunds, plus
    /// the same base and length, capped at the [estimate](Self::estimate).
    pub fn actual(info: &DispatchInfo, post_info: &PostDispatchInfo, len: usize) -> Weight {
        post_info
            .calc_actual_weight(info)
            .saturating_add(
                <T as frame_system::Config>::BlockWeights::get()
                    .get(info.class)
                    .base_extrinsic,
            )
            .saturating_add_proof_size(len as u64)
            .min(Self::estimate(info, len))
    }
}

impl<T, S> TransactionExtension<T::RuntimeCall> for ChargeUsageSubscription<T, S>
where
    T: Config + Send + Sync,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    DispatchOriginOf<T::RuntimeCall>: AsSystemOriginSigner<T::AccountId>,
    S: TransactionExtension<T::RuntimeCall> + StaticTypeInfo,
{
    const IDENTIFIER: &'static str = S::IDENTIFIER;
    type Implicit = S::Implicit;
    /// The pool path with its ticket, or the fee path with the inner extension's value.
    type Val = Path<Ticket<T>, S::Val>;
    /// The pool path with its ticket, or the fee path with the inner extension's value.
    type Pre = Path<Ticket<T>, S::Pre>;

    fn metadata() -> alloc::vec::Vec<TransactionExtensionMetadata> {
        S::metadata()
    }

    fn implicit(&self) -> Result<Self::Implicit, TransactionValidityError> {
        self.0.implicit()
    }

    /// The worst of both paths (`CTR-FEE-6`): the pool path (the check twice and the charge), or
    /// the check and the fee path.
    fn weight(&self, call: &T::RuntimeCall) -> Weight {
        T::WeightInfo::pool_path()
            .max(T::WeightInfo::fee_path_check().saturating_add(self.0.weight(call)))
    }

    fn validate(
        &self,
        origin: DispatchOriginOf<T::RuntimeCall>,
        call: &T::RuntimeCall,
        info: &DispatchInfoOf<T::RuntimeCall>,
        len: usize,
        self_implicit: Self::Implicit,
        inherited_implication: &impl Implication,
        source: TransactionSource,
    ) -> ValidateResult<Self::Val, T::RuntimeCall> {
        if let Some(who) = origin.as_system_origin_signer().cloned() {
            if let Ok(ticket) = Pallet::<T>::check(&who, Self::estimate(info, len)) {
                // No fee, no tip, and no priority above a zero-tip fee-path transaction
                // (`REQ-PL-10`). Nothing was written (`INV-1`).
                return Ok((ValidTransaction::default(), Path::Pool(ticket), origin));
            }
        }

        self.0
            .validate(
                origin,
                call,
                info,
                len,
                self_implicit,
                inherited_implication,
                source,
            )
            .map(|(valid, val, origin)| (valid, Path::Fee(val), origin))
    }

    fn prepare(
        self,
        val: Self::Val,
        origin: &DispatchOriginOf<T::RuntimeCall>,
        call: &T::RuntimeCall,
        info: &DispatchInfoOf<T::RuntimeCall>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        match val {
            // Validation chose the fee path: prepare it with its value. The pool is not asked
            // again.
            Path::Fee(val) => self.0.prepare(val, origin, call, info, len).map(Path::Fee),
            // Validation chose the pool: the same check must give the same ticket (`INV-2`).
            Path::Pool(ticket) => {
                match Pallet::<T>::check(&ticket.who, Self::estimate(info, len)) {
                    Ok(again) if again == ticket => Ok(Path::Pool(ticket)),
                    _ => Err(InvalidTransaction::Custom(PATH_MISMATCH).into()),
                }
            }
        }
    }

    fn post_dispatch_details(
        pre: Self::Pre,
        info: &DispatchInfoOf<T::RuntimeCall>,
        post_info: &PostDispatchInfoOf<T::RuntimeCall>,
        len: usize,
        result: &DispatchResult,
    ) -> Result<Weight, TransactionValidityError> {
        match pre {
            Path::Fee(pre) => S::post_dispatch_details(pre, info, post_info, len, result),
            Path::Pool(ticket) => {
                // A transaction that pays no fee is charged nothing (`REQ-PL-9`).
                if post_info.pays_fee(info) == Pays::Yes {
                    let actual = Self::actual(info, post_info, len);
                    // The ticket's contract and window, whatever dispatch changed (`INV-13`).
                    if let Some((weight, remaining)) = Pallet::<T>::charge(&ticket, actual) {
                        Pallet::<T>::deposit_event(Event::<T>::UsageCharged {
                            group: ticket.group,
                            who: ticket.who,
                            weight,
                            remaining,
                        });
                    }
                }

                // Only this extension's own unspent weight, and none is measured: the call's
                // weight stays booked for the block (`INV-11`).
                Ok(Weight::zero())
            }
        }
    }
}
