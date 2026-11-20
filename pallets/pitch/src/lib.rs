#![cfg_attr(not(feature = "std"), no_std)]

//! # Pitch Pallet
//!
//! Time-boxed economic authority over H3 cells, private by default.
//!
//! The pallet stores sparse pitch records keyed by raw H3 cell ids or opaque
//! commitments. H3 geometry remains off-chain; the runtime only stores and
//! indexes claims.

use frame::prelude::*;
use sp_runtime::{traits::AccountIdConversion, Permill};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    DecodeWithMemTracking,
    Encode,
    MaxEncodedLen,
    PartialEq,
    Eq,
    TypeInfo,
)]
pub enum Disclosure {
    Public,
    Unlisted,
    Gated,
    Sealed,
}

#[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, MaxEncodedLen, PartialEq, Eq, TypeInfo,
)]
pub enum CellRef<Hash> {
    Raw(u64),
    Commitment(Hash),
}

#[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    DecodeWithMemTracking,
    Encode,
    MaxEncodedLen,
    PartialEq,
    Eq,
    TypeInfo,
)]
pub enum SettlementPolicy<CommunityId> {
    Holder,
    Licensor,
    Members,
    Community(CommunityId),
}

#[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    DecodeWithMemTracking,
    Encode,
    MaxEncodedLen,
    PartialEq,
    Eq,
    TypeInfo,
)]
pub enum DisclosureScope {
    Location,
    Membership,
    Activity,
    All,
}

#[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, MaxEncodedLen, PartialEq, Eq, TypeInfo,
)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound(T: Config))]
pub struct Pitch<T: Config> {
    pub disclosure: Disclosure,
    pub cell: CellRef<T::Hash>,
    pub resolution: Option<u8>,
    pub start: BlockNumberFor<T>,
    pub end: BlockNumberFor<T>,
    pub holder: T::AccountId,
    pub licensor: Option<T::CommunityId>,
    pub local_tax: Permill,
    pub membership_root: Option<T::Hash>,
    pub on_dissolve: SettlementPolicy<T::CommunityId>,
    pub residue: Option<BoundedVec<u8, T::MaxResidueLen>>,
}

pub type PitchId = u64;
pub type MembershipProofOf<T> = BoundedVec<u8, <T as pallet::Config>::MaxProofLen>;
pub type ResidueOf<T> = BoundedVec<u8, <T as pallet::Config>::MaxResidueLen>;

pub trait MembershipVerifier<AccountId, Hash> {
    fn verify(who: &AccountId, root: &Hash, proof: &[u8]) -> bool;
}

impl<AccountId, Hash> MembershipVerifier<AccountId, Hash> for () {
    fn verify(_: &AccountId, _: &Hash, _: &[u8]) -> bool {
        false
    }
}

#[frame::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type WeightInfo: WeightInfo;

        type CommunityId: Parameter + MaxEncodedLen + Copy;

        type Verifier: MembershipVerifier<Self::AccountId, Self::Hash>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;

        #[pallet::constant]
        type MaxPitchesPerCell: Get<u32>;

        #[pallet::constant]
        type MaxProofLen: Get<u32>;

        #[pallet::constant]
        type MaxResidueLen: Get<u32>;

        #[pallet::constant]
        type GracePeriod: Get<BlockNumberFor<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    pub type NextPitchId<T> = StorageValue<_, PitchId, ValueQuery>;

    #[pallet::storage]
    pub type Pitches<T: Config> = StorageMap<_, Blake2_128Concat, PitchId, Pitch<T>>;

    #[pallet::storage]
    pub type ByCell<T: Config> =
        StorageMap<_, Twox64Concat, u64, BoundedVec<PitchId, T::MaxPitchesPerCell>, ValueQuery>;

    #[pallet::storage]
    pub type Expiring<T: Config> = StorageMap<
        _,
        Twox64Concat,
        BlockNumberFor<T>,
        BoundedVec<PitchId, T::MaxPitchesPerCell>,
        ValueQuery,
    >;

    #[pallet::storage]
    pub type Joined<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        PitchId,
        Blake2_128Concat,
        T::AccountId,
        (),
        OptionQuery,
    >;

    #[pallet::storage]
    pub type DisclosureGrants<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        PitchId,
        Blake2_128Concat,
        T::AccountId,
        DisclosureScope,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Claimed {
            pitch: PitchId,
            disclosure: Disclosure,
            holder: T::AccountId,
        },
        Joined {
            pitch: PitchId,
            who: T::AccountId,
        },
        Amended {
            pitch: PitchId,
        },
        Dissolved {
            pitch: PitchId,
        },
        DisclosureGranted {
            pitch: PitchId,
            to: T::AccountId,
            scope: DisclosureScope,
        },
        Reaped {
            pitch: PitchId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        BadCellRef,
        BadResolution,
        BadWindow,
        ExpiryScheduleFull,
        IndexFull,
        NotHolder,
        NotLive,
        PitchMissing,
        ProofInvalid,
        WindowInPast,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::claim())]
        pub fn claim(
            origin: OriginFor<T>,
            cell: CellRef<T::Hash>,
            resolution: Option<u8>,
            start: BlockNumberFor<T>,
            end: BlockNumberFor<T>,
            disclosure: Disclosure,
            local_tax: Permill,
            membership_root: Option<T::Hash>,
            on_dissolve: SettlementPolicy<T::CommunityId>,
            licensor: Option<T::CommunityId>,
            residue: Option<ResidueOf<T>>,
        ) -> DispatchResult {
            let holder = ensure_signed(origin)?;
            Self::ensure_cell_matches_disclosure(&cell, disclosure)?;
            Self::ensure_resolution(&cell, resolution)?;
            Self::ensure_window(start, end)?;

            let pitch = NextPitchId::<T>::get();
            let record = Pitch::<T> {
                disclosure,
                cell: cell.clone(),
                resolution,
                start,
                end,
                holder: holder.clone(),
                licensor,
                local_tax,
                membership_root,
                on_dissolve,
                residue,
            };

            Pitches::<T>::insert(pitch, record);
            NextPitchId::<T>::put(pitch.checked_add(1).ok_or(ArithmeticError::Overflow)?);
            Self::index_pitch(pitch, &cell)?;
            Expiring::<T>::try_mutate(end.saturating_add(T::GracePeriod::get()), |ids| {
                ids.try_push(pitch)
                    .map_err(|_| Error::<T>::ExpiryScheduleFull)
            })?;

            Self::deposit_event(Event::Claimed {
                pitch,
                disclosure,
                holder,
            });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::join())]
        pub fn join(
            origin: OriginFor<T>,
            pitch: PitchId,
            proof: MembershipProofOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let record = Pitches::<T>::get(pitch).ok_or(Error::<T>::PitchMissing)?;
            ensure!(Self::is_live(&record), Error::<T>::NotLive);

            if let Some(root) = record.membership_root {
                ensure!(
                    T::Verifier::verify(&who, &root, proof.as_slice()),
                    Error::<T>::ProofInvalid
                );
            }

            Joined::<T>::insert(pitch, &who, ());
            Self::deposit_event(Event::Joined { pitch, who });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::amend())]
        pub fn amend(
            origin: OriginFor<T>,
            pitch: PitchId,
            local_tax: Option<Permill>,
            membership_root: Option<Option<T::Hash>>,
            end: Option<BlockNumberFor<T>>,
            residue: Option<Option<ResidueOf<T>>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Pitches::<T>::try_mutate(pitch, |maybe_record| {
                let record = maybe_record.as_mut().ok_or(Error::<T>::PitchMissing)?;
                ensure!(record.holder == who, Error::<T>::NotHolder);
                ensure!(Self::is_live(record), Error::<T>::NotLive);

                if let Some(local_tax) = local_tax {
                    record.local_tax = local_tax;
                }
                if let Some(membership_root) = membership_root {
                    record.membership_root = membership_root;
                }
                if let Some(new_end) = end {
                    ensure!(record.start < new_end, Error::<T>::BadWindow);
                    record.end = new_end;
                    Expiring::<T>::try_mutate(
                        new_end.saturating_add(T::GracePeriod::get()),
                        |ids| {
                            if !ids.contains(&pitch) {
                                ids.try_push(pitch)
                                    .map_err(|_| Error::<T>::ExpiryScheduleFull)?;
                            }
                            Ok::<_, DispatchError>(())
                        },
                    )?;
                }
                if let Some(residue) = residue {
                    record.residue = residue;
                }

                Ok::<_, DispatchError>(())
            })?;

            Self::deposit_event(Event::Amended { pitch });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::dissolve())]
        pub fn dissolve(origin: OriginFor<T>, pitch: PitchId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let record = Pitches::<T>::get(pitch).ok_or(Error::<T>::PitchMissing)?;
            ensure!(record.holder == who, Error::<T>::NotHolder);
            Self::remove_pitch(pitch, &record);
            Self::deposit_event(Event::Dissolved { pitch });
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::grant_disclosure())]
        pub fn grant_disclosure(
            origin: OriginFor<T>,
            pitch: PitchId,
            to: T::AccountId,
            scope: DisclosureScope,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let record = Pitches::<T>::get(pitch).ok_or(Error::<T>::PitchMissing)?;
            ensure!(record.holder == who, Error::<T>::NotHolder);

            DisclosureGrants::<T>::insert(pitch, &to, scope);
            Self::deposit_event(Event::DisclosureGranted { pitch, to, scope });
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::reap())]
        pub fn reap(origin: OriginFor<T>, pitch: PitchId) -> DispatchResult {
            ensure_signed(origin)?;
            let record = Pitches::<T>::get(pitch).ok_or(Error::<T>::PitchMissing)?;
            let now = frame_system::Pallet::<T>::block_number();
            ensure!(
                now >= record.end.saturating_add(T::GracePeriod::get()),
                Error::<T>::NotLive
            );
            Self::remove_pitch(pitch, &record);
            Self::deposit_event(Event::Reaped { pitch });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn pitch_account(pitch: PitchId) -> T::AccountId {
            T::PalletId::get().into_sub_account_truncating((pitch, b"pitch"))
        }

        fn ensure_cell_matches_disclosure(
            cell: &CellRef<T::Hash>,
            disclosure: Disclosure,
        ) -> DispatchResult {
            match (disclosure, cell) {
                (Disclosure::Public, CellRef::Raw(_)) => Ok(()),
                (Disclosure::Unlisted | Disclosure::Sealed, CellRef::Commitment(_)) => Ok(()),
                (Disclosure::Gated, CellRef::Raw(_) | CellRef::Commitment(_)) => Ok(()),
                _ => Err(Error::<T>::BadCellRef.into()),
            }
        }

        fn ensure_resolution(cell: &CellRef<T::Hash>, resolution: Option<u8>) -> DispatchResult {
            match (cell, resolution) {
                (CellRef::Raw(_), Some(resolution)) if resolution <= 15 => Ok(()),
                (CellRef::Commitment(_), None) => Ok(()),
                _ => Err(Error::<T>::BadResolution.into()),
            }
        }

        fn ensure_window(start: BlockNumberFor<T>, end: BlockNumberFor<T>) -> DispatchResult {
            let now = frame_system::Pallet::<T>::block_number();
            ensure!(start >= now, Error::<T>::WindowInPast);
            ensure!(start < end, Error::<T>::BadWindow);
            Ok(())
        }

        fn is_live(record: &Pitch<T>) -> bool {
            let now = frame_system::Pallet::<T>::block_number();
            record.start <= now && now < record.end
        }

        fn index_pitch(pitch: PitchId, cell: &CellRef<T::Hash>) -> DispatchResult {
            if let CellRef::Raw(cell) = cell {
                ByCell::<T>::try_mutate(cell, |ids| {
                    ids.try_push(pitch).map_err(|_| Error::<T>::IndexFull)
                })?;
            }
            Ok(())
        }

        fn remove_pitch(pitch: PitchId, record: &Pitch<T>) {
            if let CellRef::Raw(cell) = record.cell {
                ByCell::<T>::mutate(cell, |ids| {
                    if let Some(pos) = ids.iter().position(|id| *id == pitch) {
                        ids.remove(pos);
                    }
                });
            }
            Pitches::<T>::remove(pitch);
            let _ = Joined::<T>::clear_prefix(pitch, u32::MAX, None);
            let _ = DisclosureGrants::<T>::clear_prefix(pitch, u32::MAX, None);
        }
    }
}
