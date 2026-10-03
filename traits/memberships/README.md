# Memberships

Traits to manage the memberships of groups, as non-fungible items, and two implementations over
a `nonfungibles_v2` collection system such as `pallet-nfts`:

- `GroupCollectionMemberships`: each group's memberships live in the group's own collection. The
  group account holds the group's stock, members hold assigned memberships, and one manager
  account owns every collection. Membership items are locked against transfers and burns, so only
  the memberships manager moves them; members hand a membership on only as the group's transfer
  policy (`Transfer`, `TransferPolicy`) allows. A retired membership leaves the stock for a
  retirement holder (`Issue::retire`), which is never a member, until it is burnt (`Issue::burn`,
  which refuses a membership assigned to a member).

  The manager clears only what it sets itself: a membership's rank, on release and burn.
  Attributes written through `Attributes::set_membership_attribute` are not cleared by a release,
  a transfer or a burn; whoever writes them clears them.
- `NonFungiblesMemberships` (deprecated since 2.4.0): a manager collection with a twin item per
  assigned membership.
