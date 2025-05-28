# FRAME Contrib

[![License](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](LICENSE)
![Polkadot](https://img.shields.io/badge/polkadot-E6007A?style=for-the-badge&logo=polkadot&logoColor=white)

> Built with ❤️ by the [Virto Team](https://virto.network)

A collection of production-ready pallets for FRAME compatible blockchains that extend the [PolkadotSDK](https://github.com/paritytech/polkadot-sdk). As true believers in the future potential of Polkadot and decentralized technologies, we're committed to building robust, secure, and scalable solutions that contribute to the broader Web3 ecosystem.

## 🌟 Our Vision

At Virto, we envision a future where decentralized technologies empower individuals and organizations through transparent, secure, and efficient systems. Our contribution to the Polkadot ecosystem represents our commitment to this vision, providing essential building blocks for the next generation of blockchain applications.

## 📚 Table of Contents

- [Overview](#overview)
- [Getting Started](#getting-started)
- [Available Pallets](#available-pallets)
  - [Pass Pallet](#pass-pallet)
  - [Communities Pallet](#communities-pallet)
  - [Gas Transaction Payment Pallet](#gas-transaction-payment-pallet)
  - [Listings Pallet](#listings-pallet)
  - [Orders Pallet](#orders-pallet)
  - [Payments Pallet](#payments-pallet)
  - [Referenda Tracks Pallet](#referenda-tracks-pallet)
- [Traits](#traits)
- [Contributing](#contributing)
- [About Virto](#about-virto)
- [License](#license)

## Overview

This repository contains a collection of Substrate pallets that can be integrated into any FRAME-compatible blockchain. These pallets provide additional functionality beyond the core Substrate framework, designed for production use cases. Each pallet has been carefully crafted with security, scalability, and usability in mind.

## Getting Started

To use these pallets in your Substrate runtime:

1. Add the dependency to your `Cargo.toml`:
```toml
[dependencies]
frame-contrib = { git = "https://github.com/virto-network/frame-contrib.git" }
```

2. Import the desired pallets in your runtime:
```rust
use frame_contrib::pallets::{
    pass,
    communities,
    gas_transaction_payment,
    // ... other pallets
};
```

## Available Pallets

### Pass Pallet

The `pass` pallet provides a robust authentication and account management system for Substrate-based blockchains. It implements a flexible authentication mechanism that supports multiple devices and session keys.

#### Key Features

- Multi-device authentication support
- Session key management
- Account registration with deposit system
- Device attestation and verification
- Flexible storage pricing

#### Configuration

```rust
impl pallet_pass::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
    type RegisterOrigin = EitherOf<
        EnsureRootWithSuccess<Self::AccountId, RootAccount>,
        EnsureSigned<Self::AccountId>,
    >;
    type AddressGenerator = ();
    type Balances = Balances;
    type Authenticator = PassAuthenticator;
    type Scheduler = Scheduler;
    type BlockNumberProvider = System;
    type RegistrarConsideration = RootDoesNotPayConsideration<
        HoldConsideration<AccountId, Balances, HoldAccountRegistration, RegistrationStoragePrice>,
    >;
    type DeviceConsideration = FirstItemIsFree<
        HoldConsideration<AccountId, Balances, HoldAccountDevices, ItemStoragePrice>,
    >;
    type SessionKeyConsideration = FirstItemIsFree<
        HoldConsideration<AccountId, Balances, HoldSessionKeys, ItemStoragePrice>
    >;
    type PalletId = PassPalletId;
    type MaxSessionDuration = ConstU64<10>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = benchmarks::BenchmarkHelper;
}
```

### Communities Pallet

The `communities` pallet provides functionality for managing communities within a blockchain network.

#### Key Features

- Community creation and management
- Member management
- Role-based access control
- Community governance

#### Configuration

```rust
impl pallet_communities::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    // ... additional configuration
}
```

### Gas Transaction Payment Pallet

The `gas-transaction-payment` pallet implements a flexible gas payment system for transactions.

#### Key Features

- Configurable gas pricing
- Support for multiple payment methods
- Gas tank functionality
- Transaction fee management

#### Configuration

```rust
impl pallet_gas_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    // ... additional configuration
}
```

### Listings Pallet

The `listings` pallet provides functionality for managing listings in a marketplace.

#### Key Features

- Listing creation and management
- Listing categories
- Listing status tracking
- Listing verification

#### Configuration

```rust
impl pallet_listings::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    // ... additional configuration
}
```

### Orders Pallet

The `orders` pallet implements order management functionality for marketplaces.

#### Key Features

- Order creation and tracking
- Order status management
- Order fulfillment
- Order dispute resolution

#### Configuration

```rust
impl pallet_orders::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    // ... additional configuration
}
```

### Payments Pallet

The `payments` pallet provides payment processing functionality.

#### Key Features

- Multiple payment methods
- Payment verification
- Refund processing
- Payment dispute handling

#### Configuration

```rust
impl pallet_payments::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    // ... additional configuration
}
```

### Referenda Tracks Pallet

The `referenda-tracks` pallet implements referendum tracking functionality.

#### Key Features

- Referendum creation and management
- Voting mechanisms
- Result tracking
- Execution scheduling

#### Configuration

```rust
impl pallet_referenda_tracks::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    // ... additional configuration
}
```

## Traits

The repository includes several trait definitions that can be used across different pallets:

### Authentication Traits (`traits/authn`)

- `Authenticator`: Core authentication functionality
- `Challenger`: Challenge-response authentication
- `DeviceAttestation`: Device verification

### Gas Tank Traits (`traits/gas-tank`)

- `GasTank`: Gas management functionality
- `GasProvider`: Gas provision interface

### Membership Traits (`traits/memberships`)

- `Membership`: Core membership functionality
- `RoleManager`: Role-based access control

### Payment Traits (`traits/payments`)

- `PaymentProcessor`: Payment processing interface
- `RefundHandler`: Refund management

### Listing Traits (`traits/listings`)

- `ListingManager`: Listing management interface
- `CategoryManager`: Category management

### Track Traits (`traits/tracks`)

- `TrackManager`: Track management interface
- `VoteManager`: Voting functionality

## Contributing

We welcome contributions from the community! As believers in open-source collaboration, we encourage developers to help us improve and expand these pallets. Please see our [Contributing Guidelines](CONTRIBUTING.md) for details.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## About Virto

Virto is dedicated to advancing the Polkadot ecosystem through innovative solutions and robust infrastructure. Our team combines deep technical expertise with a passion for decentralization, working to build the foundation for a more open and accessible Web3 future.

- [Website](https://virto.network)
- [Twitter](https://twitter.com/virtonetwork)
- [GitHub](https://github.com/virto-network)

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

The GPL-3.0 license requires that any derivative works must also be open source and licensed under the same terms. This means that if you modify or distribute this software, you must:
1. Make the source code available
2. License your modifications under GPL-3.0
3. Include a copy of the GPL-3.0 license
4. State significant changes made to the software

For more information about the GPL-3.0 license, visit [gnu.org/licenses/gpl-3.0](https://www.gnu.org/licenses/gpl-3.0).