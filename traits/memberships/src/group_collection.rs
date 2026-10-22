//! Memberships kept in their group's own collection.

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
///   memberships not assigned to anyone. It is never a member of its own group: its stock is not
///   returned by any membership query, and it never receives a membership.
/// - `ManagerAccount` names the account that owns and administers every membership collection.
///   The runtime creates collections with it as owner and admin; this type never signs anything
///   with it, and only exposes it through [`Self::manager_account`].
/// - `RetirementHolder` names the account that holds the memberships a group retires, until they
///   are burnt, or `None` (the default, `()`) for a deployment that retires none. Like a group's
///   account, it is never a member of any group: no membership query returns what it holds, it
///   never receives an assignment or a transfer, and what it holds cannot be released back to a
///   stock. Only [`Issue::retire`] moves an item to it, and only [`Issue::burn`] takes one from
///   it. Reading it takes no storage read when it is a constant.
/// - `AttributeKeys` lists the item attribute keys that callers may write through
///   [`Attributes::set_membership_attribute`]; the default, `()`, lists none. Each key is a byte
///   string, matched as a `&[u8]` key (SCALE-encoded, so length-prefixed), like the manager's own
///   keys: a caller passes the same `&[u8]`. The manager's own keys are refused even if listed.
///
/// Every membership item is locked against transfers and burns
/// (`nonfungibles_v2::Transfer::disable_transfer`, which `pallet-nfts` keeps as its
/// `TransferDisabled` system attribute, in the `Pallet` namespace that no signed caller can
/// write). So a holder, a delegate or a buyer cannot move or burn one through the collection
/// system: only this manager does, unlocking, acting and relocking within one call.
/// Issuers lock items when they mint them, through [`Issue::issue`].
///
/// **Atomicity.** Each method checks what it can before it writes, but a later step can still be
/// refused by the collection system after an earlier write (a transfer it refuses, say). Call
/// the methods from a transactional context, such as a dispatchable, so such a refusal rolls back
/// every write of the call.
///
/// **The membership index.** The collection system keeps no index from an item to its collection,
/// so this type keeps one: for each membership, the group whose collection holds it (an
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
/// `nonfungibles_v2::Inspect::system_attribute`, which `pallet-nfts` keeps in its `Pallet`
/// namespace (its `impl_nonfungibles` module: `system_attribute`, `set_attribute`,
/// `set_collection_attribute`, `clear_attribute` and `clear_collection_attribute`). The manager
/// writes nothing else on an item: no allowance, weight or expiration.
///
/// **Attributes.** Callers write item attributes through [`Attributes`], in the same namespace,
/// and only for the keys `AttributeKeys` lists. A release and a burn clear every listed key the
/// item carries, and its rank, so whatever was written for a holder leaves with them; each listed
/// key costs a read at a release or a burn. A transfer keeps them, as it may keep the rank.
/// Attributes others write in their own namespaces of the collection system (a holder's, a
/// delegate's) are not the manager's to clear; the issuer clears those when it burns.
pub struct GroupCollectionMemberships<
    NF,
    ItemConfig,
    GroupAccount,
    ManagerAccount,
    RetirementHolder = (),
    AttributeKeys = (),
>(
    PhantomData<(
        NF,
        ItemConfig,
        GroupAccount,
        ManagerAccount,
        RetirementHolder,
        AttributeKeys,
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

impl<NF, IC, GA, MA, R, AK> GroupCollectionMemberships<NF, IC, GA, MA, R, AK> {
    /// The account that owns and administers every membership collection.
    pub fn manager_account<AccountId>() -> AccountId
    where
        MA: Get<AccountId>,
    {
        MA::get()
    }
}

impl<NF, IC, GA, MA, R, AK, AccountId> Inspect<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R, AK>
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

    /// The memberships `who` holds, in every group, as [`Inspect::user_memberships`] finds them,
    /// counting every item read against `max_reads`: those yielded and those skipped (a group
    /// account's own stock) alike. None, and no read, for the retirement holder.
    fn user_memberships_bounded(
        who: &AccountId,
        max_reads: u32,
    ) -> BoundedMemberships<Self::Group, Self::Membership> {
        if is_retirement_holder::<R, _>(who) {
            return Box::new(core::iter::empty());
        }
        let owned = NF::owned(who);
        let who = who.clone();
        let reads = owned.map(move |(group, m)| {
            (!is_group_account::<GA, _, _>(&group, &who)).then_some((group, m))
        });
        bounded_reads(reads, max_reads)
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

impl<NF, IC, GA, MA, R, AK, AccountId> InspectEnumerable<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R, AK>
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

impl<NF, IC, GA, MA, R, AK, AccountId> Attributes<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R, AK>
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
    AK: Get<&'static [&'static [u8]]>,
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

    /// Writes the attribute in the `Pallet` namespace. A release or a burn of the membership
    /// clears it; a transfer keeps it.
    ///
    /// Refuses the manager's own keys ([`Error::ReservedAttribute`]), and any key `AttributeKeys`
    /// does not list ([`Error::AttributeNotAllowed`]).
    fn set_membership_attribute<K: Encode, V: Encode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
        value: &V,
    ) -> Result<(), DispatchError> {
        ensure!(!is_reserved(key), Error::ReservedAttribute);
        ensure!(is_allowed::<AK, _>(key), Error::AttributeNotAllowed);
        NF::set_typed_attribute(g, m, key, value)
    }

    /// Refuses the manager's own keys and any key whose removal would unlock the item
    /// ([`Error::ReservedAttribute`]), and any key `AttributeKeys` does not list
    /// ([`Error::AttributeNotAllowed`]).
    fn clear_membership_attribute<K: Encode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
    ) -> Result<(), DispatchError> {
        ensure!(!is_reserved(key), Error::ReservedAttribute);
        ensure!(is_allowed::<AK, _>(key), Error::AttributeNotAllowed);
        // The listed keys are the runtime's and the lock's key is the collection system's, so
        // this type cannot tell beforehand whether a listed key is the lock. It clears, checks
        // that a locked item is still locked, and relies on the storage layer to undo the clear
        // when it is not, whatever the caller's context.
        with_storage_layer(|| {
            let locked = !NF::can_transfer(g, m);
            NF::clear_typed_attribute(g, m, key)?;
            ensure!(!locked || !NF::can_transfer(g, m), Error::ReservedAttribute);
            Ok(())
        })
    }
}

impl<NF, IC, GA, MA, R, AK, AccountId> Manager<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R, AK>
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
    AK: Get<&'static [&'static [u8]]>,
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

        move_item::<NF, AccountId>(group, m, who)?;
        reset_rank::<NF, IC, AccountId>(group, m)?;
        let members = Self::members_total(group).saturating_add(1);
        NF::set_typed_collection_attribute(group, &ATTR_MEMBER_TOTAL, &members)
    }

    /// Moves `m` back to the stock of `group`, clears its rank (adjusting the group's rank sum)
    /// and every attribute `AttributeKeys` lists, and counts one member less.
    ///
    /// Refuses with [`Error::NotAMember`] if `m` is not held by a member: it is in the stock, the
    /// retirement holder holds it, or it doesn't exist.
    fn release(group: &Self::Group, m: &Self::Membership) -> Result<(), DispatchError> {
        let holder = NF::owner(group, m).ok_or(Error::NotAMember)?;
        ensure!(
            !is_never_a_member::<GA, R, _, _>(group, &holder),
            Error::NotAMember
        );

        clear_rank::<NF, IC, AccountId>(group, m)?;
        clear_allowed_attributes::<NF, IC, AK, AccountId>(group, m)?;
        move_item::<NF, AccountId>(group, m, &GA::convert(group.clone()))?;
        let members = Self::members_total(group).saturating_sub(1);
        NF::set_typed_collection_attribute(group, &ATTR_MEMBER_TOTAL, &members)
    }
}

impl<NF, IC, GA, MA, R, AK, AccountId> Transfer<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R, AK>
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
    AK: Get<&'static [&'static [u8]]>,
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
    /// member count stays as it was, and so does its rank sum, but for a rank that is reset. The
    /// attributes callers set through [`Attributes`] travel with the membership.
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

        if policy.rank == RankOnTransfer::Reset {
            reset_rank::<NF, IC, AccountId>(group, m)?;
        }
        move_item::<NF, AccountId>(group, m, to)
    }
}

impl<NF, IC, GA, MA, R, AK, AccountId> Issue<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R, AK>
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
    AK: Get<&'static [&'static [u8]]>,
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
        NF::mint_into(group, m, &GA::convert(group.clone()), &IC::default(), false)?;
        NF::disable_transfer(group, m)?;
        unhashed::put(&membership_index_key(m), group);
        Ok(())
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
        move_item::<NF, AccountId>(group, m, holder)
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
        clear_rank::<NF, IC, AccountId>(group, m)?;
        clear_allowed_attributes::<NF, IC, AK, AccountId>(group, m)?;
        unlock_item::<NF, AccountId>(group, m)?;
        NF::burn(group, m, None)?;
        if indexed_group::<Self::Group, _>(m).as_ref() == Some(group) {
            unhashed::kill(&membership_index_key(m));
        }
        Ok(())
    }
}

impl<NF, IC, GA, MA, R, AK, AccountId> Rank<AccountId>
    for GroupCollectionMemberships<NF, IC, GA, MA, R, AK>
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

/// Whether `who` can never be a member of `group`: it is the group's account or the retirement
/// holder. No read, for constant bindings.
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

/// Whether `key` is one of the keys `AK` lists, each matched as a `&[u8]` key.
fn is_allowed<AK, K>(key: &K) -> bool
where
    AK: Get<&'static [&'static [u8]]>,
    K: Encode,
{
    key.using_encoded(|k| AK::get().iter().any(|allowed| allowed.encode() == k))
}

/// Clears every attribute `AK` lists that `m` carries: one read per listed key, and a write per
/// key it carries. The manager's own keys are skipped: they are cleared on their own terms.
fn clear_allowed_attributes<NF, IC, AK, AccountId>(
    group: &NF::CollectionId,
    m: &NF::ItemId,
) -> DispatchResult
where
    NF: nonfungibles::Inspect<AccountId> + nonfungibles::Mutate<AccountId, IC>,
    AK: Get<&'static [&'static [u8]]>,
{
    for key in AK::get().iter().filter(|key| !is_reserved(key)) {
        if NF::system_attribute(group, Some(m), &key.encode()).is_some() {
            NF::clear_typed_attribute(group, m, key)?;
        }
    }
    Ok(())
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
