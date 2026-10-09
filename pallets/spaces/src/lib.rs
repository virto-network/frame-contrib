#![cfg_attr(not(feature = "std"), no_std)]

//! # Spaces Pallet (`fc-pallet-spaces`)
//!
//! A **Space** is an anchored, verifiable state machine: a numbered identity, assigned on
//! registration, on which an off-chain network (a VOS network of Actors) is founded; a program,
//! named by the 32-byte commitment a proof verifier recognises; and a chain of state roots, each of
//! which the chain accepted only with a proof that the program moved the Space from the previous
//! root to it.
//!
//! - [`register`](Pallet::register) a Space with its authority, its program and its genesis
//!   root. The Space's id is the next one in sequence, and is emitted in
//!   [`Event::Registered`].
//! - [`anchor`](Pallet::anchor) a new root with a proof. Anyone may submit it: the proof is the
//!   authority, and it is bound to an [`AnchorStatement`] the pallet builds (this chain, the Space,
//!   its epoch, the anchor's number, the previous root and the new one). Anchors are numbered from
//!   one with no gaps, never removed and never overwritten; resubmitting a stored anchor is a
//!   no-op.
//! - [`set_program`](Pallet::set_program): the Space's authority changes its program from the next
//!   anchor on. Every past commitment is kept, with the anchors it applied to.
//! - [`refound`](Pallet::refound) the Space on a new genesis root, by its authority, **only while
//!   nothing is bound**. The anchor sequence continues where it stands.
//! - [`set_current_head`](Pallet::set_current_head), by a privileged origin, sets the Space's
//!   head to a given root, as `Paras::set_current_head` does for a parachain. Earlier anchors stay
//!   where they are; anchoring continues from the new root.
//! - [`set_authority`](Pallet::set_authority): the authority (or the privileged origin) hands the
//!   Space to another origin, as a community's admin origin is changed.
//! - **Binds**, made and released by other pallets through [`SpaceBinds`], record that something
//!   of value depends on the Space's anchored state. They are what stops a re-founding.
//!
//! Every re-founding or reset opens a new **epoch**, recorded with the root it starts from and the
//! anchor number it starts after, so a reader can always tell which anchors belong to which run
//! of the state machine.
//!
//! The proof system is behind [`Config::Verifier`]. This crate ships only a mock for tests and
//! benchmarks; real backends live with their proof systems. See `DESIGN.md`.

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod types;
pub mod verifier;
pub mod weights;

pub use pallet::*;
pub use types::*;
#[cfg(any(test, feature = "runtime-benchmarks"))]
pub use verifier::MockVerifier;
pub use verifier::{ProofVerifier, VerifyError};
pub use weights::*;

use alloc::boxed::Box;
use frame_support::{
    dispatch::{GetDispatchInfo, PostDispatchInfo},
    pallet_prelude::*,
    traits::{Incrementable, OriginTrait},
    PalletId,
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{AccountIdConversion, Dispatchable, Zero};

pub mod origin;
pub use origin::{EnsureSpace, RawOrigin};

/// The origin type a Space's authority is: any origin the runtime has.
pub type PalletsOriginOf<T> =
    <<T as frame_system::Config>::RuntimeOrigin as OriginTrait>::PalletsOrigin;

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
pub trait BenchmarkHelper {
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
    pub trait Config:
        frame_system::Config<
        RuntimeEvent: From<Event<Self>>,
        RuntimeOrigin: From<Origin<Self>>,
        RuntimeCall: From<Call<Self>>
                         + GetDispatchInfo
                         + Dispatchable<
            RuntimeOrigin = Self::RuntimeOrigin,
            PostInfo = PostDispatchInfo,
        >,
    >
    {
        /// Derives each Space's account, as a community's account is derived: the pallet id's
        /// sub-account for the Space id.
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// How Spaces are numbered. [`register`](Pallet::register) takes the next one.
        type SpaceId: Parameter + MaxEncodedLen + Copy + Incrementable;
        /// What other pallets bind to a Space (an asset id, for example).
        type BindKey: Parameter + MaxEncodedLen + Copy;
        /// Who may register a Space.
        type CreateOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Who may set a Space's head regardless of its binds (the analogue of the origin a relay
        /// chain's governance uses for `Paras::set_current_head`), switch the program of a Space
        /// that holds binds, and recover a Space's authority.
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
        type BenchmarkHelper: BenchmarkHelper;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// A Space speaking as itself: what [`dispatch_as_space`](Pallet::dispatch_as_space)
    /// dispatches with, and what [`EnsureSpace`] accepts.
    #[pallet::origin]
    pub type Origin<T> = RawOrigin<<T as Config>::SpaceId>;

    /// The id the next registered Space takes. Unset until the first registration, which takes
    /// the id type's initial value.
    #[pallet::storage]
    pub type NextSpaceId<T: Config> = StorageValue<_, T::SpaceId>;

    /// Every Space.
    #[pallet::storage]
    pub type Spaces<T: Config> =
        StorageMap<_, Blake2_128Concat, T::SpaceId, SpaceInfo<PalletsOriginOf<T>>>;

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

    /// Every program commitment each Space has run, by version. Never pruned.
    #[pallet::storage]
    pub type Programs<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::SpaceId,
        Twox64Concat,
        ProgramVersion,
        ProgramRecord<BlockNumberFor<T>>,
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
        /// A Space was registered with id `space`.
        Registered {
            space: T::SpaceId,
            authority: PalletsOriginOf<T>,
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
        /// A Space's program changed, from anchor `from_anchor` on.
        ProgramSet {
            space: T::SpaceId,
            version: ProgramVersion,
            program: ProgramId,
            from_anchor: AnchorNumber,
        },
        /// A Space's authority changed.
        AuthoritySet {
            space: T::SpaceId,
            authority: PalletsOriginOf<T>,
        },
        /// The authority re-founded a Space on a new genesis. Its next anchor is `base + 1`.
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
        /// A call was dispatched with a Space's origin.
        DispatchedAsSpace {
            space: T::SpaceId,
            result: DispatchResult,
        },
        /// A call was dispatched signed by a Space's account.
        DispatchedAsAccount {
            space: T::SpaceId,
            result: DispatchResult,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The Space id sequence is exhausted.
        NoSpaceId,
        /// No Space with this id.
        UnknownSpace,
        /// Only the Space's authority may do this.
        NotAuthority,
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
        /// Register a Space governed by `authority`, running `program`, starting at `genesis`.
        /// It takes the next Space id, emitted in [`Event::Registered`]. Opens epoch `0` and
        /// program version `0`; the first anchor is number `1`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register())]
        pub fn register(
            origin: OriginFor<T>,
            authority: PalletsOriginOf<T>,
            program: ProgramId,
            genesis: Root,
        ) -> DispatchResult {
            T::CreateOrigin::ensure_origin(origin)?;
            let space = NextSpaceId::<T>::get()
                .or_else(T::SpaceId::initial_value)
                .ok_or(Error::<T>::NoSpaceId)?;
            // An id with no successor is never handed out, so no id is ever handed out twice.
            match space.increment() {
                Some(next) => NextSpaceId::<T>::put(next),
                None => return Err(Error::<T>::NoSpaceId.into()),
            }

            let now = frame_system::Pallet::<T>::block_number();
            Spaces::<T>::insert(
                space,
                SpaceInfo {
                    authority: authority.clone(),
                    program,
                    program_version: 0,
                    binds: 0,
                },
            );
            Programs::<T>::insert(
                space,
                0,
                ProgramRecord {
                    program,
                    from_anchor: 1,
                    at: now,
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
            // The Space's account exists from registration on, as a community's does, so it can
            // receive funds of any amount.
            frame_system::Pallet::<T>::inc_providers(&Self::space_account(&space));
            Self::deposit_event(Event::Registered {
                space,
                authority,
                program,
                genesis,
            });
            Ok(())
        }

        /// Anchor `root` as anchor `number` of `space`, with a proof that the Space's current
        /// program moved it from its head's root to `root`.
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
                    program_version: info.program_version,
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
        /// and continues the Space's anchor sequence where it stands. Only its authority, and
        /// only while nothing is bound to it. Opens a new epoch.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::refound())]
        pub fn refound(origin: OriginFor<T>, space: T::SpaceId, genesis: Root) -> DispatchResult {
            let info = Self::ensure_authority(origin, space)?;
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

        /// Change the program `space` runs, from its next anchor on. The previous commitment stays
        /// in [`Programs`], with the anchors it applied to.
        ///
        /// While nothing is bound to the Space, its authority switches freely. Once something is
        /// bound, the program is the rules that value depends on, so only the reset origin can
        /// switch it.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::set_program())]
        pub fn set_program(
            origin: OriginFor<T>,
            space: T::SpaceId,
            program: ProgramId,
        ) -> DispatchResult {
            let mut info = match T::ResetOrigin::try_origin(origin) {
                Ok(_) => Spaces::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?,
                Err(origin) => {
                    let info = Self::ensure_authority(origin, space)?;
                    ensure!(info.binds == 0, Error::<T>::HasBinds);
                    info
                }
            };
            let head = Heads::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?;
            let version = info
                .program_version
                .checked_add(1)
                .ok_or(Error::<T>::Overflow)?;
            let from_anchor = head.number.saturating_add(1);
            Programs::<T>::insert(
                space,
                version,
                ProgramRecord {
                    program,
                    from_anchor,
                    at: frame_system::Pallet::<T>::block_number(),
                },
            );
            info.program = program;
            info.program_version = version;
            Spaces::<T>::insert(space, info);
            Self::deposit_event(Event::ProgramSet {
                space,
                version,
                program,
                from_anchor,
            });
            Ok(())
        }

        /// Hand `space` to `authority`. By its current authority, or by the reset origin (to
        /// recover a Space whose authority is lost).
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::set_authority())]
        pub fn set_authority(
            origin: OriginFor<T>,
            space: T::SpaceId,
            authority: PalletsOriginOf<T>,
        ) -> DispatchResult {
            let mut info = match T::ResetOrigin::try_origin(origin) {
                Ok(_) => Spaces::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?,
                Err(origin) => Self::ensure_authority(origin, space)?,
            };
            info.authority = authority.clone();
            Spaces::<T>::insert(space, info);
            Self::deposit_event(Event::AuthoritySet { space, authority });
            Ok(())
        }

        /// Dispatch `call` as `space` itself, with the Space's origin ([`RawOrigin`]), which
        /// [`EnsureSpace`] accepts. Only its authority.
        #[pallet::call_index(6)]
        #[pallet::weight({
            let di = call.get_dispatch_info();
            (T::WeightInfo::dispatch_as_space().saturating_add(di.call_weight), di.class)
        })]
        pub fn dispatch_as_space(
            origin: OriginFor<T>,
            space: T::SpaceId,
            call: Box<<T as frame_system::Config>::RuntimeCall>,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_authority(origin, space)?;
            let res = call.dispatch(RawOrigin::new(space).into());
            Self::deposit_event(Event::DispatchedAsSpace {
                space,
                result: res.map(|_| ()).map_err(|e| e.error),
            });
            Ok(().into())
        }

        /// Dispatch `call` signed by `space`'s account ([`Pallet::space_account`]), so the Space
        /// can hold and spend funds. Only its authority.
        #[pallet::call_index(7)]
        #[pallet::weight({
            let di = call.get_dispatch_info();
            (T::WeightInfo::dispatch_as_account().saturating_add(di.call_weight), di.class)
        })]
        pub fn dispatch_as_account(
            origin: OriginFor<T>,
            space: T::SpaceId,
            call: Box<<T as frame_system::Config>::RuntimeCall>,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_authority(origin, space)?;
            let signer = frame_system::RawOrigin::Signed(Self::space_account(&space));
            let res = call.dispatch(signer.into());
            Self::deposit_event(Event::DispatchedAsAccount {
                space,
                result: res.map(|_| ()).map_err(|e| e.error),
            });
            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The account of `space`: the pallet id's sub-account for the Space id, as a community's
    /// account is derived. Deterministic, and the same whether or not the Space exists yet.
    pub fn space_account(space: &T::SpaceId) -> T::AccountId {
        T::PalletId::get().into_sub_account_truncating(space)
    }

    /// The Space, if `origin` is its authority.
    fn ensure_authority(
        origin: OriginFor<T>,
        space: T::SpaceId,
    ) -> Result<SpaceInfo<PalletsOriginOf<T>>, DispatchError> {
        let info = Spaces::<T>::get(space).ok_or(Error::<T>::UnknownSpace)?;
        ensure!(*origin.caller() == info.authority, Error::<T>::NotAuthority);
        Ok(info)
    }

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
