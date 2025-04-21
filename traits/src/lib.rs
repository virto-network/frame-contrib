#![cfg_attr(not(feature = "std"), no_std)]

// Re-export `composite_prelude` to prevent accidents when using the `composite_authenticator!` macro.
pub use authn::composite_prelude;
pub use fc_traits_authn as authn;

pub use fc_traits_gas_tank as gas_tank;
pub use fc_traits_listings as listings;
pub use fc_traits_memberships as memberships;
pub use fc_traits_payments as payments;
pub use fc_traits_tracks as tracks;
