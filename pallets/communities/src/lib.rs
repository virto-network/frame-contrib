#![cfg_attr(not(feature = "std"), no_std)]
//! # Communities Pallet
//!
//! This pallet enables people to form dynamic collectives refered to as
//! communities. In simpler terms, it can be considered a DAO Factory.
//!
//! - [`Call`]
//! - [`Config`]
//!
//! ## Overview
//!
//! The Communities pallet provides functionality for managing communities,
//! facilitating its participants to have governance over the community entity
//! (and its associated account) which can interect with other systems:
//!
//! - Community registration and removal.
//! - Enrolling/removing members from a community.
//! - Promoting/demoting members within the community.
//! - Voting on proposals to enable community governance.
//!
//! ## Terminology
//!
//! - **Community:** An entity comprised of _members_ —each one defined by their
//!   [`AccountId`][1]— with a given _description_ who can vote on _proposals_
//!   and actively take decisions on behalf of it. Communities are given a
//!   _treasury account_ they can use to hold assets.
//! - **Community Description:** A set of metadata used to identify a community
//!   distinctively. Typically, a name, a description and a URL.
//! - **Community Status:** A community can be either `active` or `blocked`.
//! - **Member:** An [`AccountId`][1] registered into the community as such. Can
//!   have a rank within it and vote in the community's polls.
//! - **Member Rank:** Members could have a rank within the community. This can
//!   determine a voting weight depending on the community's voting mechanism.
//! - **Proposal:** A poll that executes a [call][2] dispatch if approved when
//!   it's closed.
//! - **Community Account:** A keyless [`AccountId`][1] generated on behalf of
//!   the community. Like any regular account can hold balances. It can transfer
//!   funds via a privileged call executed by the community _admin_ or a call
//!   dispatched from a proposal.
//! - **Decision Method:** Can be either rank weighed, member-counted, or
//!   asset-weighed and determines how the votes of proposals will be tallied.
//!
//! ## Lifecycle
//!
//! ```ignore
//! [       ] --> [Pending]               --> [Active]            --> [Blocked]
//! create        set_metadata                set_metadata            unblock
//!                                           block                   
//!                                           add_member              
//!                                           remove_member
//!                                           promote
//!                                           demote
//!                                           set_voting_mechanism
//! ```
//!
//! ## Implementations
//!
//! > TODO: Define which traits we are defining/implementing.
//!
//! ## Interface
//!
//! ### Permissionless Functions
//!
//! - [`apply_for`][c00]: Registers an appliation as a new community, taking an
//!   [existential deposit][3] used to create the community account.
//!
//! ### Permissioned Functions
//!
//! Calling these functions requires being a member of the community.
//!
//! - [`add_member`][c02]: Enroll an account as a community member, giving it a
//!   membership from the community's stock. In theory, any community member
//!   should be able to add a member. However, this can be changed to ensure it
//!   is a privileged function.
//! - `vote`: Adds a vote into a community proposal.
//! - `transfer_membership`: A member hands its membership on to another
//!   account, as the community's transfer policy allows.
//!
//! ### Privileged Functions
//!
//! These functions can be called either by the community _admin_ or
//! dispatched through an approved proposal. !
//! - [`remove_member`][c03]: Removes an account as a community member. While
//!   enrolling a member into the community can be an action taken by any
//!   member, the decision to remove a member should not be taken arbitrarily by
//!   any community member. Also, it shouldn't be possible to arbitrarily remove
//!   the community admin, as some privileged calls would be impossible execute
//!   thereafter.
//! - `promote`: Increases the rank of a member in the community.
//! - `demote`: Decreases the rank of a member in the community.
//! - `set_decision_method`: Means for a community to make decisions.
//! - `set_transfer_policy`: Decides whether and how the community's
//!   memberships may be transferred.
//!
//! ### Public Functions
//!
//! - [`community`][g00]: Stores the basic information of the community. If a
//!   value exists for a specified [`ComumunityId`][t00], this means a community
//!   exists.
//! - [`metadata`][g01]: Stores the metadata regarding a community.
//!
//! <!-- References -->
//! [1]: `frame_system::Config::AccountId`
//! [2]: https://docs.substrate.io/reference/glossary/#call
//! [3]: https://docs.substrate.io/reference/glossary/#existential-deposit
//!
//! [t00]: `Config::CommunityId`
//! [t01]: `types::CommunityMetadata`
//!
//! [c00]: `Pallet::create`
//! [c01]: `Pallet::set_metadata`
//! [c02]: `Pallet::add_member`
//! [c03]: `Pallet::remove_member`
//!
//! [g00]: `Pallet::community`
//! [g01]: `Pallet::metadata`
//! [g02]: `Pallet::membership`
//! [g03]: `Pallet::members_count`

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use core::num::NonZeroU8;
use frame_contrib_traits::memberships::{
    self as membership, Inspect, InspectEnumerable, Manager, Rank, Transfer as MembershipTransfer,
};
use frame_support::{
    pallet_prelude::*,
    traits::{fungible, fungibles, EnsureOrigin, OriginTrait, Polling},
    Blake2_128Concat, Parameter,
};
use frame_system::pallet_prelude::{ensure_signed, OriginFor};
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::{BlockNumberProvider, StaticLookup};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod functions;
mod impls;

pub use pallet::*;

pub mod types;
pub use types::*;

pub mod weights;
pub use weights::*;

pub mod origin;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    const ONE: NonZeroU8 = NonZeroU8::MIN;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it
    /// depends.
    #[pallet::config]
    pub trait Config:
        frame_system::Config<
        RuntimeEvent: From<Event<Self>>,
        RuntimeCall: From<Call<Self>>,
        RuntimeOrigin: From<Origin<Self>>,
    >
    where
        AssetIdOf<Self>: MaybeSerializeDeserialize,
    {
        // Primitives: Some overarching types that come from the system (or the system depends on).

        /// The overarching freeze reason.
        type RuntimeFreezeReason: From<FreezeReason>;
        /// Type representing the weight of this pallet
        type WeightInfo: WeightInfo;

        // Origins: Types that manage authorization rules to allow or deny some caller origins to
        // execute a method.
        /// Origin authorized to create a new community.
        type CreateOrigin: EnsureOrigin<
            OriginFor<Self>,
            Success = Option<(NativeBalanceOf<Self>, AccountIdOf<Self>, AccountIdOf<Self>)>,
        >;
        /// Origin authorized to administer an active community
        type AdminOrigin: EnsureOrigin<OriginFor<Self>, Success = Self::CommunityId>;
        /// Origin authorized to manage memeberships of an active community
        type MemberMgmtOrigin: EnsureOrigin<OriginFor<Self>, Success = Self::CommunityId>;
        /// Origin of a member acting on a membership it holds (e.g. transferring it).
        type MemberOrigin: EnsureOrigin<OriginFor<Self>, Success = AccountIdOf<Self>>;

        // Types: A set of parameter types that the pallet uses to handle information.

        /// This type represents an unique ID for the community
        type CommunityId: Parameter + MaxEncodedLen + Copy + MaybeSerializeDeserialize;
        /// This type represents an unique ID to identify a membership within a
        /// community
        type MembershipId: Parameter + MaxEncodedLen + Copy + MaybeSerializeDeserialize;

        // Dependencies: The external components this pallet depends on.

        /// Means to manage memberships of a community. A community's stock (its
        /// unassigned memberships) is what `add_member` assigns from, and a member
        /// transfers a membership as the community's transfer policy allows.
        type MemberMgmt: Inspect<Self::AccountId, Group = CommunityIdOf<Self>, Membership = MembershipIdOf<Self>>
            + InspectEnumerable<
                Self::AccountId,
                Group = CommunityIdOf<Self>,
                Membership = MembershipIdOf<Self>,
            > + Manager<Self::AccountId, Group = CommunityIdOf<Self>, Membership = MembershipIdOf<Self>>
            + MembershipTransfer<
                Self::AccountId,
                Group = CommunityIdOf<Self>,
                Membership = MembershipIdOf<Self>,
            > + Rank<Self::AccountId, Group = CommunityIdOf<Self>, Membership = MembershipIdOf<Self>>;
        /// Means to read and mutate the state of a poll.
        type Polls: Polling<
            Tally<Self>,
            Class = CommunityIdOf<Self>,
            Index = u32,
            Votes = VoteWeight,
            Moment = BlockNumberFor<Self>,
        >;
        /// Type represents interactions between fungibles (i.e. assets)
        type Assets: fungibles::Inspect<Self::AccountId, Balance = NativeBalanceOf<Self>>
            + fungibles::Mutate<Self::AccountId>
            + fungibles::Create<Self::AccountId>;
        /// Type allows for handling fungibles' freezes
        type AssetsFreezer: fungibles::Inspect<
                Self::AccountId,
                Balance = NativeBalanceOf<Self>,
                AssetId = AssetIdOf<Self>,
            > + fungibles::freeze::Inspect<
                Self::AccountId,
                Id = Self::RuntimeFreezeReason,
                AssetId = AssetIdOf<Self>,
            > + fungibles::freeze::Mutate<
                Self::AccountId,
                Id = Self::RuntimeFreezeReason,
                AssetId = AssetIdOf<Self>,
            >;
        /// Type represents interactions between fungible tokens (native token)
        type Balances: fungible::Inspect<Self::AccountId>
            + fungible::Mutate<Self::AccountId>
            + fungible::freeze::Inspect<Self::AccountId, Id = Self::RuntimeFreezeReason>
            + fungible::freeze::Mutate<Self::AccountId, Id = Self::RuntimeFreezeReason>;
        /// Component that provides the current block number.
        type BlockNumberProvider: BlockNumberProvider;

        // Parameters: A set of constant parameters to configure limits.

        /// The pallet id used for deriving sovereign account IDs.
        #[pallet::constant]
        type PalletId: Get<frame_support::PalletId>;

        // Benchmarking: Types to handle benchmarks.

        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self>;
    }

    /// The origin of the pallet
    #[pallet::origin]
    pub type Origin<T> = origin::RawOrigin<T>;

    /// A reason for the pallet communities placing a freeze on funds.
    #[pallet::composite_enum]
    pub enum FreezeReason {
        // A vote has been casted on a poll
        VoteCasted,
    }

    /// Stores the basic information of the community. If a value exists for a
    /// specified [`ComumunityId`][`Config::CommunityId`], this means a
    /// community exists.
    #[pallet::storage]
    pub(super) type Info<T> = StorageMap<_, Blake2_128Concat, CommunityIdOf<T>, CommunityInfo>;

    /// List of origins and how they map to communities
    #[pallet::storage]
    pub(super) type CommunityIdFor<T> =
        StorageMap<_, Blake2_128Concat, PalletsOriginOf<T>, CommunityIdOf<T>>;

    /// Stores the decision method for a community
    #[pallet::storage]
    pub(super) type CommunityDecisionMethod<T> =
        StorageMap<_, Blake2_128Concat, CommunityIdOf<T>, DecisionMethodFor<T>, ValueQuery>;

    /// Stores the list of votes for a community.
    #[pallet::storage]
    pub(super) type CommunityVotes<T> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        PollIndexOf<T>,
        Blake2_128Concat,
        MembershipIdOf<T>,
        (VoteOf<T>, AccountIdOf<T>),
    >;

    /// Stores the list of votes for a community.
    #[pallet::storage]
    pub(super) type CommunityVoteLocks<T> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        AccountIdOf<T>,
        Blake2_128Concat,
        PollIndexOf<T>,
        VoteOf<T>,
    >;

    /// The weight each vote in [`CommunityVotes`] added to its poll's tally (its weight times its
    /// multiplier, such as the membership's rank), so removing the vote subtracts exactly that,
    /// whatever happened to the rank or the decision method since.
    ///
    /// A vote cast before this record existed has none; removing it recomputes the weight from
    /// the current rank, as it did then.
    #[pallet::storage]
    pub(super) type CommunityVoteWeights<T> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        PollIndexOf<T>,
        Blake2_128Concat,
        MembershipIdOf<T>,
        VoteWeight,
    >;

    // Pallets use events to inform users when important changes are made.
    // https://docs.substrate.io/main-docs/build/events-errors/
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A [`Commmunity`][`types::Community`] has been created.
        CommunityCreated {
            id: T::CommunityId,
            origin: PalletsOriginOf<T>,
        },
        AdminOriginSet {
            id: T::CommunityId,
            origin: PalletsOriginOf<T>,
        },
        DecisionMethodSet {
            id: T::CommunityId,
        },
        MemberAdded {
            who: AccountIdOf<T>,
            membership_id: MembershipIdOf<T>,
        },
        MemberRemoved {
            who: AccountIdOf<T>,
            membership_id: MembershipIdOf<T>,
        },
        MembershipRankUpdated {
            membership_id: MembershipIdOf<T>,
            rank: membership::GenericRank,
        },
        VoteCasted {
            who: AccountIdOf<T>,
            poll_index: PollIndexOf<T>,
            vote: VoteOf<T>,
        },
        VoteRemoved {
            who: AccountIdOf<T>,
            poll_index: PollIndexOf<T>,
        },
        /// A member transferred a membership of the community to another account.
        MembershipTransferred {
            id: T::CommunityId,
            membership_id: MembershipIdOf<T>,
            from: AccountIdOf<T>,
            to: AccountIdOf<T>,
        },
        /// The community set the transfer policy of its memberships.
        TransferPolicySet {
            id: T::CommunityId,
            policy: membership::TransferPolicy,
        },
    }

    // Errors inform users that something worked or went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// The community doesn't exist in storage, nor they have members.
        CommunityDoesNotExist,
        /// A community with the same [`CommunityId`][`Config::CommunityId`]
        /// already exists, therefore cannot be applied for.
        CommunityAlreadyExists,
        /// The community can't introduce new members at the moment
        CommunityAtCapacity,
        /// The specified [`AccountId`][`frame_system::Config::AccountId`] is
        /// not a member of the community
        NotAMember,
        /// The indicated index corresponds to a poll that is already ongoing
        AlreadyOngoing,
        /// The indicated index corresponds to a poll that is not ongoing
        NotOngoing,
        /// The track for the poll voted for does not correspond to the
        /// community ID
        InvalidTrack,
        /// The vote type does not correspond with the community's selected
        /// [`DecisionMethod`][`origin::DecisionMethod`]
        InvalidVoteType,
        /// The signer tried to remove a vote from a poll they haven't
        /// casted a vote yet, or they have already removed it from.
        NoVoteCasted,
        /// The poll
        NoLocksInPlace,
        /// The origin already controls another community
        AlreadyAdmin,
        /// The vote is below the minimum requried
        VoteBelowMinimum,
        /// The community's transfer policy allows no transfers of its memberships
        TransferDisabled,
        /// The community's transfer policy does not allow the recipient to
        /// receive the membership, or the recipient is the community account
        NotSameGroup,
    }

    // Dispatchable functions allows users to interact with the pallet and invoke
    // state changes. These functions materialize as "extrinsics", which are often
    // compared to transactions. Dispatchable functions must be annotated with a
    // weight and must return a DispatchResult.
    #[pallet::call(weight(<T as Config>::WeightInfo))]
    impl<T: Config> Pallet<T> {
        /// Creates a new community managed by the given origin
        #[pallet::call_index(0)]
        pub fn create(
            origin: OriginFor<T>,
            admin_origin: PalletsOriginOf<T>,
            community_id: T::CommunityId,
        ) -> DispatchResult {
            let maybe_deposit = T::CreateOrigin::ensure_origin(origin)?;

            Self::register(&admin_origin, &community_id, maybe_deposit)?;

            Self::deposit_event(crate::Event::CommunityCreated {
                id: community_id,
                origin: admin_origin,
            });
            Ok(())
        }

        /// Creates a new community managed by the given origin
        #[pallet::call_index(1)]
        pub fn set_admin_origin(
            origin: OriginFor<T>,
            admin_origin: PalletsOriginOf<T>,
        ) -> DispatchResult {
            let community_id = T::AdminOrigin::ensure_origin(origin.clone())?;

            ensure!(
                CommunityIdFor::<T>::get(origin.clone().caller()) == Some(community_id),
                DispatchError::BadOrigin
            );

            CommunityIdFor::<T>::remove(origin.caller());
            CommunityIdFor::<T>::insert(admin_origin.clone(), community_id);

            Self::deposit_event(Event::AdminOriginSet {
                id: community_id,
                origin: admin_origin,
            });
            Ok(())
        }

        // === Memberships management ===

        /// Enroll an account as a community member that receives a membership
        /// from the community's stock (its unassigned memberships).
        ///
        /// Fails with `CommunityAtCapacity` if the stock is empty.
        #[pallet::call_index(3)]
        pub fn add_member(origin: OriginFor<T>, who: AccountIdLookupOf<T>) -> DispatchResult {
            let community_id = T::MemberMgmtOrigin::ensure_origin(origin)?;
            let who = T::Lookup::lookup(who)?;

            let membership_id = T::MemberMgmt::group_available_memberships(&community_id)
                .next()
                .ok_or(Error::<T>::CommunityAtCapacity)?;

            T::MemberMgmt::assign(&community_id, &membership_id, &who)
                .map_err(Self::membership_error)?;

            Self::deposit_event(Event::MemberAdded { who, membership_id });
            Ok(())
        }

        /// Removes an account as a community member. While
        /// enrolling a member into the community can be an action taken by any
        /// member, the decision to remove a member should not be taken
        /// arbitrarily by any community member. Also, it shouldn't be possible
        /// to arbitrarily remove the community admin, as some privileged calls
        /// would be impossible to execute thereafter.
        ///
        /// Fails with `NotAMember` unless `who` holds `membership_id` in the
        /// community.
        #[pallet::call_index(4)]
        pub fn remove_member(
            origin: OriginFor<T>,
            who: AccountIdLookupOf<T>,
            membership_id: MembershipIdOf<T>,
        ) -> DispatchResult {
            let community_id = T::MemberMgmtOrigin::ensure_origin(origin)?;
            let who = T::Lookup::lookup(who)?;

            ensure!(
                T::MemberMgmt::holds(&community_id, &who, &membership_id),
                Error::<T>::NotAMember
            );

            T::MemberMgmt::release(&community_id, &membership_id)
                .map_err(Self::membership_error)?;

            Self::deposit_event(Event::MemberRemoved { who, membership_id });
            Ok(())
        }

        /// Increases the rank of a member in the community
        #[pallet::call_index(5)]
        pub fn promote(origin: OriginFor<T>, membership_id: MembershipIdOf<T>) -> DispatchResult {
            let community_id = T::MemberMgmtOrigin::ensure_origin(origin)?;

            let rank = T::MemberMgmt::rank_of(&community_id, &membership_id)
                .ok_or(Error::<T>::NotAMember)?
                .promote_by(ONE);
            T::MemberMgmt::set_rank(&community_id, &membership_id, rank)?;

            Self::deposit_event(Event::MembershipRankUpdated {
                membership_id,
                rank,
            });
            Ok(())
        }

        /// Decreases the rank of a member in the community
        #[pallet::call_index(6)]
        pub fn demote(origin: OriginFor<T>, membership_id: MembershipIdOf<T>) -> DispatchResult {
            let community_id = T::MemberMgmtOrigin::ensure_origin(origin)?;

            let rank = T::MemberMgmt::rank_of(&community_id, &membership_id)
                .ok_or(Error::<T>::NotAMember)?;
            T::MemberMgmt::set_rank(&community_id, &membership_id, rank.demote_by(ONE))?;

            Self::deposit_event(Event::MembershipRankUpdated {
                membership_id,
                rank,
            });
            Ok(())
        }

        // === Governance ===

        /// Decide the method used by the community to vote on proposals.
        ///
        /// The origin must be the admin of `community_id`; the admin of any
        /// other community is rejected with `BadOrigin`.
        #[pallet::call_index(7)]
        pub fn set_decision_method(
            origin: OriginFor<T>,
            community_id: T::CommunityId,
            decision_method: DecisionMethodFor<T>,
        ) -> DispatchResult {
            ensure!(
                T::AdminOrigin::ensure_origin(origin)? == community_id,
                DispatchError::BadOrigin
            );
            if let DecisionMethod::CommunityAsset(ref asset, min_vote) = decision_method {
                // best effort attemt to create the asset if it doesn't exist
                let _ = <T::Assets as fungibles::Create<T::AccountId>>::create(
                    asset.clone(),
                    T::PalletId::get().into_account_truncating(),
                    false,
                    min_vote,
                );
            }
            CommunityDecisionMethod::<T>::set(community_id, decision_method);
            Self::deposit_event(Event::DecisionMethodSet { id: community_id });
            Ok(())
        }

        /// Cast a vote on an on-going referendum
        #[pallet::call_index(8)]
        pub fn vote(
            origin: OriginFor<T>,
            membership_id: MembershipIdOf<T>,
            #[pallet::compact] poll_index: PollIndexOf<T>,
            vote: VoteOf<T>,
        ) -> DispatchResult {
            ensure!(VoteWeight::from(&vote).gt(&0), Error::<T>::VoteBelowMinimum);
            let who = ensure_signed(origin)?;
            let community_id = T::MemberMgmt::check_membership(&who, &membership_id)
                .ok_or(Error::<T>::NotAMember)?;
            let decision_method = CommunityDecisionMethod::<T>::get(community_id);
            if CommunityVotes::<T>::contains_key(poll_index, membership_id) {
                Self::try_remove_vote(&community_id, &decision_method, &membership_id, poll_index)?;
            }
            Self::try_vote(
                &community_id,
                &decision_method,
                &who,
                &membership_id,
                poll_index,
                &vote,
            )?;
            Self::deposit_event(Event::<T>::VoteCasted {
                who: who.clone(),
                poll_index,
                vote,
            });
            Ok(())
        }

        /// Remove any previous vote on a given referendum
        #[pallet::call_index(9)]
        pub fn remove_vote(
            origin: OriginFor<T>,
            membership_id: MembershipIdOf<T>,
            #[pallet::compact] poll_index: PollIndexOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let community_id = T::MemberMgmt::check_membership(&who, &membership_id)
                .ok_or(Error::<T>::NotAMember)?;
            let decision_method = CommunityDecisionMethod::<T>::get(community_id);
            Self::try_remove_vote(&community_id, &decision_method, &membership_id, poll_index)?;
            Self::deposit_event(Event::<T>::VoteRemoved {
                who: who.clone(),
                poll_index,
            });
            Ok(())
        }

        /// Make previously held or locked funds from a vote available
        // if the refereundum has finished
        #[pallet::call_index(10)]
        pub fn unlock(
            origin: OriginFor<T>,
            #[pallet::compact] poll_index: PollIndexOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                T::Polls::as_ongoing(poll_index).is_none(),
                Error::<T>::AlreadyOngoing
            );
            let vote =
                CommunityVoteLocks::<T>::get(&who, poll_index).ok_or(Error::<T>::NoLocksInPlace)?;
            Self::update_locks(&who, poll_index, &vote, LockUpdateType::Remove)
        }

        /// Dispatch a callable as the community account
        #[pallet::call_index(11)]
        #[pallet::weight({
			let di = call.get_dispatch_info();
			let weight = T::WeightInfo::dispatch_as_account()
				.saturating_add(T::DbWeight::get().reads_writes(1, 1))
				.saturating_add(di.call_weight);
			(weight, di.class)
		})]
        pub fn dispatch_as_account(
            origin: OriginFor<T>,
            call: Box<RuntimeCallFor<T>>,
        ) -> DispatchResult {
            let community_id = T::MemberMgmtOrigin::ensure_origin(origin)?;
            Self::do_dispatch_as_community_account(&community_id, *call)
        }

        /// Hand a membership the caller holds on to another account, as the
        /// community's transfer policy allows.
        ///
        /// Fails with `NotAMember` if the caller does not hold `membership_id`,
        /// `TransferDisabled` if the community's policy allows no transfers, and
        /// `NotSameGroup` if the policy does not allow `to` to receive it, or `to`
        /// is the community account.
        #[pallet::call_index(12)]
        pub fn transfer_membership(
            origin: OriginFor<T>,
            membership_id: MembershipIdOf<T>,
            to: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            let from = T::MemberOrigin::ensure_origin(origin)?;
            let to = T::Lookup::lookup(to)?;

            let community_id = T::MemberMgmt::check_membership(&from, &membership_id)
                .ok_or(Error::<T>::NotAMember)?;
            T::MemberMgmt::transfer(&community_id, &membership_id, &to)
                .map_err(Self::membership_error)?;

            Self::deposit_event(Event::MembershipTransferred {
                id: community_id,
                membership_id,
                from,
                to,
            });
            Ok(())
        }

        /// Decide whether and how the community's memberships may be
        /// transferred. Until a community sets one, its policy allows no
        /// transfers.
        #[pallet::call_index(13)]
        pub fn set_transfer_policy(
            origin: OriginFor<T>,
            policy: membership::TransferPolicy,
        ) -> DispatchResult {
            let community_id = T::AdminOrigin::ensure_origin(origin)?;

            T::MemberMgmt::set_transfer_policy(&community_id, policy)?;

            Self::deposit_event(Event::TransferPolicySet {
                id: community_id,
                policy,
            });
            Ok(())
        }
    }
}
