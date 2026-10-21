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
| The **capacity origin** | `CapacityOrigin` | Declare or schedule the capacity the allowance sold is capped against |
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

## Sales capacity

A deployment can cap the allowance it sells against the execution it can actually serve. The cap is opt-in: with no
capacity set (the default), nothing is refused for capacity, and the pallet only keeps the load (`REQ-CP-1`–`REQ-CP-6`).
The pallet knows a declared capacity and its clock, nothing more: it acquires, releases and schedules no capacity of
its own.

**The committed load** (`CommittedLoad`) is the sum, over the contracts that count, of each one's allowance
normalized to `CapacityPeriod`: `⌈allowance × CapacityPeriod ÷ usage period⌉`, in `ref_time` and proof size apart.
With a capacity period of a week, the figures read as "so much execution a week", and 2 520 contracts of one minute a
week commit what 42 contracts of one hour a week do: the measure is the allowance, never a count of contracts. A
contract counts from its start, trials included, until it ends or defaults: a *Defaulted* contract releases its share
when it defaults, and nothing again when it ends. The load is a rate: a burst within one usage window is bounded by
that window's allowance, and each block by the block limits.

The load is updated where a contract is written, in O(1) with no loop: a start adds, an end or a default releases, a
switch or conversion replaces the old contract's share with the new one's. An amendment that raises an allowance
counts at once (for an offer, the increase times the offer's contracts that count, which the pallet keeps in
`ContractsOnOffer`; a subscription to the offer's item made outside the pallet is no contract, and adds nothing),
while each contract moves the increase into its own share when it applies the amendment. An amendment that lowers an
allowance counts for a contract only once the lower allowance is in force for it, at the contract's next charge (or
amendment) at or after the first usage window of the lower allowance, so the load never understates what can be
drawn. A sale that would overflow the load in either component is refused (`CapacityExceeded`), with or without a
capacity, so each release subtracts exactly what its contract added.

**The capacity** (`Capacity`) is declared by `CapacityOrigin` with `set_capacity(capacity, from)`, per component.
`None` removes the cap. With `from` `None`, it applies now, and a pending change stays pending; with a moment in the
pallet's clock, it is scheduled, and takes effect at the first read at or after that moment. At most one change is
scheduled: a new schedule replaces the pending one, and one from a moment already reached replaces it and applies at
once. To drop a pending change, pass `from` = now. A capacity below the committed load in either component, now or
scheduled, is refused (`CapacityBelowLoad`): releasing capacity needs the load to fit first, by withdrawing offers or
amending allowances down with notice. A capacity with a zero component stops all sales in that component, since every
allowance is non-zero in both, and the alert reads above the mark meanwhile.

**A sale** is anything that raises the committed load: a subscription, a switch to a larger allowance, a trial's
conversion, and an amendment that raises an allowance. It must fit the **binding capacity** in both components: the
current capacity, or, per component, the lower of it and a scheduled change below it. A scheduled increase does not
count before its moment, as a contract sold now runs now. A sale that does not fit is refused with
`CapacityExceeded`, and nothing changes. A switch or conversion is checked when it is requested, and again at its
boundary, where a refusal drops it with the reason.

**The alert.** When a load update or a capacity change takes the load, in either component, to at least
`CapacityAlertThreshold` of the binding capacity (the one sales are checked against, so the alert comes before sales
stop), the pallet emits `CapacityThresholdCrossed { load, capacity }` with that capacity, once: not again while the
load stays at or above the mark, and again only after it has fallen below. The `capacity()` view
function returns the load, the capacity and the scheduled change in one read.

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
offer and anchor), the usage window and the estimate. Preparation checks again and must find the same ticket, or the
transaction is invalid (`PATH_MISMATCH`). After dispatch, the pool of the ticket's contract is charged the transaction's
**actual** metered weight, capped at the estimate, in the ticket's window, whatever the call changed (`INV-3`,
`INV-13`), and `UsageCharged` is the transaction's only event. The single exception is a transaction during whose own
dispatch the collective terminates its contract: that transaction is charged nothing (`INV-6` as amended by
`0009-A19.4`). A pool-path transaction costs its signer nothing, tip included, and gets the default priority. Every
other transaction takes the **fee path**: `S` validates, prepares and charges it exactly as it would alone (`INV-12`).

The **metered weight** is the weight the transaction consumed as the payment step sees it, counted as `CheckWeight`
counts it: the call's and every extension's declared weight, plus the base extrinsic weight of its class, plus its
length as proof size. The estimate uses the declared weights; the actual, the weights left after refunds. It is taken
before any storage-weight reclaim outside the payment step (`cumulus_pallet_weight_reclaim::StorageWeightReclaim`), so
it is never less than what the block finally books for the transaction (`REQ-PL-6`).

### Paying groups

A member of several groups chooses which pays with `set_paying_group(Some(group))`, and clears its choice with `None`.
Admission resolves the paying group as: the named group, if the member still holds a valid membership of it; otherwise
the only group among the member's memberships, if finding them reads at most `MaxMembershipScan` items (skipped ones,
such as a group account's own stock, included); otherwise none, and the fee path (`REQ-PC-2`, `REQ-PC-3`). A name that
became invalid is ignored, never deleted. Naming a group the member holds no valid membership of is refused
(`NotAMember`). Naming or clearing is feeless while the member holds a valid membership of the group it names (or had
named) and has made fewer than `MaxPayingGroupChanges` changes in the current rate window of `MinUsagePeriod` ticks,
counted from tick 0 (`REQ-PC-5`).

## The payment step: integration guide

This section is for a runtime integrator wiring `ChargeUsageSubscription<T, S>` (`DEC-24`). Every pitfall at its end
is kept out by a test in this crate.

### What it is

A transaction extension that wraps the runtime's fee extension `S` (for example
`pallet_asset_tx_payment::ChargeAssetTxPayment` or `pallet_transaction_payment::ChargeTransactionPayment`). For each
signed transaction it runs the pool meter's check: a ticket means the **pool path**, and `S` never runs; nothing means
the **fee path**, and `S` validates, prepares and charges exactly as it would alone (`INV-12`). Unsigned, root and
other non-signed origins always take the fee path.

It is invisible to clients: its identifier, implicit data, type information, metadata and encoding are `S`'s.
Replacing `S` with `ChargeUsageSubscription<Runtime, S>` changes neither the runtime metadata nor any transaction's
bytes.

### The check and the charge

| Phase | The extension | The pool meter |
|---|---|---|
| Validation (in the pool, and again in the block) | Computes the estimate and runs the check. A ticket: `Val = Path::Pool(ticket)`, `ValidTransaction::default()` (no priority above a zero-tip fee-path transaction, `REQ-PL-10`), and `S` is not asked. Nothing: `S` validates, and its value is carried as `Path::Fee(v)` | `check(who, estimate)`: read only, and the same answer for the same arguments in the same state (`CTR-FEE-1`) |
| Preparation | `Path::Fee(v)`: `S` prepares with `v`, and the pool is not asked again. `Path::Pool(t)`: the check runs again and must return the same ticket; otherwise the transaction is invalid with `InvalidTransaction::Custom(PATH_MISMATCH)`, and is never moved to a fee path validation did not check (`CTR-FEE-4`) | `check` again |
| Post-dispatch | The extension's own unspent weight, what the path taken left of its declared weight (`CTR-FEE-5`), is refunded first, on a copy of the post-dispatch information. `Path::Pool(t)`: unless the transaction pays no fee, the actual metered weight, from that copy, is charged and `UsageCharged` deposited; the unspent weight is `declared − pool_path`. `Path::Fee(p)`: `S`'s post-dispatch, with that copy, so the signer pays only for what the fee path used (`REQ-PL-11`, `INV-12`); the unspent weight is `declared − check − S::weight(call)`, where `check` is the weight of the check up to where it stopped (below), plus what `S` reports of its own | `charge(ticket, actual)`: one write, never fails, charges `min(actual, estimate)` in the ticket's window (`CTR-FEE-2`) |

### The ticket

Admission's answer is a ticket: the member, its paying group, its membership, the contract (its offer and anchor),
the usage window and the estimate. It is opaque outside this crate. Post-dispatch charges the pool the ticket names,
in the window it names, whatever the call did meanwhile: a call that releases or transfers the signer's membership,
changes its paying group, or cancels the contract is still charged to the group that admitted it (`INV-13`). If the
contract's record is gone by then (the collective terminated it during dispatch), or the group's contract is no
longer the one the ticket names, nothing is charged: there is no pool of that contract left to charge.

### Placement in `TransactionExtensions`

Put it where the fee extension would be, wrapping it:

- **After `CheckWeight`**, so the block has booked the transaction before the pool is asked, and the estimate is the
  weight `CheckWeight` books.
- **Inside `SkipCheckIfFeeless`**, which is required (`REQ-PC-5`), exactly as the fee extension would be: a feeless call
  (such as a free `set_paying_group`) then skips both paths, and touches no pool. `#[pallet::feeless_if]` does nothing
  without it: a free change would be charged to a pool or to the signer, and a member of several groups with no balance
  could never name one.
- **Around the fee extension**, which it replaces in the tuple.

```rust,ignore
pub type TxExtension = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_skip_feeless_payment::SkipCheckIfFeeless<
        Runtime,
        fc_pallet_usage_subscription::ChargeUsageSubscription<
            Runtime,
            pallet_asset_tx_payment::ChargeAssetTxPayment<Runtime>,
        >,
    >,
    frame_system::WeightReclaim<Runtime>,
);
```

Two consequences of this order:

- **`CheckNonce` needs an account.** It runs before the payment step, and refuses a signer with neither a provider
  nor a sufficient reference (`InvalidTransaction::Payment`), whatever its pool. Holding a membership gives no
  reference (an nfts item adds none), so a member that transacts on its pool with no balance at all, or names its
  paying group for free, must have one some other way. Pass accounts do: `fc-pallet-pass` adds a provider when it
  registers one.
- **The pool is charged before any later reclaim.** The charge is taken in the payment step's post-dispatch, from
  the weight reported then. An extension that lowers it afterwards, such as a parachain's storage-weight reclaim
  measuring the proof size actually used, does so after the pool was charged: the pool pays the benchmarked proof
  size, a small overcharge that never exceeds the estimate, so the allowance still holds (`INV-4`) and nothing is
  reported as unspent (`INV-11`). A runtime that wants the refund reflected places the payment step after the
  extension that makes it, where that extension's shape allows. `frame_system::WeightReclaim` only re-books the
  block's weight from the same report, so its place changes no charge.

### Declared weight and benchmarks

The extension declares `max(pool_path, fee_path_check + S::weight(call))` (`CTR-FEE-6`): the worst of the pool path
(the check in validation and in preparation, and the charge) and the fee path (the check, then `S`). Both come from
this pallet's `WeightInfo`; `S`'s weight from its own benchmark. A transaction pays only for the path it took: what
the other path would have used beyond it is the extension's unspent weight, refunded before anything is charged. On
the fee path, the delta over `S` alone is what usage subscriptions add to a fee (`NFR-2`).

On the fee path, the check keeps only its weight up to where it stopped, and the rest of `fee_path_check` is refunded:
`fee_path_no_paying_group` for a signer holding no membership at all; `fee_path_no_contract` for a signer whose paying
group is found, when it holds no membership of it, the group is not usable, or the group has no contract;
`fee_path_check` for any other reason. A non-signed origin is not checked, and keeps none. The declared weight stays the
worst case.

The `pool_path` and `fee_path_check` benchmarks run the worst case of admission (PLAN §7): a member whose named
paying group is stale (so admission reads the name, looks for a membership of it, and then scans) and who holds
`MaxMembershipScan` memberships of one usable group, an *Active* standard contract whose offer has an amendment, and a
pool at its limit minus the estimate (`pool_path`) or at its limit (`fee_path_check`, which then reads everything and
fails at the last condition). The `fee_path_no_paying_group` and `fee_path_no_contract` benchmarks run the check's
early exits at their own worst cases: a stale name and no membership; a stale name, the full scan of one usable group,
and no contract. The runtime's `BenchmarkHelper` provides the group, its members, a price, and a second group for the
stale name.

### What is metered

A pool is charged the weight the payment step sees the transaction consume, counted as `CheckWeight` counts it
(`frame_system::calculate_consumed_extrinsic_weight`). It is taken before any storage-weight reclaim outside the payment
step (`cumulus_pallet_weight_reclaim::StorageWeightReclaim`), so it is never less than what the block finally books
for the transaction (`REQ-PL-6`):

- **Estimate**, what the pool must cover: the declared call weight, plus every transaction extension's declared
  weight, plus the base extrinsic weight of the call's class, with the encoded length added to proof size. Both
  components must fit, or the fee path is taken; a pool is never partly applied (`REQ-PL-12`).
- **Actual**, what the pool is charged: the call and extension weights left after dispatch and refunds
  (`calc_actual_weight`), plus the same base and length, capped at the estimate, in each component.
- **Nothing**, when the transaction pays no fee (the call is declared `Pays::No`, or reports it after dispatch).

### Naming a paying group for free

`set_paying_group` carries `#[pallet::feeless_if]`: it is feeless when the signer holds a valid membership of the group
it names (or, clearing, of the group it had named) and has made fewer than `MaxPayingGroupChanges` changes in the
current rate window. The runtime's `SkipCheckIfFeeless` then skips the payment step entirely, so a member with no
balance can name its group, provided `CheckNonce` admits it: it must have a provider or a sufficient reference, as
above. Beyond the limit it is an ordinary transaction. A member never names a group through a transaction extension's
data.

### Pitfalls

The defects an earlier gas-tank payment step shipped with in production, each kept out of this one by a test in this crate:

1. **Writes in validation.** A write in a check runs in the pool and again in the block, and is how that
   step locked members out. Admission reads only; the window is computed, never reset by a write.
2. **`expect` in preparation.** Preparing the fee extension with a value validation never produced panicked. A
   changed answer is now `PATH_MISMATCH`, and the fee path is prepared only with its own validated value.
3. **A window reset into the future.** The window start is computed from the anchor, and always contains now.
4. **The call weight reported as unspent.** Only the extension's own unspent weight is reported: the part of its
   declared weight the path taken did not use, plus, on the fee path, what `S` reports of its own. It is refunded before
   the pool or `S` charges, so neither the pool nor the signer pays for it. The call's weight stays booked.
5. **Admission and charge measuring different things.** Both are the metered weight above; the charge is the actual,
   never the estimate.
6. **An undeclared inner weight.** The declared weight covers the fee path too.
7. **An unbounded scan.** Admission reads at most `MaxMembershipScan + 1` items to find a member's memberships, and
   beyond that takes the fee path. The bound counts the items read, not only the memberships returned
   (`Inspect::user_memberships_bounded`): a signing group account whose own stock `GroupCollectionMemberships` skips
   takes the fee path once that stock reaches the bound, instead of reading all of it.
8. **A charge decided after dispatch.** Who pays must not be looked up again after the call has run, or a call that
   changes it escapes its charge (the gas tank now charges the tank it noted before dispatch, in O(1)). Here the
   ticket decides who pays (`INV-13`): the group admission chose is the group charged.

## Configuration

| Item | What it is | Constraint |
|---|---|---|
| `WeightInfo` | The weights of the calls | Benchmarked on the deployment's reference hardware |
| `Memberships` | Group memberships: `fc_traits_memberships::Inspect` and `Transfer` (read only) | A group's account is never its own member |
| `GroupAccount` | The account of each group, which pays its charges | A conversion: no storage read. Different groups, different accounts |
| `UsableGroup` | The groups that may subscribe and draw on a pool | At most one storage read. Never the collective's own group |
| `Subscriptions` | Recurring subscriptions over listings items (e.g. `fc-pallet-listings`) | Its hooks notify this pallet. Only this pallet subscribes to, publishes in or administers `OfferInventory` |
| `OfferInventory` | The collective's inventory, where every offer is an item | Not used by anything else |
| `Payee` | The configured payee, owner of the inventory, recipient of every charge | Holds the catalog deposits of every offer |
| `StandardOfferOrigin` | Publishes and withdraws standard offers | The collective's administrative origin |
| `CustomOfferOrigin` | Publishes and withdraws custom offers | The outcome of a collective referendum |
| `TerminateOrigin` | Terminates contracts | The collective's |
| `AmendOrigin` | Amends custom contracts and standard offers, and nothing else | A collective referendum whose shortest path to enactment is at least the deployment's minimum notice, distinct from the origins above (`REQ-OF-8`) |
| `GroupOrigin` | A group's administrative origin, resolving to the group | — |
| `BlockNumberProvider` | The chain clock | The same clock as `Subscriptions` |
| `MinUsagePeriod` | The shortest usage period | Non-zero |
| `MinBillingPeriod` | The shortest billing period | Non-zero, longer than the subscriptions system's lead |
| `MaxTrialPeriods` | The longest trial, in billing periods | Non-zero |
| `MaxMembershipScan` | The most items admission reads to find a member's only group, skipped ones included | Small: every signed transaction pays for the reads |
| `MaxPayingGroupChanges` | The free paying-group changes per rate window (`MinUsagePeriod` ticks) | — |
| `MaxOffersPerPage` | The most offers one page of `open_offers` examines | Non-zero |
| `CapacityOrigin` | Declares or schedules the capacity | The deployment's governance of its execution capacity |
| `CapacityPeriod` | The period every allowance is normalized to for the committed load and the capacity, in ticks | Non-zero. For example a week |
| `CapacityAlertThreshold` | The share of the capacity at which `CapacityThresholdCrossed` is emitted | A `Perbill`, for example 80 % |
| `BenchmarkHelper` | With `runtime-benchmarks`: a usable group, its administrative origin, its members, a price, another group for a stale paying-group name, how many subscriptions fill the due queue's buckets (0 without such a queue), whether its overflow is empty, and funds for the payee | The group's account can pay many billing periods |

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
| 10 | `set_capacity(capacity, from)` | `CapacityOrigin` | Declares the capacity now, or schedules it from a moment |

Due charges are not a call of this pallet: the subscriptions system processes them, and anyone may trigger one there.

## Events

One per state change (`CTR-EVT-1`): `OfferPublished`, `OfferWithdrawn` (also when a custom offer is accepted),
`ContractStarted`, `ContractCharged`, `ContractSuspended`, `ContractRestored`, `ContractDefaulted`,
`CancellationRequested`, `ContractEnded` (with its `EndReason`), `SwitchScheduled`, `SwitchCancelled`,
`SwitchDropped` (with the reason), `ConversionScheduled`, `ConversionCancelled`, `ConversionDropped` (with the
reason), `OfferAmended` (with its last boundary), `ContractAmended` (with its effective boundary),
`ContractAmendmentInForce`, `PayingGroupSet`, `CapacitySet`, `CapacityScheduled`, `CapacityThresholdCrossed`; and
`UsageCharged`, a pool-path transaction's only event (`CTR-EVT-2`).

## Errors

`InvalidTerms`, `UnknownOffer`, `OfferWithdrawn`, `NotEligible`, `AlreadyContracted`, `ChargeFailed`, `NoContract`,
`SwitchPending`, `ChangePending`, `NoPendingSwitch`, `InvalidConversion`, `GroupUnusable`, `TrialUsed`, `NotAMember`,
`CapacityExceeded`, `CapacityBelowLoad`, and `BadOrigin`: SPEC §10's names (`CTR-CALL-2`). The subscriptions system's refusals are mapped to them. In the
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
| `capacity()` | The committed load, the capacity now (`None`: no cap) and the pending scheduled change |

## Wiring it

1. **Group bindings.** Bind `Memberships` to the runtime's group memberships (for example
   `fc_traits_memberships::GroupCollectionMemberships`), `GroupAccount` to the conversion that names a group's account
   (for communities, `fc_pallet_communities::CommunityAccount`), and `UsableGroup` to a one-read check that refuses the
   collective's own group.
2. **Listings.** Bind `Subscriptions` to `fc-pallet-listings`, and set its `OnSubscriptionChanged` to this pallet
   (in a tuple, if there are other dependants). Listings adds this pallet's `max_hook_weight`, the benchmarked
   worst case of the hooks one subscription can trigger, to every call and due-queue entry that can notify, so its
   weights cover the contract bookkeeping. Close direct subscription to `OfferInventory` in its
   `SubscribeOrigin`, so contracts are made only here. Use the same `BlockNumberProvider` in both.
3. **Payments.** Listings charges each billing period as a direct payment through its `Payments`
   (`fc_traits_payments::DirectPayment`, implemented by `fc-pallet-payments`). The payments system's fee policy should
   take no fee from either side when the beneficiary is the configured payee (`REQ-BL-3`).
4. **Memberships.** Each group's memberships are issued and managed outside this pallet; it only reads them.
5. **The payment step.** Put `ChargeUsageSubscription<Runtime, S>` in place of the fee extension `S`, after
   `CheckWeight`, inside `pallet_skip_feeless_payment::SkipCheckIfFeeless`, which is required: `set_paying_group` is
   feeless only through it (`REQ-PC-5`). Give every member account a provider or a sufficient reference (pass accounts
   have one): `CheckNonce` refuses a signer with neither before the payment step is asked, and a membership gives none.
6. **Sales capacity.** Bind `CapacityOrigin` to the origin that governs the deployment's execution capacity,
   `CapacityPeriod` to the period operators reason in (for example a week, in ticks of the pallet's clock), and
   `CapacityAlertThreshold` to the share at which they want the alert. Nothing is capped until that origin calls
   `set_capacity`.

## Community fees (the `fees` feature)

`PayingGroupCommunity<T>` implements `fc_pallet_fees::AccountCommunity` over this pallet, so `fc-pallet-fees` applies
a group's fees to the asset transfers of its members. An account's community is its **resolved paying group**
(`REQ-PC-2`): the group it named and still belongs to, or else the only group it belongs to; with no membership, or
several and none named, it has none. It is the resolution the `paying_group` view and the payment step make, with
the same bounded reads (`REQ-PC-3`).

The adapter is behind the optional cargo feature `fees`, which adds `fc-pallet-fees` as a dependency; without it the
crate does not depend on that pallet. A runtime whose fees pallet has a `CommunityId` equal to the group type binds
it there:

```rust,ignore
type CommunityDetector = fc_pallet_usage_subscription::PayingGroupCommunity<Runtime>;
```
