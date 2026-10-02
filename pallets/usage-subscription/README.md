# Usage subscriptions

A **usage subscription** is a recurring **usage contract** between a **group** and the **collective**. The contract
gives the group a **pool**: an **allowance** of weight (`ref_time` and proof size) per **usage period**, shared by
every member of the group, for a price per **billing period**, always paid in advance. While the group is within its
allowance and paid up, its members' transactions are free and their weight is charged to the pool; otherwise members
pay their own fees, as they would without this pallet.

This crate is the contract registry and pool meter of usage subscriptions. Its normative specification is
[`SPEC.md`](SPEC.md), beside this file; the technical decisions it implements (`DEC-*`) are in
[`PLAN.md`](PLAN.md), the frame-contrib edition of the plan. Identifiers such as `REQ-CT-7` or `INV-8` below point
into them.

## Parties

| Party | Acts through | May |
|---|---|---|
| The **collective** | Its referendum origins: `StandardOfferOrigin`, `CustomOfferOrigin`, `TerminateOrigin` | Publish and withdraw offers, agree custom terms with one group, terminate a contract |
| A **group** | Its administrative origin, `GroupOrigin` | Subscribe, cancel, switch offer, cancel a switch |
| A **member** | A signed transaction | Draw on its group's pool |
| The **configured payee** | Passive | Receive every charge |
| **Anyone** | A signed transaction | Trigger a charge that is due (in the subscriptions system) |

The pallet is generic over the group: it knows groups only through the memberships traits
(`fc_traits_memberships::Inspect`), the account of each group (`GroupAccount`) and whether a group is usable
(`UsableGroup`). On Kreivo a group is a community.

## Offers

An **offer** is a set of terms the collective makes available:

| Term | Rule |
|---|---|
| allowance | Both components non-zero, per usage period |
| usage period | At least `MinUsagePeriod` |
| price | An asset and an amount: a non-zero amount, at least the asset's minimum |
| billing period | At least `MinBillingPeriod`, and longer than the subscriptions system's lead |
| term limit, minimum commitment | Standard: none. Custom: optional, the commitment at most the term limit |
| grace | Shorter than the billing period |

Offers come in kinds:

- **Standard**: open-ended, with no commitment, for any usable group.
- **Custom**: agreed with exactly one group through a referendum (`CustomOfferOrigin`), optionally with a minimum
  commitment and a term limit. Only its group may accept it, once; once accepted, it reads as withdrawn.

Terms that break a rule are refused with `InvalidTerms`. Withdrawing an offer stops new contracts from it and changes
none of the contracts made from it (`REQ-OF-3`).

Every offer is an item of the collective's inventory in the subscriptions system (`OfferInventory`), created on the
first offer and owned by the configured payee (`Payee`), so every charge is paid to it (`DEC-8`). The item's
subscription conditions carry the money and time terms; this pallet keeps the offer's kind and weight terms.

## Contracts and their lifecycle

A group has at most one contract at a time, in any state (`REQ-CT-1`, `INV-8`). A contract copies its offer's terms
when it starts (`INV-9`): its weight terms in this pallet's `Contracts`, its money and time terms in the subscription
of the group's account to the offer's item, which also holds its `paid through`, state and commitment. Recurring
billing (due charges, suspension, grace, lapse, default, commitments, replacements) is the subscriptions system's;
this pallet follows every transition through its `OnSubscriptionChanged` hooks, in the same block.

| From | To | When |
|---|---|---|
| — | *Active* | `subscribe`, and billing period 0 is charged. The anchor is now |
| *Active* | *Active* | A charge attempted within the lead, or at the due tick, succeeds |
| *Active* | *Suspended* | The charge at the due tick fails. The pool is unusable from the due tick |
| *Suspended* | *Active* | The charge succeeds within grace. The anchor does not move |
| *Suspended* | *Defaulted* | Grace elapses unpaid within the minimum commitment. It blocks the group until the commitment end |
| *Suspended* | *Ended* (*lapsed*) | Grace elapses unpaid outside the commitment |
| *Defaulted* | *Ended* (*lapsed*) | The commitment end arrives |
| *Active* | *Ended* (*completed*) | The term limit's last period has passed, and no switch takes over |
| *Active* | *Ended* (*cancelled*) | The group cancelled, at the later of `paid through` and the commitment end |
| *Suspended* | *Ended* (*cancelled*) | The group cancels outside its commitment: at once |
| *Active* | *Ended* (*switched*) | A switch is pending, at the first billing boundary past the commitment end, and the new contract's first charge succeeds |
| any | *Ended* (*terminated*) | The collective terminates it: at once, nothing refunded |

An ended contract's record is removed in the same block, and its event names the reason (`REQ-CT-12`). `subscribe`
first settles the group's old contract, so a transition already due takes effect even before the subscriptions
system's due queue reaches it: a *Suspended* contract past its grace end and outside its commitment, or a *Defaulted*
one past its commitment end, ends (*lapsed*) and no longer blocks.

A switch (`switch_offer`) takes effect at the first billing boundary at or after both `paid through` and the
commitment end, and only if the new contract's first charge succeeds then; otherwise it is dropped and the old
contract goes on (`REQ-CT-7`). It is also dropped if, by then, the target was withdrawn or the group is no longer
usable or eligible for it, and the event says why. A group may cancel a pending switch (`cancel_switch`).

## Usage windows and billing periods

The two clocks are independent (`REQ-BL-2`). Billing period *n* covers `[anchor + n·B, anchor + (n+1)·B)` and is
charged at its start. Usage window *k* covers `[anchor + k·U, anchor + (k+1)·U)`. A contract stores one window start
and the weight used in it; the current window is computed from the anchor and the clock, and the stored usage counts
only if it belongs to that window. Nothing is reset by a write, and a boundary passing needs no processing (`DEC-3`).

## Configuration

| Item | What it is | Constraint |
|---|---|---|
| `WeightInfo` | The weights of the calls | Benchmarked on the deployment's reference hardware |
| `Memberships` | Group memberships: `fc_traits_memberships::Inspect` and `Transfer` (read only) | A group's account is never its own member |
| `GroupAccount` | The account of each group, which pays its charges | A conversion: no storage read. Different groups, different accounts |
| `UsableGroup` | The groups that may subscribe and draw on a pool | At most one storage read. Never the collective's own group |
| `Subscriptions` | Recurring subscriptions over listings items (e.g. `fc-pallet-listings`) | Its hooks notify this pallet. Only this pallet subscribes to, publishes in or administers `OfferInventory` |
| `OfferInventory` | The collective's inventory, where every offer is an item | Not used by anything else |
| `Payee` | The configured payee, owner of the inventory, recipient of every charge | — |
| `StandardOfferOrigin` | Publishes and withdraws standard offers | The collective's administrative origin |
| `CustomOfferOrigin` | Publishes and withdraws custom offers | The outcome of a collective referendum |
| `TerminateOrigin` | Terminates contracts | The collective's |
| `GroupOrigin` | A group's administrative origin, resolving to the group | — |
| `BlockNumberProvider` | The chain clock | The same clock as `Subscriptions` |
| `MinUsagePeriod` | The shortest usage period | Non-zero |
| `MinBillingPeriod` | The shortest billing period | Non-zero, longer than the subscriptions system's lead |

## Calls

| Index | Call | Origin | Does |
|---|---|---|---|
| 0 | `publish_offer(kind, terms)` | `StandardOfferOrigin` or, for a custom offer, `CustomOfferOrigin` | Publishes an offer |
| 1 | `withdraw_offer(offer)` | The origin that publishes its kind | Withdraws an open offer |
| 2 | `subscribe(offer, converts_into)` | `GroupOrigin` | Starts the group's contract, charging period 0 |
| 3 | `cancel()` | `GroupOrigin` | Cancels the group's contract, effective at the later of `paid through` and the commitment end |
| 4 | `switch_offer(offer)` | `GroupOrigin` | Schedules a switch of the group's *Active* contract |
| 5 | `terminate_contract(group)` | `TerminateOrigin` | Ends a contract at once |
| 8 | `cancel_switch()` | `GroupOrigin` | Drops the group's pending switch |

Due charges are not a call of this pallet: the subscriptions system processes them, and anyone may trigger one there.

## Events

One per state change (`CTR-EVT-1`): `OfferPublished`, `OfferWithdrawn` (also when a custom offer is accepted),
`ContractStarted`, `ContractCharged`, `ContractSuspended`, `ContractRestored`, `ContractDefaulted`,
`CancellationRequested`, `ContractEnded` (with its `EndReason`), `SwitchScheduled`, `SwitchCancelled`,
`SwitchDropped` (with the reason).

## Errors

`InvalidTerms`, `UnknownOffer`, `OfferWithdrawn`, `NotEligible`, `AlreadyContracted`, `ChargeFailed`, `NoContract`,
`SwitchPending`, `ChangePending`, `NoPendingSwitch`, `InvalidConversion`, `GroupUnusable`, `TrialUsed`, `NotAMember`,
and `BadOrigin`: SPEC §10's names (`CTR-CALL-2`). The subscriptions system's refusals are mapped to them.

## Wiring it

1. **Group bindings.** Bind `Memberships` to the runtime's group memberships (for example
   `fc_traits_memberships::GroupCollectionMemberships`), `GroupAccount` to the conversion that names a group's account
   (for communities, `fc_pallet_communities::CommunityAccount`), and `UsableGroup` to a one-read check that refuses the
   collective's own group.
2. **Listings.** Bind `Subscriptions` to `fc-pallet-listings`, and set its `OnSubscriptionChanged` to this pallet
   (in a tuple, if there are other dependants). Close direct subscription to `OfferInventory` in its
   `SubscribeOrigin`, so contracts are made only here. Use the same `BlockNumberProvider` in both.
3. **Payments.** Listings charges each billing period as a direct payment through its `Payments`
   (`fc_traits_payments::DirectPayment`, implemented by `fc-pallet-payments`). The payments system's fee policy should
   take no fee from either side when the beneficiary is the configured payee (`REQ-BL-3`).
4. **Memberships.** Each group's memberships are issued and managed outside this pallet; it only reads them.
