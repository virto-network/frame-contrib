use frame_support::{weights::Weight, Parameter};

/// Handles
pub trait PrepaidGasHandler {
    type AccountId: Parameter;

    /// Attempts an initial charge of the prepaid gas for a given `who`.
    ///
    /// Fails if the handler cannot make a charge on `who`, or it doesn't have enough prepaid gas.
    fn initiate_payment(who: &Self::AccountId, weight: &Weight) -> Result<(), ()>;

    /// Completes the charge of the prepaid gas for a given `who`.
    ///
    /// This method is not expected to fail.
    fn complete_payment(
        who: &Self::AccountId,
        initial_weight: &Weight,
        actual_weight: &Weight,
        pays_fees: bool,
    );
}
