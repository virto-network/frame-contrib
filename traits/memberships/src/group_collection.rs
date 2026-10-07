//! Memberships kept in their group's own collection (`DEC-17`, `DEC-18`, `DEC-26`, `DEC-36`).

use crate::*;
use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    sp_runtime::traits::Convert,
    storage::{storage_prefix, unhashed, with_storage_layer},
    traits::{tokens::nonfungibles_v2 as nonfungibles, Get},
    Blake2_128Concat, StorageHasher,
};

/// Memberships kept in their group's own collection of a non-fungibles system `NF`.
///
/// - A group is a collection, and a membership an item of it. Its identifiers are `NF`'s.
/// - `GroupAccount` names the account of each group: it holds the group's **stock**, the
///   memberships not assigned to anyone. It is never a member of its own group (REQ-MI-13): its
///   stock is not returned by any membership query, and it never receives a membership.
/// - `ManagerAccount` names the account that owns and administers every membership collection
///   (`DEC-36`). The runtime creates collections with it as owner and admin; this type never signs
///   anything with it, and only exposes it through [`Self::manager_account`].
/// - `RetirementHolder` names the account that holds the memberships a group retires, until they
///   are burnt (`DEC-19`), or `None` (the default, `()`) for a deployment that retires none. Like
///   a group's account, it is never a member of any group: no membership query returns what it
///   holds, it never receives an assignment or a transfer, and what it holds cannot be released
///   back to a stock. Only [`Issue::retire`] moves an item to it, and only [`Issue::burn`] takes
///   one from it. Reading it takes no storage read when it is a constant.
///
/// Every membership item is locked against transfers and burns
/// (`nonfungibles_v2::Transfer::disable_transfer`, which `pallet-nfts` keeps as its
/// `TransferDisabled` system attribute, in the `Pallet` namespace that no signed caller can
/// write). So a holder, a delegate or a buyer cannot move or burn one through the collection
/// system: only this manager does, unlocking, acting and relocking within one call (`DEC-18`).
/// Issuers lock items when they mint them, through [`Issue::issue`].
///
/// **The membership index.** The collection system keeps no index from an item to its collection,
/// so this type keeps one: for each membership, the group whose collection holds it (REQ-MI-14: an
/// identifier names one membership of one group). [`Issue::issue`] writes it, [`Issue::burn`]
/// removes it, and [`Issue::index`] writes it for a membership minted otherwise (a runtime
/// migration indexes existing memberships with it). [`Inspect::check_membership`] reads it, then
/// the item's holder: two reads, whatever `who` holds. A membership that is not indexed is not
/// found.
///
/// The index lives in unhashed storage, under the key
/// `twox_128(b"GroupCollectionMemberships") ++ twox_128(b"GroupOf") ++ blake2_128_concat(m.encode())`,
/// and its value is the group, SCALE-encoded. A runtime has one such index, shared by every
/// `GroupCollectionMemberships` it defines, so membership identifiers must be unique across all of
/// them: [`Issue::issue`] and [`Issue::index`] refuse an identifier another group's membership
/// already has ([`Error::IdInUse`]). Because the index is shared by every instance, a runtime must
/// use at most one `GroupCollectionMemberships` instance, or ids that are unique across instances.
///
/// The manager keeps its state in attributes of the `Pallet` namespace: each membership's rank
/// ([`ATTR_MEMBER_RANK`]), and each group's member count ([`ATTR_MEMBER_TOTAL`]), rank sum
/// ([`ATTR_MEMBER_RANK_TOTAL`]) and transfer policy ([`ATTR_TRANSFER_POLICY`]). It writes them
/// with `nonfungibles_v2::Mutate`'s attribute methods and reads them with
/// `nonfungibles_v2::Inspect::system_attribute`; in `pallet-nfts` 43, both use
/// `AttributeNamespace::Pallet` (`src/impl_nonfungibles.rs`: `system_attribute`, lines 83-93;
/// `set_attribute`, `set_collection_attribute`, `clear_attribute`, `clear_collection_attribute`,
/// lines 268-376). A membership item carries nothing else: no allowance, weight or expiration
/// (INV-16).
///
/// What the manager clears on an item is its own rank, and the lock it lifts to burn. Any other
/// attribute, such as one written through [`Attributes::set_membership_attribute`] or by the
/// item's holder in its own namespace, stays on the item through a release, a transfer and a
/// burn (`pallet-nfts` 43 does not remove an item's attributes when it burns it); whoever writes
/// one clears it (REQ-MI-4).
pub struct GroupCollectionMemberships<
    NF,
    ItemConfig,
    GroupAccount,
    ManagerAccount,
    RetirementHolder = (),
>(
    PhantomData<(
        NF,
        ItemConfig,
        GroupAccount,
        ManagerAccount,
        RetirementHolder,
    )>,
);

/// Where [`GroupCollectionMemberships`] keeps the group of the membership `m`:
/// `twox_128(b"GroupCollectionMemberships") ++ twox_128(b"GroupOf") ++ blake2_128_concat(m.encode())`.
pub fn membership_index_key<Membership: Encode>(m: &Membership) -> Vec<u8> {
    let mut key = storage_prefix(b"GroupCollectionMemberships", b"GroupOf").to_vec();
    key.extend(Blake2_128Concat::hash(&m.encode()));
    key
}

/// The group the index names for `m`, if any. One read.
fn indexed_group<Group: Decode, Membership: Encode>(m: &Membership) -> Option<Group> {
    unhashed::get(&membership_index_key(m))
}

impl<NF, IC, GA, MA, R> GroupCollectionMemberships<NF, IC, GA, MA, R> {
    /// The account that owns and administers every membership collection (`DEC-36`).
    pub fn manager_account<AccountId>() -> AccountId
    where
        MA: Get<AccountId>,
    {
        MA::get()
    }
}

/// Issues memberships into a group's stock, locked, takes them out of it when the group retires
/// them, and burns them.
///
/// Issuance and retirement belong to the group's issuer (the runtime's), which also holds the
/// deposits and enforces the removal delay; this is the part of both that touches the items.
pub trait Issue<AccountId>: Inspect<AccountId> {
    /// Mints `m` into the stock of `group`, held by the group's account, locks it, and indexes
    /// its group.
    ///
    /// Refuses with [`Error::IdInUse`] if `m` is already indexed as another group's membership.
    fn issue(group: &Self::Group, m: &Self::Membership) -> DispatchResult;

    /// Indexes `m` as a membership of `group`, for a membership minted without [`Issue::issue`]
    /// (a runtime migration indexes the existing ones with it). Idempotent.
    ///
    /// Refuses with [`Error::UnknownMembership`] if `group`'s collection holds no item `m`, and
    /// with [`Error::IdInUse`] if `m` is already indexed as another group's membership.
    fn index(group: &Self::Group, m: &Self::Membership) -> DispatchResult;

    /// Locks `m` against transfers and burns through the collection system, if it isn't already.
    fn lock(group: &Self::Group, m: &Self::Membership) -> DispatchResult;

    /// Takes `m` out of the stock of `group` and gives it to `holder`, the manager's retirement
    /// holder, where it is no member's and cannot be assigned (REQ-MI-D2, `DEC-19`). It stays
    /// locked; the group's member count and rank sum do not change.
    ///
    /// Refuses with [`Error::NotRetirementHolder`] if `holder` is not the manager's retirement
    /// holder, and with [`Error::NotInStock`] if `m` is not in the stock of `group`.
    fn retire(group: &Self::Group, m: &Self::Membership, holder: &AccountId) -> DispatchResult;

    /// Clears the membership's rank if it has one, unlocks it, burns it, and removes it from the
    /// index.
    ///
    /// Only a membership that is not assigned is burnt: one in the stock of `group`, or one its
    /// retirement holder holds. One held by any other account is a member's, and is refused with
    /// [`Error::Assigned`] (REQ-MI-D2: an assigned membership is released first). Burning does
    /// not change the group's member count.
    ///
    /// The rank is the only attribute this clears (with the lock, which unlocking lifts): any
    /// other attribute the item carries, such as one set through
    /// [`Attributes::set_membership_attribute`], is left to whoever wrote it (REQ-MI-4).
    fn burn(group: &Self::Group, m: &Self::Membership) -> DispatchResult;
}

impl<NF, IC, GA, MA, R, AccountId> Inspect<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R>
where
    NF: nonfungibles::Inspect<AccountId> + nonfungibles::InspectEnumerable<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
    NF::ItemId: 'static,
    GA: Convert<NF::CollectionId, AccountId>,
    R: Get<Option<AccountId>>,
    AccountId: Clone + PartialEq + 'static,
{
    type Group = NF::CollectionId;
    type Membership = NF::ItemId;

    /// The memberships `who` holds, lazily, in the collection system's order (deterministic): one
    /// read per item yielded or skipped.
    ///
    /// - In one group: the items `who` holds in its collection, none if `who` is the group's
    ///   account or the retirement holder.
    /// - In every group: the items `who` holds, skipping those of any group whose account is
    ///   `who` (finding a group's account takes no read); none if `who` is the retirement holder.
    fn user_memberships(
        who: &AccountId,
        maybe_group: Option<Self::Group>,
    ) -> Box<dyn Iterator<Item = (Self::Group, Self::Membership)>> {
        match maybe_group {
            Some(group) if is_never_a_member::<GA, R, _, _>(&group, who) => {
                Box::new(core::iter::empty())
            }
            Some(group) => {
                Box::new(NF::owned_in_collection(&group, who).map(move |m| (group.clone(), m)))
            }
            None if is_retirement_holder::<R, _>(who) => Box::new(core::iter::empty()),
            None => {
                let owned = NF::owned(who);
                let who = who.clone();
                Box::new(owned.filter(move |(group, _)| !is_group_account::<GA, _, _>(group, &who)))
            }
        }
    }

    /// The group of the membership `m`, if `who` holds it as a member: the group the index
    /// names (one read), if `who` holds `m` there ([`Inspect::holds`], one read). O(1), whatever
    /// else `who` holds. A membership that is not indexed is not found.
    fn check_membership(who: &AccountId, m: &Self::Membership) -> Option<Self::Group> {
        if is_retirement_holder::<R, _>(who) {
            return None;
        }
        let group = indexed_group::<Self::Group, _>(m)?;
        Self::holds(&group, who, m).then_some(group)
    }

    /// One read: the holder of `m`. Never the group's account or the retirement holder.
    fn holds(group: &Self::Group, who: &AccountId, m: &Self::Membership) -> bool {
        !is_never_a_member::<GA, R, _, _>(group, who) && NF::owner(group, m).as_ref() == Some(who)
    }

    fn members_total(group: &Self::Group) -> u32 {
        NF::typed_system_attribute(group, None, &ATTR_MEMBER_TOTAL).unwrap_or(0u32)
    }
}

impl<NF, IC, GA, MA, R, AccountId> InspectEnumerable<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R>
where
    NF: nonfungibles::Inspect<AccountId> + nonfungibles::InspectEnumerable<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
    NF::ItemId: 'static,
    GA: Convert<NF::CollectionId, AccountId>,
    R: Get<Option<AccountId>>,
    AccountId: Clone + PartialEq + 'static,
{
    /// The stock of `group`: the items its account holds in its collection, lazily.
    fn group_available_memberships(
        group: &Self::Group,
    ) -> Box<dyn Iterator<Item = Self::Membership>> {
        Box::new(NF::owned_in_collection(group, &GA::convert(group.clone())))
    }

    /// The same as [`Inspect::user_memberships`].
    fn memberships_of(
        who: &AccountId,
        maybe_group: Option<Self::Group>,
    ) -> Box<dyn Iterator<Item = (Self::Group, Self::Membership)>> {
        Self::user_memberships(who, maybe_group)
    }
}

impl<NF, IC, GA, MA, R, AccountId> Attributes<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R>
where
    NF: nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>
        + nonfungibles::Mutate<AccountId, IC>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
    NF::ItemId: 'static,
    GA: Convert<NF::CollectionId, AccountId>,
    R: Get<Option<AccountId>>,
    AccountId: Clone + PartialEq + 'static,
{
    /// Reads the attribute from the `Pallet` namespace, where this type writes it.
    fn membership_attribute<K: Encode, V: Parameter>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
    ) -> Option<V> {
        NF::typed_system_attribute(g, Some(m), key)
    }

    /// Refuses the manager's own keys ([`Error::ReservedAttribute`]).
    ///
    /// The attribute is written in the `Pallet` namespace and stays on the item through a release,
    /// a transfer and a burn: the manager clears only its rank (REQ-MI-4). Whoever sets it clears
    /// it, with [`Attributes::clear_membership_attribute`], before the item changes hands or is
    /// burnt.
    fn set_membership_attribute<K: Encode, V: Encode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
        value: &V,
    ) -> Result<(), DispatchError> {
        ensure!(!is_reserved(key), Error::ReservedAttribute);
        NF::set_typed_attribute(g, m, key, value)
    }

    /// Refuses the manager's own keys, and any key whose removal would unlock the item
    /// ([`Error::ReservedAttribute`]).
    fn clear_membership_attribute<K: Encode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
    ) -> Result<(), DispatchError> {
        ensure!(!is_reserved(key), Error::ReservedAttribute);
        with_storage_layer(|| {
            let locked = !NF::can_transfer(g, m);
            NF::clear_typed_attribute(g, m, key)?;
            ensure!(!locked || !NF::can_transfer(g, m), Error::ReservedAttribute);
            Ok(())
        })
    }
}

impl<NF, IC, GA, MA, R, AccountId> Manager<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R>
where
    NF: nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>
        + nonfungibles::Mutate<AccountId, IC>
        + nonfungibles::Transfer<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
    NF::ItemId: 'static,
    GA: Convert<NF::CollectionId, AccountId>,
    R: Get<Option<AccountId>>,
    AccountId: Clone + PartialEq + 'static,
{
    /// Moves `m` from the stock of `group` to `who`, with rank zero, and counts one more member.
    ///
    /// Refuses with [`Error::NotSameGroup`] if `who` is the group's account or the retirement
    /// holder, and with [`Error::NotInStock`] if `m` is not in the stock.
    fn assign(
        group: &Self::Group,
        m: &Self::Membership,
        who: &AccountId,
    ) -> Result<(), DispatchError> {
        ensure!(
            !is_never_a_member::<GA, R, _, _>(group, who),
            Error::NotSameGroup
        );
        ensure!(
            NF::owner(group, m) == Some(GA::convert(group.clone())),
            Error::NotInStock
        );

        with_storage_layer(|| {
            move_item::<NF, AccountId>(group, m, who)?;
            reset_rank::<NF, IC, AccountId>(group, m)?;
            let members = Self::members_total(group).saturating_add(1);
            NF::set_typed_collection_attribute(group, &ATTR_MEMBER_TOTAL, &members)
        })
    }

    /// Moves `m` back to the stock of `group`, clears its rank (adjusting the group's rank sum),
    /// and counts one member less (REQ-MI-4, REQ-MI-8).
    ///
    /// Refuses with [`Error::NotAMember`] if `m` is not held by a member: it is in the stock, the
    /// retirement holder holds it, or it doesn't exist.
    fn release(group: &Self::Group, m: &Self::Membership) -> Result<(), DispatchError> {
        let holder = NF::owner(group, m).ok_or(Error::NotAMember)?;
        ensure!(
            !is_never_a_member::<GA, R, _, _>(group, &holder),
            Error::NotAMember
        );

        with_storage_layer(|| {
            clear_rank::<NF, IC, AccountId>(group, m)?;
            move_item::<NF, AccountId>(group, m, &GA::convert(group.clone()))?;
            let members = Self::members_total(group).saturating_sub(1);
            NF::set_typed_collection_attribute(group, &ATTR_MEMBER_TOTAL, &members)
        })
    }
}

impl<NF, IC, GA, MA, R, AccountId> Transfer<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R>
where
    NF: nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>
        + nonfungibles::Mutate<AccountId, IC>
        + nonfungibles::Transfer<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
    NF::ItemId: 'static,
    GA: Convert<NF::CollectionId, AccountId>,
    R: Get<Option<AccountId>>,
    AccountId: Clone + PartialEq + 'static,
{
    /// One read of the group's collection attribute; the default when it is absent.
    fn transfer_policy(group: &Self::Group) -> TransferPolicy {
        NF::typed_system_attribute(group, None, &ATTR_TRANSFER_POLICY).unwrap_or_default()
    }

    fn set_transfer_policy(
        group: &Self::Group,
        policy: TransferPolicy,
    ) -> Result<(), DispatchError> {
        NF::set_typed_collection_attribute(group, &ATTR_TRANSFER_POLICY, &policy)
    }

    /// Reads the policy once, then moves `m` to `to`, resetting or keeping its rank. The group's
    /// member count stays as it was, and so does its rank sum, but for a rank that is reset.
    ///
    /// The retirement holder is treated as the group's account: what it holds is no member's to
    /// transfer ([`Error::NotAMember`]), and it never receives a transfer
    /// ([`Error::NotSameGroup`]).
    fn transfer(
        group: &Self::Group,
        m: &Self::Membership,
        to: &AccountId,
    ) -> Result<(), DispatchError> {
        let holder = NF::owner(group, m).ok_or(Error::NotAMember)?;
        ensure!(
            !is_never_a_member::<GA, R, _, _>(group, &holder),
            Error::NotAMember
        );
        ensure!(
            !is_never_a_member::<GA, R, _, _>(group, to),
            Error::NotSameGroup
        );

        let policy = Self::transfer_policy(group);
        match policy.receivers {
            Receivers::Disabled => return Err(Error::TransferDisabled.into()),
            Receivers::ToExistingMembers => {
                ensure!(Self::is_member_of(group, to), Error::NotSameGroup)
            }
            Receivers::ToAnyAccount => (),
        }

        with_storage_layer(|| {
            if policy.rank == RankOnTransfer::Reset {
                reset_rank::<NF, IC, AccountId>(group, m)?;
            }
            move_item::<NF, AccountId>(group, m, to)
        })
    }
}

impl<NF, IC, GA, MA, R, AccountId> Issue<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R>
where
    NF: nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>
        + nonfungibles::Mutate<AccountId, IC>
        + nonfungibles::Transfer<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
    NF::ItemId: 'static,
    GA: Convert<NF::CollectionId, AccountId>,
    R: Get<Option<AccountId>>,
    AccountId: Clone + PartialEq + 'static,
    IC: Default,
{
    /// Mints with the default item configuration; the group's account pays any item deposit the
    /// collection system asks for.
    fn issue(group: &Self::Group, m: &Self::Membership) -> DispatchResult {
        ensure!(
            indexed_group::<Self::Group, _>(m).is_none_or(|indexed| &indexed == group),
            Error::IdInUse
        );
        with_storage_layer(|| {
            NF::mint_into(group, m, &GA::convert(group.clone()), &IC::default(), false)?;
            NF::disable_transfer(group, m)?;
            unhashed::put(&membership_index_key(m), group);
            Ok(())
        })
    }

    /// One read of the item's holder, one of the index, one write.
    fn index(group: &Self::Group, m: &Self::Membership) -> DispatchResult {
        ensure!(NF::owner(group, m).is_some(), Error::UnknownMembership);
        ensure!(
            indexed_group::<Self::Group, _>(m).is_none_or(|indexed| &indexed == group),
            Error::IdInUse
        );
        unhashed::put(&membership_index_key(m), group);
        Ok(())
    }

    fn lock(group: &Self::Group, m: &Self::Membership) -> DispatchResult {
        lock_item::<NF, AccountId>(group, m)
    }

    /// One read of the holder, then the manager's move (unlock, transfer, relock).
    fn retire(group: &Self::Group, m: &Self::Membership, holder: &AccountId) -> DispatchResult {
        ensure!(
            is_retirement_holder::<R, _>(holder),
            Error::NotRetirementHolder
        );
        ensure!(
            NF::owner(group, m) == Some(GA::convert(group.clone())),
            Error::NotInStock
        );
        with_storage_layer(|| move_item::<NF, AccountId>(group, m, holder))
    }

    /// One read of the holder before anything is written. A membership that doesn't exist is
    /// left to the collection system to refuse.
    fn burn(group: &Self::Group, m: &Self::Membership) -> DispatchResult {
        if let Some(holder) = NF::owner(group, m) {
            ensure!(
                is_never_a_member::<GA, R, _, _>(group, &holder),
                Error::Assigned
            );
        }
        with_storage_layer(|| {
            clear_rank::<NF, IC, AccountId>(group, m)?;
            unlock_item::<NF, AccountId>(group, m)?;
            NF::burn(group, m, None)?;
            if indexed_group::<Self::Group, _>(m).as_ref() == Some(group) {
                unhashed::kill(&membership_index_key(m));
            }
            Ok(())
        })
    }
}

impl<NF, IC, GA, MA, R, AccountId> Rank<AccountId> for GroupCollectionMemberships<NF, IC, GA, MA, R>
where
    NF: nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>
        + nonfungibles::Mutate<AccountId, IC>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
    NF::ItemId: 'static,
    GA: Convert<NF::CollectionId, AccountId>,
    R: Get<Option<AccountId>>,
    AccountId: Clone + PartialEq + 'static,
{
    /// The rank of an assigned membership; `None` for one in the stock.
    fn rank_of(group: &Self::Group, m: &Self::Membership) -> Option<GenericRank> {
        rank_of::<NF, AccountId>(group, m)
    }

    /// Sets the rank of an assigned membership, adjusting the group's rank sum. Refuses with
    /// [`Error::NotAMember`] for a membership that is not assigned.
    fn set_rank(
        group: &Self::Group,
        m: &Self::Membership,
        rank: impl Into<GenericRank>,
    ) -> Result<(), DispatchError> {
        let prev = Self::rank_of(group, m).ok_or(Error::NotAMember)?;
        set_rank::<NF, IC, AccountId>(group, m, prev, rank.into())
    }

    fn ranks_total(group: &Self::Group) -> u32 {
        ranks_total::<NF, AccountId>(group)
    }
}

/// Whether `who` is the account of `group`. No read.
fn is_group_account<GA, Group, AccountId>(group: &Group, who: &AccountId) -> bool
where
    GA: Convert<Group, AccountId>,
    Group: Clone,
    AccountId: PartialEq,
{
    &GA::convert(group.clone()) == who
}

/// Whether `who` is the manager's retirement holder. No read, for a constant binding.
fn is_retirement_holder<R, AccountId>(who: &AccountId) -> bool
where
    R: Get<Option<AccountId>>,
    AccountId: PartialEq,
{
    R::get().as_ref() == Some(who)
}

/// Whether `who` can never be a member of `group`: it is the group's account (REQ-MI-13) or the
/// retirement holder (REQ-MI-D2). No read, for constant bindings.
fn is_never_a_member<GA, R, Group, AccountId>(group: &Group, who: &AccountId) -> bool
where
    GA: Convert<Group, AccountId>,
    R: Get<Option<AccountId>>,
    Group: Clone,
    AccountId: PartialEq,
{
    is_group_account::<GA, _, _>(group, who) || is_retirement_holder::<R, _>(who)
}

/// Whether `key` is one of the manager's own item attributes.
fn is_reserved<K: Encode>(key: &K) -> bool {
    key.using_encoded(|k| k == ATTR_MEMBER_RANK.encode().as_slice())
}

/// Locks `m` against transfers and burns, unless it cannot be transferred already.
fn lock_item<NF, AccountId>(group: &NF::CollectionId, m: &NF::ItemId) -> DispatchResult
where
    NF: nonfungibles::Transfer<AccountId>,
{
    if NF::can_transfer(group, m) {
        NF::disable_transfer(group, m)
    } else {
        Ok(())
    }
}

/// Unlocks `m`, unless it is unlocked already.
fn unlock_item<NF, AccountId>(group: &NF::CollectionId, m: &NF::ItemId) -> DispatchResult
where
    NF: nonfungibles::Transfer<AccountId>,
{
    if NF::can_transfer(group, m) {
        Ok(())
    } else {
        NF::enable_transfer(group, m)
    }
}

/// The manager's move: unlock, transfer, relock. An item that arrives unlocked leaves locked.
fn move_item<NF, AccountId>(
    group: &NF::CollectionId,
    m: &NF::ItemId,
    to: &AccountId,
) -> DispatchResult
where
    NF: nonfungibles::Transfer<AccountId>,
{
    unlock_item::<NF, AccountId>(group, m)?;
    NF::transfer(group, m, to)?;
    NF::disable_transfer(group, m)
}

fn rank_of<NF, AccountId>(group: &NF::CollectionId, m: &NF::ItemId) -> Option<GenericRank>
where
    NF: nonfungibles::Inspect<AccountId>,
{
    NF::typed_system_attribute(group, Some(m), &ATTR_MEMBER_RANK)
}

fn ranks_total<NF, AccountId>(group: &NF::CollectionId) -> u32
where
    NF: nonfungibles::Inspect<AccountId>,
{
    NF::typed_system_attribute(group, None, &ATTR_MEMBER_RANK_TOTAL).unwrap_or(0u32)
}

/// Replaces the rank `prev` of `m` with `new`, adjusting the group's rank sum.
fn set_rank<NF, IC, AccountId>(
    group: &NF::CollectionId,
    m: &NF::ItemId,
    prev: GenericRank,
    new: GenericRank,
) -> DispatchResult
where
    NF: nonfungibles::Mutate<AccountId, IC>,
{
    let total = ranks_total::<NF, AccountId>(group)
        .saturating_sub(u32::from(prev))
        .saturating_add(u32::from(new));
    NF::set_typed_attribute(group, m, &ATTR_MEMBER_RANK, &new)?;
    NF::set_typed_collection_attribute(group, &ATTR_MEMBER_RANK_TOTAL, &total)
}

/// Sets the rank of `m` to zero, adjusting the group's rank sum.
fn reset_rank<NF, IC, AccountId>(group: &NF::CollectionId, m: &NF::ItemId) -> DispatchResult
where
    NF: nonfungibles::Mutate<AccountId, IC>,
{
    let prev = rank_of::<NF, AccountId>(group, m).unwrap_or(GenericRank::MIN);
    set_rank::<NF, IC, AccountId>(group, m, prev, GenericRank::MIN)
}

/// Removes the rank of `m`, if it has one, adjusting the group's rank sum.
fn clear_rank<NF, IC, AccountId>(group: &NF::CollectionId, m: &NF::ItemId) -> DispatchResult
where
    NF: nonfungibles::Mutate<AccountId, IC>,
{
    if let Some(prev) = rank_of::<NF, AccountId>(group, m) {
        let total = ranks_total::<NF, AccountId>(group).saturating_sub(u32::from(prev));
        NF::set_typed_collection_attribute(group, &ATTR_MEMBER_RANK_TOTAL, &total)?;
        NF::clear_typed_attribute(group, m, &ATTR_MEMBER_RANK)?;
    }
    Ok(())
}
