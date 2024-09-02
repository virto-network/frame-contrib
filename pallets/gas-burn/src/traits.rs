use frame_support::Parameter;

/// Handles
pub trait GasBurner {
    type Gas: Parameter;
    type AccountId: Parameter;

    /// Check if account has a minimum of `gas` to consume.
    /// Returns the gas that would be left after burning the requested amount or `None` if there's not enough left.  
    /// When `gas` is not provided it simply returns the available gas.
    fn check_available_gas(who: &Self::AccountId, gas: &Option<Self::Gas>) -> Option<Self::Gas>;

    /// Spend as much `gas` as possible returning what is left in the tank.
    ///
    /// This method is expected not to fail.
    fn burn_gas(who: &Self::AccountId, gas: &Self::Gas) -> Self::Gas;
}
