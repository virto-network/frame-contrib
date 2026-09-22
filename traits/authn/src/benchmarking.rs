//! Helpers to produce valid authenticator inputs in benchmarks.
//!
//! Consumers of an [`Authenticator`] (e.g. `pallet-pass`) need valid device attestations and
//! credentials to benchmark their own logic, but only the authenticator knows how to produce
//! them (which keys to generate, how to sign, how to encode). These traits let each
//! authenticator provide them, so runtimes don't have to.
//!
//! There are two ways to implement them:
//!
//! - **Authenticators built with [`Auth`][crate::util::Auth] / [`Dev`][crate::util::Dev]**
//!   (the usual case) implement [`DeviceAttestationBenchmarkHelper`] for their attestation type
//!   and [`CredentialBenchmarkHelper`] for their credential type. The runtime implements
//!   [`ChallengerBenchmarkHelper`] for its [`Challenger`], and [`AuthenticatorBenchmarkHelper`]
//!   is then implemented for `Auth<Dev, Att>`: it picks the context, computes the challenge
//!   and gets the authority, and hands them to the attestation/credential helpers.
//! - **Hand-written authenticators** implement [`AuthenticatorBenchmarkHelper`] directly.
//!
//! [`composite_authenticator!`][crate::composite_authenticator] implements
//! [`AuthenticatorBenchmarkHelper`] for the composite by delegating to the **first** listed
//! authenticator, which must implement it.
//!
//! ## Keeping state
//!
//! An authenticator usually needs to remember the keys it generated for a device, so it can
//! sign credentials for it later, and a counter to hand out a fresh `device_id` on each call.
//! These helpers run inside the benchmark's externalities, so implementors can keep that state
//! in storage under a prefix of their own (e.g. `frame_support::storage::unhashed` or a
//! `#[frame_support::storage_alias]`). Consumers impose no storage layout.

use crate::{
    Authenticator, AuthorityId, Challenge, Challenger, DeviceChallengeResponse, DeviceId,
    ExtrinsicContext, HashedUserId, UserAuthenticator, UserChallengeResponse,
};

/// Produces valid inputs for an [`Authenticator`], for benchmarking its consumers.
pub trait AuthenticatorBenchmarkHelper: Authenticator {
    /// Returns a valid attestation of a **new** device, bound to the extrinsic context `xtc`,
    /// so that [`Authenticator::verify_device`] accepts it for that same `xtc`.
    ///
    /// Each call must return an attestation with a different `device_id`.
    fn device_attestation(xtc: &impl ExtrinsicContext) -> Self::DeviceAttestation;

    /// Returns a valid credential for the user `user_id`, from the device `device_id`
    /// (registered with an attestation previously returned by
    /// [`device_attestation`][Self::device_attestation]), bound to the extrinsic context
    /// `xtc`, so that [`UserAuthenticator::verify_user`] accepts it for that same `xtc`.
    fn credential(
        user_id: HashedUserId,
        device_id: DeviceId,
        xtc: &impl ExtrinsicContext,
    ) -> <Self::Device as UserAuthenticator>::Credential;
}

/// Provides a context that a [`Challenger`] accepts at the current point of a benchmark (e.g.
/// the current block number). Implemented by the runtime for its challenger.
pub trait ChallengerBenchmarkHelper: Challenger {
    fn benchmark_context() -> Self::Context;
}

/// Produces a valid device attestation. Implemented by an authenticator's attestation type.
pub trait DeviceAttestationBenchmarkHelper<Cx>: DeviceChallengeResponse<Cx> {
    /// Returns a valid attestation of a **new** device for the given `authority`, which
    /// answers `challenge`, generated with `context`.
    ///
    /// Each call must return an attestation with a different `device_id`.
    fn benchmark_attestation(authority: AuthorityId, context: Cx, challenge: Challenge) -> Self;
}

/// Produces a valid credential. Implemented by an authenticator's credential type.
pub trait CredentialBenchmarkHelper<Cx>: UserChallengeResponse<Cx> {
    /// Returns a valid credential of the user `user_id` for the given `authority`, from the
    /// device `device_id` (previously returned by
    /// [`DeviceAttestationBenchmarkHelper::benchmark_attestation`]), which answers
    /// `challenge`, generated with `context`.
    fn benchmark_credential(
        authority: AuthorityId,
        user_id: HashedUserId,
        device_id: DeviceId,
        context: Cx,
        challenge: Challenge,
    ) -> Self;
}

/// Conditionally expands its input when `fc-traits-authn` is built with `runtime-benchmarks`.
/// Used by [`composite_authenticator!`][crate::composite_authenticator].
#[doc(hidden)]
#[macro_export]
macro_rules! __if_runtime_benchmarks {
    ($($t:tt)*) => { $($t)* };
}
