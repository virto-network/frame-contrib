#![cfg_attr(not(feature = "std"), no_std)]

//! # Authentication Traits
//!
//! A collection of traits that define a flexible and secure authentication system for Substrate-based blockchains.
//! This crate provides the building blocks for implementing various authentication mechanisms, including
//! device-based authentication and user verification.
//!
//! ## Overview
//!
//! The authentication system is built around several key concepts:
//! - **Challenges**: Cryptographic challenges used to verify authentication attempts
//! - **Devices**: Authentication devices that can verify user credentials
//! - **Credentials**: User-provided authentication data
//! - **Attestations**: Device verification data
//!
//! ## Key Components
//!
//! ### Core Traits
//!
//! - `Authenticator`: Verifies authentication devices
//! - `UserAuthenticator`: Verifies user credentials
//! - `Challenger`: Generates and verifies challenges
//! - `DeviceChallengeResponse`: Handles device attestation responses
//! - `UserChallengeResponse`: Handles user credential responses
//!
//! ### Security Features
//!
//! - Challenge-response authentication
//! - Device attestation verification
//! - Authority-based validation
//! - Extrinsic context validation
//!
//! ## Usage
//!
//! ```rust
//! use fc_traits_authn::{
//!     Authenticator, UserAuthenticator, Challenger,
//!     DeviceChallengeResponse, UserChallengeResponse,
//! };
//!
//! // Implement the traits for your authentication system
//! struct MyAuthenticator;
//! impl Authenticator for MyAuthenticator {
//!     // ... implementation ...
//! }
//! ```
//!
//! ## Architecture
//!
//! The authentication system follows a layered approach:
//! 1. Device verification through attestations
//! 2. User verification through credentials
//! 3. Challenge-response validation
//! 4. Authority verification
//!
//! ## Security Considerations
//!
//! - All challenges are cryptographically secure
//! - Device attestations must be verified
//! - User credentials are validated against device capabilities
//! - Authority verification is required at multiple levels
//!
//! ## License
//!
//! Licensed under the GPL version 3.0

use codec::{FullCodec, MaxEncodedLen};
use frame_support::{traits::Get, Parameter};
use scale_info::TypeInfo;

pub mod util;

pub use fc_traits_authn_proc::composite_authenticator;

const LOG_TARGET: &str = "authn";

/// Common imports for composite authenticators.
///
/// This module provides a prelude of commonly used types and traits for implementing
/// composite authenticators.
pub mod composite_prelude {
    pub use crate::{
        Authenticator, AuthorityId, Challenge, Challenger, DeviceChallengeResponse, DeviceId,
        ExtrinsicContext, HashedUserId, UserAuthenticator, UserChallengeResponse,
    };
    pub use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
    pub use frame_support::{pallet_prelude::TypeInfo, traits::Get, DebugNoBound, EqNoBound};
}

/// Macro for creating composite authenticators.
///
/// This macro simplifies the creation of composite authenticators by combining multiple
/// authentication mechanisms.
///
/// # Example
///
/// ```rust
/// composite_authenticators! {
///     pub CompositePassA<AuthorityA> { AuthA, AuthB };
/// }
/// ```
#[macro_export]
macro_rules! composite_authenticators {
    // Match a single composite authenticator with the format:
    // pub CompositePassA<AuthorityA> { AuthA, AuthB };
    ($(
        pub $name:path {
            $($auth:path),* $(,)?
        };
    )*) => {
        $(
            $crate::composite_authenticator!(
                pub $name {
                    $($auth),*
                }
            );
        )*
    }
}

// A reasonably sized secure challenge
const CHALLENGE_SIZE: usize = 32;
pub type Challenge = [u8; CHALLENGE_SIZE];
type CxOf<C> = <C as Challenger>::Context;

/// A unique identifier for an authentication device.
pub type DeviceId = [u8; 32];

/// A unique identifier for an authentication authority.
pub type AuthorityId = [u8; 32];

/// The length of a hashed user ID in bytes.
pub const HASHED_USER_ID_LEN: usize = 32;

/// A hashed user identifier.
pub type HashedUserId = [u8; HASHED_USER_ID_LEN];

/// Context information provided during extrinsic verification.
///
/// This trait represents additional context that can be provided during the verification
/// of authentication attempts. The context is opaque to the challenger implementation
/// and is handled as a byte slice.
pub trait ExtrinsicContext: AsRef<[u8]> + core::fmt::Debug {}
impl<T> ExtrinsicContext for T where T: AsRef<[u8]> + core::fmt::Debug {}

/// Generates and verifies authentication challenges.
///
/// This trait provides functionality for generating cryptographic challenges and
/// verifying challenge responses. It is a core component of the challenge-response
/// authentication mechanism.
pub trait Challenger {
    /// The type of context used for challenge generation and verification.
    type Context: Parameter;

    /// Generates a new challenge based on the provided context and extrinsic context.
    fn generate(cx: &Self::Context, xtc: &impl ExtrinsicContext) -> Challenge;

    /// Verifies that a challenge response matches the expected challenge.
    ///
    /// # Arguments
    ///
    /// * `cx` - The context used to generate the challenge
    /// * `xtc` - The extrinsic context
    /// * `challenge` - The challenge response to verify
    ///
    /// # Returns
    ///
    /// * `Some(())` if the challenge is valid
    /// * `None` if the challenge is invalid
    fn check_challenge(
        cx: &Self::Context,
        xtc: &impl ExtrinsicContext,
        challenge: &[u8],
    ) -> Option<()> {
        Self::generate(cx, xtc).eq(challenge).then_some(())
    }
}

/// Verifies authentication devices and their attestations.
///
/// This trait provides functionality for verifying the authenticity of devices
/// through attestations. It is the first layer of the authentication system.
pub trait Authenticator {
    /// The type representing the authority that can verify devices.
    type Authority: Get<AuthorityId>;
    /// The type of challenger used for device verification.
    type Challenger: Challenger;
    /// The type of device attestation.
    type DeviceAttestation: DeviceChallengeResponse<CxOf<Self::Challenger>>;
    /// The type of device that can be verified.
    type Device: UserAuthenticator<Challenger = Self::Challenger>;

    /// Verifies a device attestation and returns the verified device if successful.
    ///
    /// # Arguments
    ///
    /// * `attestation` - The device attestation to verify
    /// * `xtc` - The extrinsic context
    ///
    /// # Returns
    ///
    /// * `Some(device)` if the attestation is valid
    /// * `None` if the attestation is invalid
    fn verify_device(
        attestation: Self::DeviceAttestation,
        xtc: &impl ExtrinsicContext,
    ) -> Option<Self::Device> {
        log::trace!(target: LOG_TARGET, "Verifying device with attestation: {:?}", attestation);

        log::trace!(target: LOG_TARGET, "Assert authority {:?}", attestation.authority());
        attestation
            .authority()
            .eq(&Self::Authority::get())
            .then_some(())?;
        log::trace!(target: LOG_TARGET, "Authority verified");

        let (cx, challenge) = &attestation.used_challenge();
        log::trace!(target: LOG_TARGET, "Check challenge {:?} (with cx={cx:?}, xtc={xtc:?})", challenge);
        Self::Challenger::check_challenge(cx, xtc, challenge)?;
        log::trace!(target: LOG_TARGET, "Challenge checked");

        log::trace!(target: LOG_TARGET, "Validate attestation");
        attestation.is_valid().then_some(())?;

        log::trace!(target: LOG_TARGET, "Retrieve device");
        Some(Self::unpack_device(attestation))
    }

    /// Extracts device information from an attestation.
    ///
    /// # Arguments
    ///
    /// * `attestation` - The device attestation to unpack
    ///
    /// # Returns
    ///
    /// The device information extracted from the attestation
    fn unpack_device(attestation: Self::DeviceAttestation) -> Self::Device;
}

/// A device capable of verifying user credentials.
///
/// This trait represents an authentication device that can verify user-provided
/// credentials. It is the second layer of the authentication system.
pub trait UserAuthenticator: FullCodec + MaxEncodedLen + TypeInfo {
    /// The type representing the authority that can verify users.
    type Authority: Get<AuthorityId>;
    /// The type of challenger used for user verification.
    type Challenger: Challenger;
    /// The type of credential that can be verified.
    type Credential: UserChallengeResponse<CxOf<Self::Challenger>> + Send + Sync;

    /// Verifies a user credential and returns success if valid.
    ///
    /// # Arguments
    ///
    /// * `credential` - The user credential to verify
    /// * `xtc` - The extrinsic context
    ///
    /// # Returns
    ///
    /// * `Some(())` if the credential is valid
    /// * `None` if the credential is invalid
    fn verify_user(
        &self,
        credential: &Self::Credential,
        xtc: &impl ExtrinsicContext,
    ) -> Option<()> {
        log::trace!(target: LOG_TARGET, "Verifying user for credential: {:?}", credential);

        log::trace!(target: LOG_TARGET, "Assert authority {:?}", credential.authority());
        credential
            .authority()
            .eq(&Self::Authority::get())
            .then_some(())?;
        log::trace!(target: LOG_TARGET, "Authority verified");

        let (cx, challenge) = &credential.used_challenge();
        log::trace!(target: LOG_TARGET, "Check challenge {:?}", challenge);
        Self::Challenger::check_challenge(cx, xtc, challenge)?;
        log::trace!(target: LOG_TARGET, "Challenge checked");

        log::trace!(target: LOG_TARGET, "Credential verified");
        credential.is_valid().then_some(())?;

        log::trace!(target: LOG_TARGET, "Verify credential");
        self.verify_credential(credential)
    }

    /// Verifies a credential against the device's capabilities.
    ///
    /// # Arguments
    ///
    /// * `credential` - The credential to verify
    ///
    /// # Returns
    ///
    /// * `Some(())` if the credential is valid
    /// * `None` if the credential is invalid
    fn verify_credential(&self, credential: &Self::Credential) -> Option<()>;

    /// Returns the device's unique identifier.
    fn device_id(&self) -> &DeviceId;
}

/// A response to a challenge for creating a new authentication device.
///
/// This trait represents the response to a challenge when creating a new
/// authentication device. It includes the device's attestation and verification data.
pub trait DeviceChallengeResponse<Cx>: Parameter {
    /// Verifies if the response is valid.
    fn is_valid(&self) -> bool;

    /// Returns the challenge and context used in the response.
    fn used_challenge(&self) -> (Cx, Challenge);

    /// Returns the authority that verified the device.
    fn authority(&self) -> AuthorityId;

    /// Returns the device's unique identifier.
    fn device_id(&self) -> &DeviceId;
}

/// A response to a challenge for identifying a user.
///
/// This trait represents the response to a challenge when authenticating a user.
/// It includes the user's credential and verification data.
pub trait UserChallengeResponse<Cx>: Parameter {
    /// Verifies if the response is valid.
    fn is_valid(&self) -> bool;

    /// Returns the challenge and context used in the response.
    fn used_challenge(&self) -> (Cx, Challenge);

    /// Returns the authority that verified the user.
    fn authority(&self) -> AuthorityId;

    /// Returns the user's hashed identifier.
    fn user_id(&self) -> HashedUserId;
}
