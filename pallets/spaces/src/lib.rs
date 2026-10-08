#![cfg_attr(not(feature = "std"), no_std)]

//! # Spaces Pallet (`fc-pallet-spaces`)
//!
//! A **Space** is an anchored, verifiable state machine: a program, identified by the 32-byte
//! commitment a proof verifier recognises, and a chain of state roots, each of which the chain
//! accepted only with a proof that the program moved the Space from the previous root to it.
//!
//! - [`register`](Pallet::register) a Space with its program and its genesis root. The
//!   registering origin's account becomes its owner.
//! - [`anchor`](Pallet::anchor) a new root with a proof. Anyone may submit it: the proof is the
//!   authority, and it is bound to an [`AnchorStatement`] the pallet builds (this chain, the Space,
//!   its epoch, the anchor's number, the previous root and the new one). Anchors are numbered from
//!   one with no gaps, never removed and never overwritten; resubmitting a stored anchor is a
//!   no-op.
//! - [`refound`](Pallet::refound) the Space on a new genesis root, by its owner, **only while
//!   nothing is bound**. The anchor sequence continues where it stands.
//! - [`set_current_head`](Pallet::set_current_head), by a privileged origin, sets the Space's
//!   head to a given root, as `Paras::set_current_head` does for a parachain. Earlier anchors stay
//!   where they are; anchoring continues from the new root.
//! - **Binds**, made and released by other pallets through [`SpaceBinds`], record that something
//!   of value depends on the Space's anchored state. They are what stops an owner from re-founding.
//!
//! Every re-founding or reset opens a new **epoch**, recorded with the root it starts from and the
//! anchor number it starts after, so a reader can always tell which anchors belong to which run
//! of the state machine.
//!
//! The proof system is behind [`Config::Verifier`]: tests use [`MockVerifier`], and VOS's STARK
//! verifier (Circle STARK over M31, built on Stwo) is `vos::VosVerifier` behind the
//! `vos-verifier` feature. See `DESIGN.md`.

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod types;
pub mod verifier;
#[cfg(feature = "vos-verifier")]
pub mod vos;
pub mod weights;

pub use pallet::*;
pub use types::*;
#[cfg(any(test, feature = "runtime-benchmarks"))]
pub use verifier::MockVerifier;
pub use verifier::{vos_io_hash, ProofVerifier, VerifyError};
pub use weights::*;

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_runtime::traits::Zero;

/// What other pallets use to bind value to a Space's anchored state.
pub trait SpaceBinds<SpaceId, BindKey> {
    /// Record that `key` now depends on `space`'s anchored state. Returns the anchor number it
    /// was made after.
    fn bind(space: &SpaceId, key: &BindKey) -> Result<AnchorNumber, DispatchError>;
    /// Release `key`.
    fn unbind(space: &SpaceId, key: &BindKey) -> DispatchResult;
    /// Whether `key` is bound to `space`.
    fn is_bound(space: &SpaceId, key: &BindKey) -> bool;
}

/// What a runtime's benchmarks need to drive the pallet.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<SpaceId> {
    /// A Space id, distinct for each `i`.
    fn space(i: u32) -> SpaceId;
    /// The program the benchmarked Space runs.
    fn program() -> ProgramId;
    /// A proof the runtime's [`Config::Verifier`] accepts for `program`, `public` and `output`,
    /// as large as the worst proof the runtime admits.
    ///
    /// TODO: for a real STARK backend, this needs a program that returns the pallet's
    /// [`AnchorStatement`]; a precomputed proof cannot match a statement that names the chain.
    fn proof(program: &ProgramId, public: &[u8], output: &[u8]) -> alloc::vec::Vec<u8>;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// How Spaces are named.
        type SpaceId: Parameter + MaxEncodedLen + Copy;
        /// What other pallets bind to a Space (an asset id, for example).
        type BindKey: Parameter + MaxEncodedLen + Copy;
        /// Who may register a Space. Its success is the Space's owner.
        type RegisterOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;
        /// Who may set a Space's head regardless of its binds: the analogue of the origin a relay
        /// chain's governance uses for `Paras::set_current_head`.
        type ResetOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// The proof system anchors are checked with.
        type Verifier: ProofVerifier;
        /// The longest proof [`anchor`](Pallet::anchor) accepts, in bytes.
        #[pallet::constant]
        type MaxProofLen: Get<u32>;
        /// The longest public input [`anchor`](Pallet::anchor) accepts, in bytes.
        #[pallet::constant]
        type MaxPublicLen: Get<u32>;
        /// Weights of the pallet's own work. The verifier's is [`ProofVerifier::weight`].
        type WeightInfo: WeightInfo;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self::SpaceId>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every Space.
    #[pallet::storage]
    pub type Spaces<T: Config> =
        StorageMap<_, Blake2_128Concat, T::SpaceId, SpaceInfo<T::AccountId>>;

    /// Where each Space's anchoring stands.
    #[pallet::storage]
    pub type Heads<T: Config> =
        StorageMap<_, Blake2_128Concat, T::SpaceId, Head<BlockNumberFor<T>>>;

    /// Every anchor, by Space and number. Never pruned.
    #[pallet::storage]
    pub type Anchors<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::SpaceId,
        Twox64Concat,
        AnchorNumber,
        AnchorRecord<BlockNumberFor<T>>,
    >;

    /// How each epoch of each Space began.
    #[pallet::storage]
    pub type Epochs<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::SpaceId,
        Twox64Concat,
        Epoch,
        EpochInfo<BlockNumberFor<T>>,
    >;

    /// The binds each Space holds.
    #[pallet::storage]
    pub type Binds<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::SpaceId,
        Blake2_128Concat,
        T::BindKey,
        BindRecord<BlockNumberFor<T>>,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A Space was registered.
        Registered {
            space: T::SpaceId,
            owner: T::AccountId,
            program: ProgramId,
            genesis: Root,
        },
        /// A proof moved a Space to `root`.
        Anchored {
            space: T::SpaceId,
            epoch: Epoch,
            number: AnchorNumber,
            root: Root,
            by: T::AccountId,
        },
        /// The owner re-founded a Space on a new genesis. Its next anchor is `base + 1`.
        Refounded {
            space: T::SpaceId,
            epoch: Epoch,
            base: AnchorNumber,
            genesis: Root,
        },
        /// The reset origin set a Space's head. Its next anchor is `base + 1`; `binds` binds
        /// carried over.
        HeadSet {
            space: T::SpaceId,
            epoch: Epoch,
            base: AnchorNumber,
            previous: Root,
            root: Root,
            binds: u32,
        },
        /// Something was bound to a Space's anchored state.
        Bound {
            space: T::SpaceId,
            key: T::BindKey,
            epoch: Epoch,
            after_anchor: AnchorNumber,
        },
        /// A bind was released.
        Unbound { space: T::SpaceId, key: T::BindKey },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// A Space with this id exists.
        SpaceExists,
        /// No Space with this id.
        UnknownSpace,
        /// Only the Space's owner may do this.
        NotOwner,
        /// The anchor's number is not the head's plus one.
        AnchorOutOfOrder,
        /// This number is anchored, to another root.
        AnchorConflict,
        /// The verifier could not decode the proof.
        MalformedProof,
        /// The proof does not bind this Space's head, this anchor and this public input.
        NotBound,
        /// The proof is of another program.
        WrongProgram,
        /// The proof does not verify.
        InvalidProof,
        /// The Space holds binds: only the reset origin can move its head off its anchors.
        HasBinds,
        /// This key is bound to the Space already.
        AlreadyBound,
        /// This key is not bound to the Space.
        UnknownBind,
        /// A counter would overflow.
        Overflow,
    }

    impl<T> From<VerifyError> for Error<T> {
        fn from(e: VerifyError) -> Self {
            match e {
                VerifyError::Malformed => Error::MalformedProof,
                VerifyError::NotBound => Error::NotBound,
                VerifyError::WrongProgram => Error::WrongProgram,
                VerifyError::Invalid => Error::InvalidProof,
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register `space`, running `program`, starting at `genesis`. The origin's account is its
        /// owner. Opens epoch `0`; the first anchor is number `1`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register())]
        pub fn register(
            origin: OriginFor<T>,
            space: T::SpaceId,
            program: ProgramId,
            genesis: Root,
        ) -> DispatchResult {
            let owner = T::RegisterOrigin::ensure_origin(origin)?;
            ensure!(!Spaces::<T>::contains_key(space), Error::<T>::SpaceExists);
            let now = frame_system::Pallet::<T>::block_number();
            Spaces::<T>::insert(
                space,
                SpaceInfo {
                    owner: owner.clone(),
                    program,
                    binds: 0,
                },
            );
            Heads::<T>::insert(
                space,
                Head {
                    epoch: 0,
                    number: 0,
                    root: genesis,
                    at: now,
                },
            );
            Epochs::<T>::insert(
                space,
                0,
                EpochInfo {
                    root: genesis,
                    base: 0,
                    at: now,
                    cause: EpochCause::Registered,
                },
            );
            Self::deposit_event(Event::Registered {
                space,
                owner,
                program,
                genesis,
            });
            Ok(())
        }

        /// Anchor `root` as anchor `number` of `space`, with a proof that the Space's program
        /// moved it from its head's root to `root`.
        ///
        /// The proof must bind `public` (opaque to the chain) and, as the program's output, the
        /// encoded [`AnchorStatement`] for this chain, Space, epoch, number and roots.
        /// Resubmitting an anchor already stored with the same root succeeds and changes nothing.
        #[pallet::call_index(1)]
        #[pallet::weight(
            T::WeightInfo::anchor(public.len() as u32)
                .saturating_add(T::Verifier::weight(proof.len() as u32))
        )]
        pub fn anchor(
            origin: OriginFor<T>,
            space: T::SpaceId,
            number: AnchorNumber,
            root: Root,
            proof: BoundedVec<u8, T::MaxProofLen>,
            public: BoundedVec<u8, T::MaxPublicLen>,
        ) -> DispatchResultWithPostInfo {
            let by = ensure_signed(origin)?;
            let info = Spaces::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?;
            let head = Heads::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?;

            if let Some(stored) = Anchors::<T>::get(space, number) {
                ensure!(stored.root == root, Error::<T>::AnchorConflict);
                return Ok(Some(T::WeightInfo::anchor_replay()).into());
            }
            ensure!(
                head.number.checked_add(1) == Some(number),
                Error::<T>::AnchorOutOfOrder
            );

            let statement = Self::anchor_statement(space, &head, root);
            T::Verifier::verify(&info.program, &proof, &public, &statement.encode())
                .map_err(Error::<T>::from)?;

            let now = frame_system::Pallet::<T>::block_number();
            Anchors::<T>::insert(
                space,
                number,
                AnchorRecord {
                    root,
                    epoch: head.epoch,
                    at: now,
                },
            );
            Heads::<T>::insert(
                space,
                Head {
                    number,
                    root,
                    at: now,
                    ..head
                },
            );
            Self::deposit_event(Event::Anchored {
                space,
                epoch: head.epoch,
                number,
                root,
                by,
            });
            Ok(().into())
        }

        /// Re-found `space` on `genesis`: a new run of its program, which starts from `genesis`
        /// and continues the Space's anchor sequence where it stands. Only its owner, and only
        /// while nothing is bound to it. Opens a new epoch.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::refound())]
        pub fn refound(origin: OriginFor<T>, space: T::SpaceId, genesis: Root) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let info = Spaces::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?;
            ensure!(info.owner == who, Error::<T>::NotOwner);
            ensure!(info.binds == 0, Error::<T>::HasBinds);
            let (epoch, base) = Self::open_epoch(space, genesis, EpochCause::Refounded)?;
            Self::deposit_event(Event::Refounded {
                space,
                epoch,
                base,
                genesis,
            });
            Ok(())
        }

        /// Set `space`'s head to `root`, whatever it holds: the analogue of
        /// `Paras::set_current_head`. Earlier anchors stay; the next anchor continues the
        /// sequence from `root`, in a new epoch. Binds carry over: the origin answers for `root`
        /// accounting for them.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_current_head())]
        pub fn set_current_head(
            origin: OriginFor<T>,
            space: T::SpaceId,
            root: Root,
        ) -> DispatchResult {
            T::ResetOrigin::ensure_origin(origin)?;
            let info = Spaces::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?;
            let previous = Heads::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?.root;
            let (epoch, base) = Self::open_epoch(space, root, EpochCause::Reset)?;
            Self::deposit_event(Event::HeadSet {
                space,
                epoch,
                base,
                previous,
                root,
                binds: info.binds,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The statement an anchor of `space` to `root` must prove, given its current `head`.
    pub fn anchor_statement(
        space: T::SpaceId,
        head: &Head<BlockNumberFor<T>>,
        root: Root,
    ) -> AnchorStatement<T::SpaceId> {
        let genesis = frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::zero());
        let mut chain = [0u8; 32];
        let bytes = genesis.as_ref();
        let n = bytes.len().min(32);
        chain[..n].copy_from_slice(&bytes[..n]);
        AnchorStatement {
            domain: ANCHOR_DOMAIN,
            version: ANCHOR_STATEMENT_VERSION,
            chain,
            space,
            epoch: head.epoch,
            number: head.number.saturating_add(1),
            prev_root: head.root,
            root,
        }
    }

    /// Move `space`'s head to `root` in a new epoch, keeping its anchor number.
    fn open_epoch(
        space: T::SpaceId,
        root: Root,
        cause: EpochCause,
    ) -> Result<(Epoch, AnchorNumber), DispatchError> {
        let head = Heads::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?;
        let epoch = head.epoch.checked_add(1).ok_or(Error::<T>::Overflow)?;
        let now = frame_system::Pallet::<T>::block_number();
        Epochs::<T>::insert(
            space,
            epoch,
            EpochInfo {
                root,
                base: head.number,
                at: now,
                cause,
            },
        );
        Heads::<T>::insert(
            space,
            Head {
                epoch,
                number: head.number,
                root,
                at: now,
            },
        );
        Ok((epoch, head.number))
    }
}

impl<T: Config> SpaceBinds<T::SpaceId, T::BindKey> for Pallet<T> {
    fn bind(space: &T::SpaceId, key: &T::BindKey) -> Result<AnchorNumber, DispatchError> {
        let head = Heads::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?;
        ensure!(
            !Binds::<T>::contains_key(space, key),
            Error::<T>::AlreadyBound
        );
        Spaces::<T>::try_mutate(space, |info| -> DispatchResult {
            let info = info.as_mut().ok_or(Error::<T>::UnknownSpace)?;
            info.binds = info.binds.checked_add(1).ok_or(Error::<T>::Overflow)?;
            Ok(())
        })?;
        Binds::<T>::insert(
            space,
            key,
            BindRecord {
                epoch: head.epoch,
                after_anchor: head.number,
                at: frame_system::Pallet::<T>::block_number(),
            },
        );
        Self::deposit_event(Event::Bound {
            space: *space,
            key: *key,
            epoch: head.epoch,
            after_anchor: head.number,
        });
        Ok(head.number)
    }

    fn unbind(space: &T::SpaceId, key: &T::BindKey) -> DispatchResult {
        ensure!(
            Binds::<T>::contains_key(space, key),
            Error::<T>::UnknownBind
        );
        Spaces::<T>::try_mutate(space, |info| -> DispatchResult {
            let info = info.as_mut().ok_or(Error::<T>::UnknownSpace)?;
            info.binds = info.binds.saturating_sub(1);
            Ok(())
        })?;
        Binds::<T>::remove(space, key);
        Self::deposit_event(Event::Unbound {
            space: *space,
            key: *key,
        });
        Ok(())
    }

    fn is_bound(space: &T::SpaceId, key: &T::BindKey) -> bool {
        Binds::<T>::contains_key(space, key)
    }
}
