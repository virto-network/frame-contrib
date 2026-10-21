# Memberships

Traits to manage the memberships of groups, as non-fungible items, and an implementation over
a `nonfungibles_v2` collection system such as `pallet-nfts`:

- `GroupCollectionMemberships`: each group's memberships live in the group's own collection. The
  group account holds the group's stock, members hold assigned memberships, and one manager
  account owns every collection. Membership items are locked against transfers and burns, so only
  the memberships manager moves them; members hand a membership on only as the group's transfer
  policy (`Transfer`, `TransferPolicy`) allows. A retired membership leaves the stock for a
  retirement holder (`Issue::retire`), which is never a member, until it is burnt (`Issue::burn`,
  which refuses a membership assigned to a member). It keeps an index from each membership to its
  group, written at issuance and removed at burn (`Issue::index` fills it in for memberships
  minted otherwise), so finding the group of a membership an account holds takes two reads,
  whatever else the account holds.

  `Attributes::set_membership_attribute` writes only the keys the runtime allows (the
  `AttributeKeys` type parameter; none by default). A release and a burn clear those keys and the
  membership's rank, so nothing written for a holder outlives their membership; a transfer keeps
  them. `Inspect::user_memberships_bounded` enumerates an account's memberships reading at most a
  given number of items, skipped ones (a group account's own stock) included, and says when it
  reached that bound.
