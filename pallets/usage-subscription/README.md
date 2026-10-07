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
| The **collective** | Its referendum origins: `StandardOfferOrigin`, `CustomOfferOrigin`, `TerminateOrigin`, and the dedicated `AmendOrigin` | Publish and withdraw offers, agree custom terms with one group, amend a custom contract or a standard offer, terminate a contract |
| A **group** | Its administrative origin, `GroupOrigin` | Subscribe (optionally naming a trial's conversion), cancel, switch offer, cancel a switch or conversion |
| A **member** | A signed transaction | Draw on its group's pool; name its paying group (`set_paying_group`) |
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
| price | An asset and an amount: at least the asset's minimum, and non-zero except for a trial |
| billing period | At least `MinBillingPeriod`, and longer than the subscriptions system's lead |
| term limit, minimum commitment | Standard: none. Trial: a term limit of 1 to `MaxTrialPeriods` periods, no commitment. Custom: optional, the commitment at most the term limit |
| grace | Shorter than the billing period |

Offers come in kinds:

- **Standard**: open-ended, with no commitment, for any usable group.
- **Trial**: a few billing periods, free or discounted, that a group may start once, ever, across all trials
  (`TrialUsed`). It never renews.
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
| *Active* (trial) | *Ended* (*converted*) | At the trial's end, a conversion is pending, its target is open and eligible, and the new contract's first charge succeeds |
| *Active*, *Suspended* | (same) | An amendment is enacted: it is pending until the contract's effective boundary, and the free exit is open |
| *Active* | *Ended* (*cancelled*) | The group cancelled while an amendment was pending: at `paid through`, the commitment waived |
| any | *Ended* (*terminated*) | The collective terminates it: at once, nothing refunded |

An ended contract's record is removed in the same block, and its event names the reason (`REQ-CT-12`). `subscribe`
first settles the group's old contract, so a transition already due takes effect even before the subscriptions
system's due queue reaches it: a *Suspended* contract past its grace end and outside its commitment, or a *Defaulted*
one past its commitment end, ends (*lapsed*) and no longer blocks.

A switch (`switch_offer`) takes effect at the first billing boundary at or after both `paid through` and the
commitment end, and only if the new contract's first charge succeeds then; otherwise it is dropped and the old
contract goes on (`REQ-CT-7`). It is also dropped if, by then, the target was withdrawn or the group is no longer
usable or eligible for it, and the event says why. A group may cancel a pending switch (`cancel_switch`).

### Trials and conversions

A group subscribing to a trial may name, in `subscribe`, an offer the trial **converts** into: an open offer, not a
trial, that the group is eligible for (otherwise `InvalidConversion`, and nothing is created). The conversion is the
trial's pending switch. At the trial's end, if the target is still open and the group still usable and eligible, the
target's first charge is attempted, and on success a contract to it starts there, with the target's terms as they
stand then (*converted*). Otherwise the trial completes, and the event says why. A trial with no conversion never
converts, and a switch scheduled during a trial also waits for its end (`REQ-CT-9`, `REQ-CT-13`).

### Amendments, notice and the free exit

The collective may amend, through `AmendOrigin` only, either one *Active* or *Suspended* **custom** contract
(`amend_contract`), or an open **standard** offer together with every contract made from it (`amend_offer`). No
group is asked. An amendment may change the allowance, price, grace, minimum commitment and term limit, keeps the
kind's rules, and never changes the usage or billing period (`InvalidTerms` otherwise). For each contract it takes
effect at that contract's **effective boundary** `b`: the first billing boundary at least one full billing period
after the enactment (`REQ-CT-14`). The new price is charged for the billing period starting at `b`, never earlier;
the new allowance applies from the first usage window starting at or after `b` (`INV-20`). The anchor never moves.

Until `b`, the group has a **free exit**: cancelling ends the contract at its `paid through` (at once, if
*Suspended*) with any minimum commitment waived. An amendment drops a pending switch, and while one is pending no
switch and no second amendment may be made (`ChangePending`).

An offer amendment is lazy (`DEC-34`): the offer's terms change at once for new contracts, its amendment is recorded
once (`OfferAmendment`), and each contract made from it before takes it at its own boundary, as its billing reaches
it. Nothing loops over contracts. A second amendment of the offer waits until every contract made from it before the
first has applied it (or ended), as listings counts them, and until the allowance it brought is in force for each of
them; a custom contract is not amended again while the allowance of its last amendment is not yet in force. So a
contract never holds more than one allowance besides its current one (`DEC-3`). Every offer's listings item lets the
collective amend with one billing period of notice, and terminate (its subscription policy).

## Usage windows and billing periods

The two clocks are independent (`REQ-BL-2`). Billing period *n* covers `[anchor + n·B, anchor + (n+1)·B)` and is
charged at its start. Usage window *k* covers `[anchor + k·U, anchor + (k+1)·U)`. A contract stores one window start
and the weight used in it; the current window is computed from the anchor and the clock, and the stored usage counts
only if it belongs to that window. Nothing is reset by a write, and a boundary passing needs no processing (`DEC-3`).

## Admission, metering and charging

The payment step, `ChargeUsageSubscription<T, S>`, wraps the runtime's fee extension `S`. For each signed
transaction it asks the pool meter whether the transaction takes the **pool path**. It does if, and only if, all of
these hold when it is validated (`REQ-PL-7`):

1. its origin is a signed account A;
2. a paying group P is resolved for A (see below);
3. A holds a valid membership of P (one of P's memberships, A not being P's group account);
4. P is usable (`UsableGroup`);
5. P has an *Active* contract, and now is before its `paid through`;
6. in both components, the current usage window's usage plus the transaction's **estimate** is at most the
   allowance. A pool is never partly applied: a transaction it cannot cover takes the fee path.

Admission writes nothing (`INV-1`) and yields a **ticket**: the member, the group, the membership, the contract (its
offer and anchor), the usage window and the estimate. Preparation checks again and must find the same ticket, or the transaction is invalid
(`PATH_MISMATCH`). After dispatch, the pool of the ticket's contract is charged the transaction's **actual** metered
weight, capped at the estimate, in the ticket's window, whatever the call changed (`INV-3`, `INV-13`), and
`UsageCharged` is the transaction's only event. The single exception is a transaction during whose own dispatch the collective terminates its contract: that
transaction is charged nothing (`INV-6` as amended by `0009-A19.4`). A pool-path transaction costs its signer nothing, tip included, and
gets the default priority. Every other transaction takes the **fee path**: `S` validates, prepares and charges it
exactly as it would alone (`INV-12`).

The **metered weight** is what the block books for the transaction, as `CheckWeight` counts it: the call's and every
extension's declared weight, plus the base extrinsic weight of its class, plus its length as proof size. The estimate
uses the declared weights; the actual, the weights left after refunds.

### Paying groups

A member of several groups chooses which pays with `set_paying_group(Some(group))`, and clears its choice with
`None`. Admission resolves the paying group as: the named group, if the member still holds a valid membership of it;
otherwise the only group among the member's memberships, reading at most `MaxMembershipScan` of them; otherwise none,
and the fee path (`REQ-PC-2`, `REQ-PC-3`). A name that became invalid is ignored, never deleted. Naming a group the
member holds no valid membership of is refused (`NotAMember`). Naming or clearing is feeless while the member holds a
valid membership of the group it names (or had named) and has made fewer than `MaxPayingGroupChanges` changes in the
current rate window of `PayingGroupChangeWindow` ticks, counted from tick 0 (`REQ-PC-5`).

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
| `AmendOrigin` | Amends custom contracts and standard offers, and nothing else | A collective referendum whose shortest path to enactment is at least the deployment's minimum notice, distinct from the origins above (`REQ-OF-8`) |
| `GroupOrigin` | A group's administrative origin, resolving to the group | — |
| `BlockNumberProvider` | The chain clock | The same clock as `Subscriptions` |
| `MinUsagePeriod` | The shortest usage period | Non-zero |
| `MinBillingPeriod` | The shortest billing period | Non-zero, longer than the subscriptions system's lead |
| `MaxTrialPeriods` | The longest trial, in billing periods | Non-zero |
| `MaxMembershipScan` | The most memberships admission reads to find a member's only group | Small: every signed transaction pays for the reads |
| `MaxPayingGroupChanges` | The free paying-group changes per rate window | — |
| `PayingGroupChangeWindow` | The rate window of paying-group changes, in ticks | Non-zero; the deployment's minimum usage period |
| `MaxOffersPerPage` | The most offers one page of `open_offers` examines | Non-zero |

## Calls

| Index | Call | Origin | Does |
|---|---|---|---|
| 0 | `publish_offer(kind, terms)` | `StandardOfferOrigin` or, for a custom offer, `CustomOfferOrigin` | Publishes an offer |
| 1 | `withdraw_offer(offer)` | The origin that publishes its kind | Withdraws an open offer |
| 2 | `subscribe(offer, converts_into)` | `GroupOrigin` | Starts the group's contract, charging period 0 |
| 3 | `cancel()` | `GroupOrigin` | Cancels the group's contract, effective at the later of `paid through` and the commitment end |
| 4 | `switch_offer(offer)` | `GroupOrigin` | Schedules a switch of the group's *Active* contract |
| 5 | `terminate_contract(group)` | `TerminateOrigin` | Ends a contract at once |
| 6 | `set_paying_group(group)` | Signed | Names or clears the signer's paying group; feeless within the rate limit |
| 7 | `amend_contract(group, terms)` | `AmendOrigin` | Amends a custom contract, with notice |
| 8 | `cancel_switch()` | `GroupOrigin` | Drops the group's pending switch or conversion |
| 9 | `amend_offer(offer, terms)` | `AmendOrigin` | Amends a standard offer and its contracts, with notice |

Due charges are not a call of this pallet: the subscriptions system processes them, and anyone may trigger one there.

## Events

One per state change (`CTR-EVT-1`): `OfferPublished`, `OfferWithdrawn` (also when a custom offer is accepted),
`ContractStarted`, `ContractCharged`, `ContractSuspended`, `ContractRestored`, `ContractDefaulted`,
`CancellationRequested`, `ContractEnded` (with its `EndReason`), `SwitchScheduled`, `SwitchCancelled`,
`SwitchDropped` (with the reason), `ConversionScheduled`, `ConversionCancelled`, `ConversionDropped` (with the
reason), `OfferAmended` (with its last boundary), `ContractAmended` (with its effective boundary),
`ContractAmendmentInForce`, `PayingGroupSet`; and `UsageCharged`, a pool-path transaction's only event
(`CTR-EVT-2`).

## Errors

`InvalidTerms`, `UnknownOffer`, `OfferWithdrawn`, `NotEligible`, `AlreadyContracted`, `ChargeFailed`, `NoContract`,
`SwitchPending`, `ChangePending`, `NoPendingSwitch`, `InvalidConversion`, `GroupUnusable`, `TrialUsed`, `NotAMember`,
and `BadOrigin`: SPEC §10's names (`CTR-CALL-2`). The subscriptions system's refusals are mapped to them. In the
payment step, a preparation that disagrees with validation is `InvalidTransaction::Custom(PATH_MISMATCH)`
(`ERR-PathMismatch`); a failing fee path is the fee extension's own error (`ERR-Payment`).

## View functions

Every query of `CTR-QRY-1` is a view function, readable without a transaction; each answers from one state and
writes nothing (`REQ-OB-1`):

| View function | Answers |
|---|---|
| `offer(offer)` | Its kind, terms, status, and its last amendment while it may be pending for some contract |
| `open_offers(start_after, limit)` | Open offers in ascending id order. A page examines at most `min(limit, MaxOffersPerPage)` offers, skipping withdrawn ones; its `next` is the cursor for the following page, `None` when there are no more. A zero limit examines nothing and gives `start_after` back |
| `contract(group)` | State, kind, terms, anchor, `paid through`, next due, grace end, commitment end, periods charged, and any pending cancellation, switch, conversion or amendment (with its effective boundary and whether the free exit is open) |
| `pool(group)` | The allowance, the current window's start and end, usage, remainder, and whether it is usable now, or why not |
| `paying_group(account)` | The resolved paying group, or none, with the reason |
| `trial_used(group)` | Whether the group has started a trial |
| `transfer_policy(group)` | The group's transfer policy, as its memberships manager keeps it |
| `would_waive(account, estimate)` | The pool path with the remainder, or the fee path with the first condition of admission that fails |

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
5. **The payment step.** Put `ChargeUsageSubscription<Runtime, S>` in place of the fee extension `S`, after
   `CheckWeight`, inside `SkipCheckIfFeeless` if the runtime uses it (so `set_paying_group` can be feeless). Give
   every member account a provider or a sufficient reference (pass accounts have one): `CheckNonce` refuses a signer
   with neither before the payment step is asked, and a membership gives none.
