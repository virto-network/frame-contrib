# Content Pallet

The **Content** pallet enables permanent logging of content events via the blockchain. 

## ✨ Overview

Substrate chains with the Content pallet can be the basis for decentralized versions of public content platforms, such as:

- **Feeds** - X, YouTube, RSS, etc
- **Topics** - Reddit, Forums, etc
- **Wikis** - Wikipedia
- **Software Repo** - NPM, crates.io, APT, RPM, Homebrew, Docker Hub, Google Play
- **E-commerce** - amazon.com

## 🔧 Features

- **Hierarchical** - content items can declare their parent items at creation time. This cannot be changed. For example the item could be a reply to another item.
- **Linked** - each content revision has a set of links to other items. These can be referenced within the content.
- **Revisionable** - content items can be updated by the item owner.
- **Retractable** - content items can be retracted by the item owner. While nothing can be guaranteed to be deleted, this flag indicates that the item should be removed.
- **Efficient** - Transactions are very cheap; only a bare minimum of information is stored in state. IPFS hashes are emitted in events. An indexer such as [Acuity Index](https://index.acuity.network/) can be used to construct the content graph.
- **Timestamped** - each content revision is timestamped by the blockchain.
- **Permanent** - there is a provable record of everything that has been published.
- **Semantic First** - content is encoded with Protocol Buffers and does not have to be scraped.
- **Separation of Concerns** - anyone can publish, anyone can make a backend / frontend

## ⚙️ Configuration

This pallet requires the following types and parameters to be configured in the runtime:

| Type / Constant              | Description                                                 |
|------------------------------|-------------------------------------------------------------|
| `RuntimeEvent`               | Event type for this pallet                                  |
| `WeightInfo`                 | Weight cost functions                                       |

## 🧠 Storage

| Item            | Description                                                   |
|-----------------|---------------------------------------------------------------|
| `ItemState`     | `Item` data structure containing owner, revision_id and flags. |

Each content item has a unique item_id that is produced by hashing together the AccountId with a provided nonce. The nonce is decided by the publishing software. For example it could be random, incremental or BIP32 derived.

## 📦 Dispatchable Functions

### `publish_item`

Publish a new content item.

```rust
fn publish_item(
    origin: OriginFor<T>,
    nonce: Nonce,
    parents: Vec<ItemId>,
    flags: u8,
    links: Vec<ItemId>,
    ipfs_hash: IpfsHash,
)
```

### `publish_revision`

Update an existing content item.

```rust
fn publish_revision(
    origin: OriginFor<T>,
    item_id: ItemId,
    links: Vec<ItemId>,
    ipfs_hash: IpfsHash,
)
```

### `retract_item`

Retract a content item.

```rust
fn retract_item(origin: OriginFor<T>, item_id: ItemId)
```

### `set_not_revisionable`

Mark a content item as not revisionable.

```rust
fn set_not_revisionable(origin: OriginFor<T>, item_id: ItemId)
```

### `set_not_retractable`

Mark a content item as not retractable.

```rust
fn set_not_retractable(origin: OriginFor<T>, item_id: ItemId)
```

## 🧪 Testing and Benchmarking

- Includes unit tests and benchmarking support behind `runtime-benchmarks`.


## Project History

This pallet was originally a Solidity smart contract started in 2016 as part of MIX Blockchain (EVM). MIX is now called Acuity and is a Substrate chain. The Solidity smart contract can be seen [here](https://github.com/acuity-network/acuity-contracts/blob/master/src/acuity-item-store/AcuityItemStoreIpfsSha256.sol).
