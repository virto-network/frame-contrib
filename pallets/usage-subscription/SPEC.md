---
name: Usage subscriptions
summary: A group subscribes to a recurring usage contract with the collective. The contract gives every member a shared, group-billed allowance of weight per usage period, priced per billing period in an asset and paid in advance. While the group is within its allowance and paid up, its members' transaction fees are waived and the weight is charged to the group's pool. Otherwise members pay their own fees. Memberships belong to their group, never expire, and each one is backed by a deposit.
spec_version: 0.4.0
status: draft
date: 2026-10-01
amendments: 0001-owner-review, 0002-owner-review-2, 0003-owner-review-3 (applied)
---

# Usage subscriptions — Specification

## 0. How to read this document

This document is normative only. It specifies **what** usage subscriptions do, for whom, and under what conditions,
for the target system as it must always behave. It contains no rationale, no history, no technical decisions and
no deployment values:

- **why** a rule exists is in `OVERVIEW.md`;
- **how** the system is built, every technical decision, and how the chain gets from today's state to this one are
  in `PLAN.md`;
- history, prior art and findings in code or live data are in `NOTES.md`;
- the values one deployment chooses are in its deployment document (`deployment/`).

**Scope is exactly what this document states.** Behaviour it does not state is out of scope. A gap or a conflict is
resolved by an amendment in `amendments/`, never by reading intent into this text or by looking for problems outside
it. Where this document and `PLAN.md` disagree, this document wins.

### What each section answers

| Section | Question it answers |
|---|---|
| §1 Purpose and scope | What the system is and does, and where the boundary is |
| §2 Glossary | What each normative term means. Terms are used exactly as defined |
| §3 Parties | Who acts, with which powers |
| §4 Structure | Which responsibilities exist, and how they relate |
| §5 Domain model | Groups, offers, contracts, pools, periods, memberships, and their rules |
| §6 User stories | The behaviour seen from outside, with acceptance criteria |
| §7 Surface contracts | The boundaries other components build against |
| §8 Normative behaviour | Every call, the transaction path, and every query |
| §9 Invariants | What is always true, and which layer keeps it true |
| §10 Errors | Every named refusal |
| §11 Non-functional requirements | Cost, boundedness, operability |
| §12–§13 Deferred scope, non-goals | What is out |
| §14 Open questions | What is undecided |
| §15 Traceability | How stories map to requirements |

### Normative language

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in RFC 2119. They have
that meaning in numbered requirements (`REQ-*`), invariants (`INV-*`), contracts (`CTR-*`), acceptance criteria
(`AC-*`), and tables marked **normative**.

### Identifiers

| Prefix | Meaning |
|---|---|
| `US-*` | User story |
| `AC-*` | Acceptance criterion, scoped to its story |
| `REQ-*` | Normative requirement |
| `CTR-*` | Surface contract (§7) |
| `INV-*` | Invariant that MUST hold at all times |
| `ERR-*` | Named error condition |
| `NFR-*` | Non-functional requirement |
| `OQ-*` | Open question. One marked **blocking** MUST be closed before the release it blocks is frozen |
| `DEF-*` | Explicitly deferred scope |

Identifiers are stable. Every family starts at 1 and is never renumbered. A withdrawn or moved identifier is not
reused, and its line stays with a note naming the amendment item that withdrew or moved it.

### Units

- **Weight** has two components: `ref_time`, in units where 10¹² is one second of execution on reference hardware,
  and **proof size**, in bytes.
- **Time** is counted in ticks of **the chain clock** (§2). Every duration in this document is a whole number of
  ticks. There are no calendar months: "a month" is whatever fixed number of ticks an offer states.
- **Amounts** are exact integers in the minor units of their asset.

---

## 1. Purpose and scope

### 1.1 What it is

A **usage subscription** is a recurring **usage contract** between a **group** and the **collective**, with two
parts:

- **Periodic limits in weight.** A `ref_time` allowance and a proof-size allowance per **usage period**.
- **A cost.** A price, in a given asset, per **billing period**, always paid in advance. A contract renews until
  cancelled, unless its terms set a term limit.

The contract establishes a **pool**. Every member of the group draws on it. While the group is within its
allowance and the contract is paid, a member's transaction fee is **waived**, and the transaction's weight is
charged to the pool. In every other case the member pays the normal fee.

Offers come in three kinds:

- **Standard**: an open-ended offer the collective publishes, which any usable group may accept.
- **Trial**: a limited number of billing periods, free or discounted, which a group may take once, and which
  converts into a paid contract at its end only if the group asked for that when it subscribed.
- **Custom**: terms the collective agrees with one group through a collective referendum, optionally with a minimum
  commitment and a term limit.

All three share one lifecycle. The collective may amend a custom contract, or a standard offer and every contract
made from it, under notice and with a free exit for each affected group.

A **group** is any collective body with its own account and memberships. The system is generic over the group.
Groups issue their own memberships, each backed by a deposit, and control how they move.

### 1.2 Who it is for

- **Groups**, which buy usage for their members and see what they buy and use.
- **Members**, who transact without holding the fee token while their group's contract covers them.
- **The collective**, which sets the terms on which usage is sold and receives the payments.

### 1.3 In scope

- Standard, trial and custom offers, contracts and their lifecycle: subscribe, renew, suspend, restore, lapse,
  default, cancel, switch, convert, amend, terminate, complete.
- The pool: admission, metering and charging of pool-path transactions, and how the three phases of the payment
  step agree.
- Which group pays when a member belongs to several.
- Recurring billing, as a general capability (Epic E).
- Group memberships: issuance with a deposit, assignment, release, transfer under the group's policy, and
  retirement after a removal delay.
- Billing that stays correct across multi-block data migrations.

### 1.4 Out of scope

- Reserved or guaranteed capacity.
- Fees for anything other than a signed transaction.
- Per-role or per-membership quotas inside a pool (`DEF-1`).
- Staking membership deposits (`DEF-8`).

### 1.5 Standing assumptions

1. The chain clock does not move within a block. Every phase of one transaction's payment step reads the same
   tick.
2. The three phases of a transaction's payment step run in order, in one state, for one transaction at a time:
   validation, then preparation, then dispatch, then post-dispatch. A decision made against the transaction pool's
   view of state is not carried into block execution; validation runs again there.
3. The deployment names a payee that receives the collective's charges, and the collective has a referendum process
   whose outcome can act as an origin.
4. A membership of a group is a distinct, inspectable object, owned by one account.
5. Every group has its own account, which can hold balances and holds.
6. The deployment may run data migrations that span several blocks, during which no transaction is included and no
   background processing runs.

---

## 2. Glossary

| Term | Meaning |
|---|---|
| **Allowance** | The weight a contract allows per usage period: a `ref_time` amount and a proof-size amount. Both MUST be non-zero |
| **Amendment** | New terms approved by a referendum of the collective's amend origin, with no acceptance by any group, for one custom contract or for a standard offer and every live contract made from it (`REQ-CT-8`) |
| **Amendment notice period** | For one contract, the time from an amendment's enactment to that contract's effective boundary: at least one billing period, and less than two (`REQ-CT-14`) |
| **Anchor** | The tick at which a contract started. Every usage window and billing period of the contract is counted from it |
| **Billing period** | The fixed number of ticks that one payment covers. Every billing period is paid at its start |
| **Chain clock** | The monotonic tick counter that every time-based rule here reads. One tick is nominally six seconds. Several blocks may share one tick |
| **Charge** | One payment of a contract's price for one billing period, from the group's account to the payee, made as a direct (never escrowed) payment in the deployment's payments system (`REQ-BL-3`) |
| **Collective** | The chain-wide body that sells usage. It acts through its referendum origins and is paid through the payee |
| **Commitment end** | anchor + c · billing period, where c is the minimum commitment. A contract with no commitment has none |
| **Contract** | Short for usage contract: a group's subscription to an offer, with the offer's terms copied in at subscription |
| **Conversion** | A trial's opt-in continuation: at the trial's end, a contract to the offer the group named at subscription starts, if its first charge succeeds (`REQ-CT-13`) |
| **Defaulted** | The state of a contract that lapsed within its minimum commitment. It blocks the group until the commitment ends |
| **Deposit** | An amount of the native token held on a group's account for as long as one of its memberships exists |
| **Effective boundary** | For one contract, the first billing boundary b with b − e ≥ its billing period, where e is the tick an amendment was enacted (`REQ-CT-14`) |
| **Estimate** | A transaction's metered weight before dispatch, from its declared weights |
| **Fee path** | The payment step's ordinary behaviour: the payer is charged a fee |
| **Grace** | The time after a missed charge during which the group can still pay and restore the contract |
| **Group** | A collective body with its own account, an administrative origin and its own memberships |
| **Group account** | The account a group pays from, holds its deposits in, and holds its unassigned stock in |
| **Lead** | How long before a charge is due it may first be attempted |
| **Membership** | An item of a group's collection. It is either unassigned stock (held by the group account), assigned (held by a member), or retiring |
| **Member** | An account, other than the group account, that holds a membership of the group |
| **Metered weight** | The weight a transaction is accounted for in its block: its call, every transaction extension, and the per-transaction base, with the transaction's encoded length added to proof size (`REQ-PL-6`) |
| **Migration pause** | The ticks during which a multi-block data migration runs, and no transaction or background processing can run (`REQ-BL-8`) |
| **Minimum commitment** | A number of billing periods a custom contract must be paid for, even if cancelled earlier |
| **Offer** | A set of terms the collective makes available. It is **standard**, **trial** or **custom** (§5.1) |
| **Paid through** | The tick up to which a contract's billing is settled |
| **Paying group** | The group whose pool a member's transaction draws on (§5.6) |
| **Payee** | The account the deployment configures to receive the collective's charges |
| **Pool** | A group's allowance in its current usage window, less what that window has used |
| **Pool path** | The payment step's behaviour when the pool pays: no fee, and the actual metered weight charged to the pool |
| **Removal delay** | How long a retiring membership waits before it is burnt and its deposit released (`REQ-MI-11`) |
| **Retirement** | Removing a membership from its group: requested, then completed after the removal delay |
| **Stock** | A group's unassigned memberships, held by the group account |
| **Term limit** | How many billing periods a contract runs. A contract with none is **open-ended** and renews until cancelled |
| **Terms** | Allowance, usage period, price, billing period, term limit, minimum commitment and grace together |
| **Ticket** | What admission found: the member, the paying group, the membership, the contract and the estimate. Post-dispatch charges against it |
| **Transfer policy** | A group's rule for transfers of its memberships: who may receive (none, existing members, any account) and whether the rank travels (`REQ-MI-15`) |
| **Trial** | An offer kind: a limited number of billing periods, free or discounted, acquirable once per group. It converts into a paid contract only if the group opted in (`REQ-CT-13`) |
| **Usable group** | A group the deployment allows to subscribe and to draw on a pool (`REQ-GR-3`). The collective's own group never is (`REQ-GR-4`) |
| **Usage contract** | See *contract* |
| **Usage period** | The fixed number of ticks after which a pool's usage resets |
| **Usage subscription** | This system: groups buying weight for their members on a recurring contract |
| **Usage window** | One usage period of one contract, counted from its anchor |

---

## 3. Parties

| Party | Acts through | May |
|---|---|---|
| **Collective** | Its referendum origins: an administrative origin for offers and terminations, and a dedicated amend origin for amendments (`REQ-OF-1`, `REQ-OF-8`). It may also have a group of its own (`REQ-GR-4`) | Publish and withdraw offers. Agree custom terms with a group. Amend a custom contract or a standard offer. Terminate a contract |
| **Group** | Its administrative origin, which the deployment configures | Subscribe, cancel, switch offer, opt into or cancel a conversion, pay arrears. Issue, assign, release and retire memberships. Set its transfer policy |
| **Member** | A signed transaction from its own account, after authentication | Transact on the pool path when eligible. Choose a paying group. Transfer its membership as its group's policy allows |
| **Payee** | Passive | Receive charges |
| **Anyone** | A signed transaction | Trigger a charge that is due. Complete a retirement whose delay has passed. Read every query |

A group's **account** pays its charges and holds its deposits. A member never pays for the group, and the group
never pays a member's fee-path fees.

---

## 4. Structure

### 4.1 Responsibilities

| Responsibility | Covers |
|---|---|
| **Contract registry** | Offers, contracts, their lifecycle, and the terms copied into each contract |
| **Pool meter** | Each contract's usage window and usage. Admission (read-only) and charging (one write per pool-path transaction) |
| **Payment step** | Asks the pool meter whether a transaction takes the pool path, carries the answer across the three phases, and otherwise defers to the fee path |
| **Recurring subscriptions** | Subscription offers, subscriptions, due charges, suspension, grace, lapse, default, commitments, cancellation, amendment, termination (Epic E) |
| **Memberships manager** | Group memberships: issuance with a deposit, assignment, release, transfer under the group's policy, retirement with a delay. The only path that moves a membership (Epic F) |
| **Group bindings** | What the deployment says about a group: its memberships, its account, and whether it is usable (`REQ-GR-*`) |

### 4.2 Relations

- **Recurring subscriptions** own money and time: charges, `paid through`, suspension, grace, lapse, default and
  commitments. They know nothing about weight or groups, and move each charge as a direct payment through the
  deployment's payments system.
- **The contract registry and pool meter** own weight: allowances, usage windows and admission. They read billing
  state from recurring subscriptions and never move money. They know groups only through the group bindings.
- **The payment step** owns the fee decision for one transaction. It never decides eligibility itself, and it never
  charges a pool except through a ticket.
- **The memberships manager** owns membership items and their deposits. Nothing else moves, burns or sells them.

### 4.3 The three phases

1. **Validation** decides the path, pool or fee, and writes nothing.
2. **Preparation** reaches the same decision as validation did for the same transaction (`INV-2`). The pool path
   prepares nothing; the fee path prepares the fee.
3. **Post-dispatch** charges the pool the transaction's *actual* metered weight against the ticket admission issued
   (`INV-3`), whatever the call itself changed (`INV-13`).

---

## 5. Domain model

### 5.0 Groups

- `REQ-GR-1` The system MUST be generic over a group. It MUST learn about a group's memberships only through the
  deployment's memberships interface: which memberships an account holds (optionally in one group), whether an
  account holds a membership of a group, and which group a membership belongs to. It MUST NOT depend on any one kind
  of group.
- `REQ-GR-2` The deployment MUST name, for every group, the account that pays the group's charges and holds its
  deposits (the group account). Naming it MUST need no storage read.
- `REQ-GR-3` The deployment MUST say whether a group is **usable**, in at most one storage read. Only a usable group
  MAY subscribe, and only a usable group's pool MAY admit a transaction.
- `REQ-GR-4` If the collective has a group of its own, that group MUST NOT be usable: it MUST NOT subscribe, and no
  pool of its MAY admit a transaction.

### 5.1 Offer

**Normative.** An offer holds:

| Field | Rule |
|---|---|
| kind | *Standard*, *trial* or *custom* |
| `ref_time` allowance | > 0, per usage period |
| proof-size allowance | > 0 bytes, per usage period |
| usage period | ≥ the deployment's minimum usage period, in ticks |
| price | An asset the deployment accepts, the native token included, and an amount ≥ that asset's minimum. A trial's amount MAY be zero |
| billing period | ≥ the deployment's minimum billing period, in ticks |
| term limit | Standard: none (open-ended). Trial: a positive number of billing periods, at most the deployment's maximum trial length. Custom: none, or a positive number of billing periods |
| minimum commitment | Standard and trial: none. Custom: none, or a positive number of billing periods, at most the term limit if there is one |
| grace | < billing period, in ticks |
| eligibility | Standard and trial: any usable group. Custom: exactly one named group |
| status | *Open* or *withdrawn* |

- `REQ-OF-1` Only the collective MAY publish or withdraw an offer, or make an amendment. Standard and trial offers
  MUST be published by the collective's administrative origin. A custom offer MUST be published only by the outcome
  of a collective referendum. An amendment MUST come only from the amend origin (`REQ-OF-8`).
- `REQ-OF-2` An offer that breaks any rule of the table MUST be refused with `ERR-InvalidTerms`. The native token
  MUST be an accepted asset.
- `REQ-OF-3` Withdrawing an offer MUST stop new subscriptions to it. It MUST NOT change any contract made from it,
  except as an amendment already enacted for that offer provides (`REQ-CT-8`, `INV-9`).
- `REQ-OF-4` A custom offer MUST be acceptable only by its named group. Once accepted, it MUST NOT be accepted
  again.
- `REQ-OF-5` Every billing period MUST be paid at its start. No kind of offer MAY bill in arrears.
- `REQ-OF-6` A trial offer MUST have a term limit and MUST NOT have a minimum commitment. Its price MAY be zero or
  any amount the table allows.
- `REQ-OF-7` A group MUST be able to start at most one trial, ever, across all trial offers. Subscribing to a trial
  after the group has started one MUST be refused with `ERR-TrialUsed`.
- `REQ-OF-8` The deployment MUST configure a dedicated **amend origin**, distinct from the origin that publishes
  offers, whose only power is to make amendments. It MUST be the outcome of a collective referendum such that:
  - the shortest possible time from the referendum's submission to its enactment is at least the deployment's
    minimum notice;
  - the referendum is confirmed only after a confirmation period during which it passes continuously; and
  - with the collective's voting membership at the time, a single member changing its vote during confirmation stops
    the confirmation.

### 5.2 Contract and its lifecycle

A contract copies the offer's terms at subscription. It records its group, its offer, its kind, its anchor, its
`paid through`, the number of billing periods charged, its state, and any pending cancellation, switch, conversion
or amendment.

- `REQ-CT-1` A group MUST have at most one contract at a time, **regardless of its state**: *Active*, *Suspended*
  or *Defaulted* (`INV-8`). Subscribing while one exists MUST be refused with `ERR-AlreadyContracted`.
- `REQ-CT-2` A contract's terms MUST NOT change after subscription, except by an amendment (`REQ-CT-8`, `INV-9`).
  Any other change of terms is a different contract (`REQ-CT-7`).

**States (normative).**

| State | Pool usable? | Blocks a new contract? | Meaning |
|---|---|---|---|
| *Active* | Yes, while now < `paid through` | Yes | Paid up to `paid through` |
| *Suspended* | No | Yes | A charge is due and unpaid, and grace has not elapsed |
| *Defaulted* | No | Yes, until the commitment end | It lapsed within its minimum commitment. Arrears are not collected |
| *Ended* | — | No | Terminal. The record is removed in the same block (`REQ-CT-12`). The event records one reason: *completed*, *cancelled*, *lapsed*, *switched*, *converted* or *terminated* |

**Transitions (normative).** `d` is the due tick of the next charge (`paid through`). `G` is grace. `L` is lead.
`E` is the commitment end; a contract with no commitment behaves as if E were the anchor. `b` is the contract's
effective boundary for a pending amendment (`REQ-CT-14`). Every grace end here is extended by any migration pause, as
`REQ-BL-8` requires.

| From | To | When | Money |
|---|---|---|---|
| — | *Active* | `subscribe`, and the first charge succeeds | Charge for billing period 0 (nothing if the price is zero). Anchor = now |
| *Active* | *Active* | A charge attempted at t ≥ d − L succeeds, or at t ≥ d when d = b | `paid through` += billing period. A pending amendment applies from b (`REQ-CT-8`, `REQ-CT-14`) |
| *Active*, *Suspended* | (same) | An amendment that covers the contract is enacted | None. It is pending until b, and the free exit is open (`REQ-CT-15`) |
| *Active* | *Suspended* | A charge attempted at t ≥ d fails | None |
| *Suspended* | *Active* | A charge attempted at t < d + G succeeds | `paid through` += billing period. The anchor does not move |
| *Suspended* | *Defaulted* | t ≥ d + G, the charge is still unpaid, and d < E | None. Arrears are not collected |
| *Suspended* | *Ended* (*lapsed*) | t ≥ d + G, the charge is still unpaid, and d ≥ E | None |
| *Defaulted* | *Ended* (*lapsed*) | t ≥ E | None |
| *Active* | *Ended* (*completed*) | t ≥ `paid through`, the term limit's last period is paid, and no switch or conversion takes over | None |
| *Active* (trial) | *Ended* (*converted*) | t ≥ `paid through`, the trial's last period is paid, a conversion is pending, its target is open and eligible, and the new contract's first charge succeeds (`REQ-CT-13`) | Charge for the new contract's period 0 |
| *Active* | *Ended* (*cancelled*) | t ≥ `paid through`, the group cancelled, and `paid through` ≥ E | None |
| *Active* | *Ended* (*cancelled*) | t ≥ `paid through`, and the group cancelled while an amendment was pending (`REQ-CT-15`) | None. The commitment is waived |
| *Suspended* | *Ended* (*cancelled*) | The group cancels, and d ≥ E or an amendment is pending | None. Arrears are not collected |
| *Active* | *Ended* (*switched*) | t ≥ `paid through`, a switch is pending, `paid through` ≥ E, and the new contract's first charge succeeds | Charge for the new contract's period 0 |
| *Active*, *Suspended*, *Defaulted* | *Ended* (*terminated*) | The collective terminates | None. The prepaid, unused part of the current period is forfeited |

- `REQ-CT-3` Every transition MUST follow this table. Any other transition is refused, and nothing changes.
- `REQ-CT-4` Between `d` and a successful charge, the pool MUST be unusable, even when the state has not yet moved
  to *Suspended*. Pool usability is computed from `paid through` and the clock, not from bookkeeping that may run
  late (`INV-7`).
- `REQ-CT-5` A cancellation MUST take effect at the later of `paid through` and the commitment end, except under the
  free exit (`REQ-CT-15`). No charge is attempted for a billing period that starts at or after that tick, and the
  pool stays usable until then while the contract is paid. Cancelling a *Suspended* contract outside its commitment
  MUST end it at once.
- `REQ-CT-6` A collective termination MUST end the contract at once, whatever its state, *Defaulted* included. No
  refund is made: the prepaid, unused part of the current billing period is forfeited. Termination MUST NOT touch
  the group's membership deposits. A termination lifts the block of `REQ-CT-1` at once.
- `REQ-CT-7` A group MAY request a **switch** to another offer it is eligible for. The switch MUST take effect at
  the first billing boundary at or after both `paid through` and the commitment end, and only if the new contract's
  first charge succeeds then. If that charge fails, the switch MUST be dropped, and the old contract MUST proceed as
  it would have without it (renew, or complete if its term limit is reached). At most one switch MAY be pending
  (`ERR-SwitchPending`); a trial's conversion counts as one (`REQ-CT-13`). The group MAY cancel a pending switch
  before it takes effect (`ERR-NoPendingSwitch` if there is none).
- `REQ-CT-8` **Amendment.** The collective MAY, through its amend origin (`REQ-OF-8`), amend either one *Active* or
  *Suspended* custom contract, or a standard offer together with every *Active* or *Suspended* contract made from
  it. No group's acceptance MUST be required. An amendment MAY change the allowance, price, grace, minimum commitment
  and term limit, MUST keep the terms valid for the offer's kind (§5.1), and MUST NOT change the usage period or the
  billing period (`ERR-InvalidTerms`). For each contract it covers, it takes effect at that contract's effective
  boundary (`REQ-CT-14`): the new price for the billing period that starts there, and the new allowance from the
  first usage window that starts at or after it, and each such contract has the free exit (`REQ-CT-15`). The anchor
  MUST NOT move, and nothing is prorated. A contract made from an amended offer after the amendment's enactment
  takes the amended terms. At most one change MAY be pending per contract: an amendment enacted while a switch is
  pending cancels the switch, and a switch requested, or a second amendment enacted, while an amendment is pending
  for that contract is refused with `ERR-ChangePending`.
- `REQ-CT-9` A trial contract MUST NOT renew. At its term limit's end, a pending conversion (`REQ-CT-13`) or a switch
  the group scheduled during the trial takes effect under `REQ-CT-7`. Otherwise, or if that fails, the trial MUST end
  (*completed*).
- `REQ-CT-10` **Cancelling within the commitment.** If the group cancels before the commitment end, billing MUST
  continue, period by period, until the commitment end. The contract then ends (*cancelled*).
- `REQ-CT-11` **Lapsing within the commitment.** If a charge due before the commitment end stays unpaid past grace,
  the contract MUST become *Defaulted*. A *Defaulted* contract's pool is unusable, no charge is attempted, arrears
  are not collected, and the contract MUST keep blocking the group (`REQ-CT-1`) until the commitment end or a
  collective termination, whichever comes first.
- `REQ-CT-12` A contract that ends MUST have its record removed in the same block, and the event MUST name the
  reason. A *Defaulted* contract MUST be kept until the commitment end, and is then ended (*lapsed*) and removed. A
  *Defaulted* contract whose commitment end has passed MUST NOT block a new subscription, even if its record has not
  been removed yet.
- `REQ-CT-13` **Conversion is opt-in.** A group subscribing to a trial MAY name one offer the trial converts into.
  The named offer MUST be open, MUST NOT be a trial, and MUST be one the group is eligible for; otherwise the
  subscription is refused with `ERR-InvalidConversion`, and nothing changes. The conversion is the trial's pending
  switch: at the trial's end it takes effect under `REQ-CT-7`, and the target's terms are copied into the new
  contract then. If the target has been withdrawn, or the group is no longer eligible for it, or the new contract's
  first charge fails, the trial MUST end (*completed*) with no conversion, and the event MUST name the reason. The
  group MAY cancel the conversion before the trial ends. A trial with no named offer MUST NOT convert.
- `REQ-CT-14` **Amendment notice period.** For each contract an enacted amendment covers, the amendment MUST take
  effect at that contract's **effective boundary**: the first billing boundary b = anchor + n·B with b − e ≥ B,
  where e is the tick of enactment and B the contract's billing period. The charge for the billing period that
  starts at b MUST NOT be attempted before b: the lead does not apply to it. If the contract ends before b, the
  amendment never applies to it.
- `REQ-CT-15` **Free exit.** While an amendment is pending for a contract (from its enactment to that contract's b),
  the group MAY cancel with its minimum commitment waived. An *Active* contract then ends (*cancelled*) at its
  `paid through`, which is never later than b; a *Suspended* one ends at once. No commitment billing follows, the
  contract never becomes *Defaulted*, and the block of `REQ-CT-1` lifts when it ends. Nothing is refunded.

### 5.3 Pool and usage windows

- `REQ-PL-1` A contract's usage windows MUST be `[anchor + k·U, anchor + (k+1)·U)` for k = 0, 1, 2, …, where U is
  the usage period. The current window is the one that contains now (`INV-5`).
- `REQ-PL-2` A pool's usage MUST reset to zero exactly at each window boundary. Computing the current window and
  its usage MUST need no write. A boundary passing is not an event that has to be processed.
- `REQ-PL-3` Allowance not used in a window MUST be lost when the window ends.
- `REQ-PL-4` A pool's usage in a window MUST never exceed its allowance, in either component (`INV-4`).
- `REQ-PL-5` Neither suspension nor a migration pause MAY pause or shift usage windows. After a restore, usage
  continues in whatever window now contains the clock, with that window's recorded usage.

### 5.4 Billing periods

- `REQ-BL-1` Billing period n MUST cover `[anchor + n·B, anchor + (n+1)·B)`, where B is the billing period. Its
  charge falls due at `anchor + n·B`, in advance (`REQ-OF-5`). Period 0 MUST be charged at subscription.
- `REQ-BL-2` Usage periods and billing periods are independent. Neither MUST be a multiple of the other. A billing
  boundary that falls inside a usage window MUST NOT change that window's allowance or usage.
- `REQ-BL-3` Each billing period MUST be charged at most once (`INV-10`). A charge MUST be a direct payment in the
  deployment's payments system, from the group's account to the payee, never held in escrow, and it MUST move exactly
  the contract's price: the payments system MUST take no fee from either side of a charge whose beneficiary is the
  payee.
- `REQ-BL-4` A charge falls **due** at `d`. It MAY be attempted from `d − L`. Due charges MUST be attempted
  automatically, a bounded number per block (`NFR-3`). Anyone MAY also trigger a due charge.
- `REQ-BL-5` A failed charge MUST move nothing.
- `REQ-BL-6` Once a term limit's last period is paid, no further charge MUST be attempted.
- `REQ-BL-7` A charge of zero (a free trial) MUST move nothing and MUST count as paid.
- `REQ-BL-8` **Migration pauses.** A migration pause MUST NOT change any contract's billing outcome:
  - the ticks of a migration pause MUST NOT count towards grace: a grace end that has not passed when the pause
    begins MUST be extended by the pause's length;
  - no contract MAY be suspended, lapsed or defaulted because a charge could not be attempted during a pause;
  - charges that fell due, or whose lead began, during a pause MUST be attempted before any other due charge,
    starting in the first block after the pause and before that block's transactions;
  - billing boundaries, commitment ends and usage windows MUST NOT move (`REQ-PL-5`).

### 5.5 Admission, metering and charging

- `REQ-PL-6` **Metered weight.** A transaction's metered weight MUST be exactly what the block's weight accounting
  books for it: its call and every transaction extension's weight, plus the per-transaction base weight of its
  dispatch class, plus its encoded length as proof size. The **estimate** uses declared weights. The **actual** uses
  the post-dispatch call and extension weights after refunds, plus the same base and length, and MUST be capped at
  the estimate.
- `REQ-PL-7` **Admission.** A transaction MUST take the pool path if, and only if, all of these hold when it is
  validated:
  1. Its origin, after authentication, is a signed account A.
  2. A paying group P is resolved for A (§5.6).
  3. A holds a valid membership of P (`REQ-PC-1`).
  4. P is a usable group (`REQ-GR-3`).
  5. P has an *Active* contract, and now < its `paid through`.
  6. For both components, the current window's usage plus the estimate is at most the allowance.

  Otherwise it MUST take the fee path.
- `REQ-PL-8` Admission MUST write no state (`INV-1`). It MUST yield a **ticket** that names A, P, the membership,
  the contract and the estimate.
- `REQ-PL-9` Post-dispatch MUST charge the ticket's contract with the actual metered weight, and with nothing for a
  transaction that reports it pays no fee. It MUST add that weight to the usage of the window the ticket was issued
  in (`INV-3`, `INV-13`).
- `REQ-PL-10` A pool-path transaction MUST cost its signer nothing. Any tip is ignored and not charged. The
  transaction MUST get no priority above that of a zero-tip fee-path transaction.
- `REQ-PL-11` A fee-path transaction MUST be charged exactly as the fee path alone would charge it (`INV-12`).
- `REQ-PL-12` When the pool cannot cover a transaction's estimate, that transaction MUST take the fee path, even if
  a smaller transaction would still fit. A pool is never partly applied to a fee.

### 5.6 Paying group and valid membership

- `REQ-PC-1` A membership is **valid** for group P when it is a membership of P held by the account, and the
  account is not P's group account (`REQ-MI-13`).
- `REQ-PC-2` A member MAY name a paying group. The resolved paying group MUST be:
  - the named one, if the member holds a valid membership of it;
  - otherwise, if the member holds valid memberships of exactly one group, that group;
  - otherwise none, and the transaction takes the fee path.
- `REQ-PC-3` Resolving the paying group MUST read a bounded number of a member's memberships (`NFR-1`). If that
  bound is reached before the answer is certain, the result MUST be none.
- `REQ-PC-4` Naming a group the account holds no valid membership of MUST be refused with `ERR-NotAMember`. A name
  that later becomes invalid MUST be ignored, never deleted as a side effect of admission. Naming MUST be a call of
  its own, never data carried by a transaction extension.
- `REQ-PC-5` Naming or clearing a paying group MUST take no fee, and MUST NOT be charged to any pool, when the
  signer holds a valid membership of the group it names (or, when clearing, of the group it had named), and has made
  fewer than the deployment's maximum number of changes in the current rate window. The rate window is the
  deployment's minimum usage period, counted from tick 0. Otherwise the call is an ordinary transaction.

### 5.7 Memberships: issuance, deposits, assignment and retirement

- `REQ-MI-1` A group MUST be able to issue memberships into its own collection, with its administrative origin.
  Each issued membership MUST be owned by the group account, as unassigned stock, and MUST place a deposit
  (`REQ-MI-D1`).
- `REQ-MI-2` Issuing MUST attach no allowance or weight to a membership. A membership carries no usage. Its group's
  contract does.
- `REQ-MI-3` *Withdrawn by 0001-A7.*
- `REQ-MI-4` Releasing a member's membership MUST reset every attribute membership management set for the former
  holder (the rank included). Burning a membership (only at retirement) MUST remove every attribute of the item.
  Nothing is left behind on a burnt item.
- `REQ-MI-5` *Withdrawn by 0001-A8.*
- `REQ-MI-6` The membership limit MUST be the deposit.
- `REQ-MI-7` Memberships MUST NOT expire. A membership is valid for as long as its holder holds it.
- `REQ-MI-8` Releasing a member MUST return the membership to the group's stock, owned by the group account. It
  MUST NOT change any deposit.
- `REQ-MI-9` A member MAY transfer its membership only as its group's transfer policy allows (`REQ-MI-15`). The
  membership MUST stay in its group's collection. A transfer MUST NOT change any deposit or the group's count of
  assigned memberships.
- `REQ-MI-10` Only the memberships manager MAY assign, release, transfer, retire or burn a membership. Moving,
  burning, selling or swapping a membership item by any other path MUST fail, and nothing else MAY make the manager
  unable to act on its group's items (`INV-18`). This MUST rest on a lock on each item, not on refusing every call of
  the underlying item system: a group's administrative origin issues and retires stock, and a member transfers under
  its group's policy, each through an origin the deployment configures.
- `REQ-MI-11` A retirement MUST wait a removal delay before the membership is burnt and its deposit released. The
  delay MUST be at least the staking unbonding period of the chain where the deposits' aggregated balance can be
  staked, plus a margin.
- `REQ-MI-12` The deposit amount and the removal delay MUST be deployment parameters. A membership's deposit is
  fixed when it is placed: a later change of the parameter MUST NOT change what an existing membership holds, and a
  later change of the delay MUST NOT change a retirement already requested.
- `REQ-MI-13` A group account MUST NOT be a member of its own group. Its stock is not a membership for admission,
  for voting, or for any membership query.
- `REQ-MI-14` Membership identifiers MUST be unique across all groups of a deployment, so that an identifier alone
  names one membership of one group.
- `REQ-MI-15` **Transfer policy.** Every group MUST have a transfer policy, set only by its administrative origin and
  kept by the memberships manager. It has two parts:
  - **who may receive**: *disabled* (no transfer, `ERR-TransferDisabled`); *existing members* (only an account that
    already holds a valid membership of the group, otherwise `ERR-NotSameGroup`); or *any account* (any account
    other than the group account, which then holds a membership of the group);
  - **the rank**: *reset* to zero on transfer, or *kept*.

  A group that has never set a policy MUST behave as *disabled*, with the rank *reset*. The group account MUST never
  receive a transfer (`REQ-MI-13`).
- `REQ-MI-16` No origin but the deployment's root origin MAY, on a group's collection or its items: destroy the
  collection; mint into it; change its mint settings; change its team of roles; transfer its ownership; lock it; set
  its maximum supply; lock an item's transfer; or lock an item's properties. The memberships manager does these
  through its own calls only.

**The deposit (normative).**

- `REQ-MI-D1` Every membership MUST have a deposit, in the native token, held on its group account for as long as
  the membership exists, recorded per membership. Issuing a membership places it; an issuance the group account
  cannot cover is refused with `ERR-MembershipLimit`, and nothing is issued.
- `REQ-MI-D2` Retiring is requested by the group, and only for unassigned stock (`ERR-NotRetirable`). The
  membership MUST leave the stock at once and MUST NOT be assignable from then on. After the removal delay, anyone
  MAY complete the retirement: the membership is burnt and its deposit released to the group account. Completing
  early is refused with `ERR-RetirementNotElapsed`. An assigned membership is released first.
- `REQ-MI-D3` A membership has no recurring charge. The limit is the group account's free balance divided by the
  deposit.
- `REQ-MI-D4` *Withdrawn by 0003-A3.*
- `REQ-MI-S1`–`REQ-MI-S4` *Withdrawn by 0001-A7.*

---

## 6. User stories

### Epic A — Offers (the collective)

**`US-A1`** — As the **collective**, I publish a standard offer that any group can accept.
- `AC-A1.1` Given the administrative origin and valid open-ended terms, When I publish, Then the offer is *open*
  and visible to every group (`REQ-OF-1`).
- `AC-A1.2` Given a zero allowance component, a grace ≥ the billing period, a period below the minimum, or a term
  limit or commitment on a standard offer, Then it is refused with `ERR-InvalidTerms` (`REQ-OF-2`, `REQ-OF-5`).
- `AC-A1.3` Given any other origin, Then it is refused with `ERR-BadOrigin`.

**`US-A2`** — As the **collective**, I agree custom terms with one group through a referendum.
- `AC-A2.1` Given a passed collective referendum, When it publishes a custom offer for group C, with or without a
  minimum commitment and term limit, Then only C can accept it (`REQ-OF-4`, `ERR-NotEligible`).
- `AC-A2.2` Given the custom offer was accepted, When anyone tries to accept it again, Then it is refused with
  `ERR-OfferWithdrawn`.

**`US-A3`** — As the **collective**, I withdraw an offer without touching the contracts made from it.
- `AC-A3.1` Given an open offer with live contracts, When I withdraw it, Then new subscriptions are refused, and
  every existing contract keeps its terms and renews as before (`REQ-OF-3`, `INV-9`).

**`US-A4`** — As the **collective**, I terminate a contract.
- `AC-A4.1` Given an *Active*, *Suspended* or *Defaulted* contract, When the collective terminates it, Then it is
  *Ended* (*terminated*) at once, nothing is refunded, the group's membership deposits are unchanged, its members
  take the fee path from the next transaction, and the group may subscribe again (`REQ-CT-6`).

**`US-A5`** — As the **collective**, I amend a custom contract, or a standard offer and its contracts, by
referendum, with notice.
- `AC-A5.1` Given an *Active* custom contract and a passed referendum of the amend origin with new allowance and
  price, When its effective boundary b arrives, Then the charge for the period from b is attempted no earlier than b
  and at the new price, and the new allowance applies from the first usage window starting at or after b. The anchor
  is unchanged, and the group was never asked to accept (`REQ-CT-8`, `REQ-CT-14`).
- `AC-A5.2` Given an amendment that changes the usage period or the billing period, that gives a standard offer a
  term limit or a commitment, or that targets a trial offer, a trial contract, or a standard contract on its own,
  Then it is refused with `ERR-InvalidTerms`.
- `AC-A5.3` Given a pending switch, When an amendment is approved, Then the switch is cancelled and the amendment
  is pending. Given a pending amendment, When the group requests a switch, Then it is refused with
  `ERR-ChangePending`.
- `AC-A5.4` Given B = 30 days and boundaries on days 30, 60 and 90, When an amendment is enacted on day 27, Then it
  takes effect on day 60. Enacted on day 30, it takes effect on day 60; enacted on day 31, on day 90 (`REQ-CT-14`).
- `AC-A5.5` Given any origin other than the amend origin, the administrative origin included, When it amends, Then
  it is refused with `ERR-BadOrigin` (`REQ-OF-1`, `REQ-OF-8`).
- `AC-A5.6` Given the contract ends before b, Then the amendment never applies to it, and no charge is made at the
  new price (`REQ-CT-14`).
- `AC-A5.7` Given a standard offer with live contracts on different anchors, When an amendment of the offer is
  enacted, Then each contract takes the new terms at its own effective boundary, each group has the free exit until
  then, and a group that subscribes to the offer afterwards gets the new terms (`REQ-CT-8`, `REQ-CT-14`,
  `REQ-CT-15`).
- `AC-A5.8` Given an offer amendment pending for a contract, When the offer is withdrawn, Then the contract still
  takes the amended terms at its effective boundary (`REQ-OF-3`).

### Epic B — Contracts (the group)

**`US-B1`** — As a **group**, I subscribe to an offer and my members can transact at once.
- `AC-B1.1` Given an open offer I am eligible for, no contract record, and enough funds, When I subscribe, Then
  period 0 is charged to the payee, the contract is *Active* with its anchor at now, and my members' next
  transactions take the pool path (`REQ-BL-1`, `REQ-PL-7`).
- `AC-B1.2` Given insufficient funds, Then it is refused with `ERR-ChargeFailed`, and nothing is created.
- `AC-B1.3` Given any contract record, *Defaulted* included, Then it is refused with `ERR-AlreadyContracted`
  (`INV-8`).

**`US-B2`** — As a **group**, I cancel, and I keep what I paid for.
- `AC-B2.1` Given an *Active* contract with no commitment, When I cancel, Then the pool stays usable until `paid
  through`, no further charge is attempted, and the contract then ends (*cancelled*) (`REQ-CT-5`).

**`US-B3`** — As a **group**, I switch to another offer at my next billing boundary.
- `AC-B3.1` Given a pending switch and enough funds at `paid through`, past any commitment, Then the old contract
  ends (*switched*) and the new one starts there, with period 0 charged (`REQ-CT-7`).
- `AC-B3.2` Given the new first charge fails, Then the switch is dropped, and the old contract renews normally.

**`US-B4`** — As a **group**, I recover from a missed payment within grace.
- `AC-B4.1` Given a *Suspended* contract and t < d + G, When the charge succeeds, Then the contract is *Active*
  with `paid through` = d + B and the same anchor (§5.2).
- `AC-B4.2` Given t ≥ d + G outside any commitment, Then the contract has *Ended* (*lapsed*) and its record is
  gone, and paying is refused with `ERR-GraceElapsed`. Within a commitment, see `US-B7`.

**`US-B5`** — As a **group**, I can see my contract and my pool.
- `AC-B5.1` Given any contract, Then a query returns its state, kind, terms, `paid through`, commitment end, the
  current window's end, the allowance and the usage, all from one consistent state (`REQ-OB-1`).

**`US-B6`** — As a **group**, I try the system once before paying.
- `AC-B6.1` Given a trial offer and a group that has never started a trial, When I subscribe, Then the contract is
  *Active*, and a zero price moves nothing (`REQ-OF-6`, `REQ-BL-7`).
- `AC-B6.2` Given the trial's last period ends with no conversion and no switch pending, Then the contract ends
  (*completed*), no charge is attempted, and my members take the fee path (`REQ-CT-9`).
- `AC-B6.3` Given my group has started a trial before, When I subscribe to any trial, Then it is refused with
  `ERR-TrialUsed` (`REQ-OF-7`).

**`US-B7`** — As a **group** under a minimum commitment, I am held to it.
- `AC-B7.1` Given a custom contract with a commitment end E, When I cancel before E, Then charges continue until E
  and the contract then ends (*cancelled*) (`REQ-CT-10`).
- `AC-B7.2` Given a charge due before E stays unpaid past grace, Then the contract is *Defaulted*, my pool is
  unusable, no arrears are collected, and subscribing is refused with `ERR-AlreadyContracted` until E
  (`REQ-CT-11`).
- `AC-B7.3` Given a *Defaulted* contract and t ≥ E, Then it ends (*lapsed*), and subscribing succeeds
  (`REQ-CT-12`).
- `AC-B7.4` Given a *Defaulted* contract, When the collective terminates it by referendum, Then the block is lifted
  at once (`REQ-CT-6`).

**`US-B8`** — As a **group**, I can ask my trial to turn into a paid contract when it ends.
- `AC-B8.1` Given I subscribed to a trial naming open standard offer S, When the trial's last period ends and S's
  first charge succeeds, Then the trial ends (*converted*), a contract to S starts at the trial's `paid through` with
  S's terms as they stand then, and my members stay on the pool path (`REQ-CT-13`).
- `AC-B8.2` Given S's first charge fails at the trial's end, Then the trial ends (*completed*), no contract to S
  exists, and the event names the failed charge.
- `AC-B8.3` Given S was withdrawn, or I am no longer eligible for it, by the trial's end, Then the trial ends
  (*completed*) with no charge attempted, and the event names the reason.
- `AC-B8.4` Given I cancel the conversion before the trial ends, Then the trial ends (*completed*) and does not
  convert. Given no switch or conversion is pending, When I cancel one, Then it is refused with
  `ERR-NoPendingSwitch`.
- `AC-B8.5` Given I name a trial, a withdrawn offer, or a custom offer for another group, When I subscribe, Then it is
  refused with `ERR-InvalidConversion`, and nothing is created. Given I name nothing, Then the trial never converts.

**`US-B9`** — As a **group** whose contract is being amended, I can leave for free before it changes.
- `AC-B9.1` Given an *Active* contract with commitment end E and an amendment pending with effective boundary b < E,
  When I cancel before b, Then the contract ends (*cancelled*) at its `paid through` (≤ b), no charge is made after
  it, the contract is never *Defaulted*, and I may subscribe again at once. Nothing is refunded (`REQ-CT-15`).
- `AC-B9.2` Given a *Suspended* contract with an amendment pending, When I cancel, Then it ends at once.
- `AC-B9.3` Given the amendment has taken effect, When I cancel, Then the commitment applies again, as amended
  (`REQ-CT-10`).

### Epic C — The pool (members)

**`US-C1`** — As a **member**, my transactions are free while my group is covered.
- `AC-C1.1` Given admission holds (`REQ-PL-7`), When I submit a transaction, Then I pay nothing, and the pool is
  charged the actual metered weight (`REQ-PL-9`, `REQ-PL-10`).
- `AC-C1.2` Given the actual weight is below the estimate, Then only the actual weight is charged (`INV-3`).

**`US-C2`** — As a **member**, I fall back to paying my own fee, and I am never locked out.
- `AC-C2.1` Given the pool cannot cover my estimate, or the contract is *Suspended*, *Defaulted*, ended or unpaid at
  d, or my membership is not valid, Then my transaction takes the fee path (`REQ-PL-7`, `REQ-PL-12`).
- `AC-C2.2` Given the transaction is the first after a usage window boundary, Then it is admitted against a fresh
  window, and validation, preparation and dispatch all succeed (`INV-2`, `INV-5`).
- `AC-C2.3` Given admission chose the pool path in validation, Then preparation never panics and never falls back
  to an unvalidated fee path (`INV-2`, `INV-15`).

**`US-C3`** — As a **member of several groups**, I choose which one pays.
- `AC-C3.1` Given valid memberships of C1 and C2 and no named group, Then my transactions take the fee path
  (`REQ-PC-2`).
- `AC-C3.2` Given I name C2, Then C2's pool is used. And given I then leave C2, Then my named choice is ignored, and
  I take the fee path (`REQ-PC-4`).
- `AC-C3.3` Given I hold a valid membership of C2 and no balance, When I name C2 within the rate limit, Then the call
  succeeds and takes no fee. Given I exceed the rate limit, Then the call is an ordinary transaction (`REQ-PC-5`).

**`US-C4`** — As a **member**, a transaction that changes my membership or the contract cannot dodge the charge.
- `AC-C4.1` Given a pool-path transaction whose call transfers or releases my membership, or cancels the contract,
  Then post-dispatch still charges the pool the ticket named (`INV-13`).

### Epic D — Billing (collective, payee, anyone)

**`US-D1`** — As the **payee**, I receive each billing period's price exactly once.
- `AC-D1.1` Given a due charge and funds, When it is attempted automatically or by anyone, Then exactly the price
  moves to the payee, and `paid through` advances by one billing period (`REQ-BL-3`, `INV-10`).
- `AC-D1.2` Given the same period is triggered twice, Then the second attempt is refused with `ERR-NothingDue`.

**`US-D2`** — As the **collective**, a missed payment suspends the pool.
- `AC-D2.1` Given t ≥ d and no successful charge, Then no member's transaction is admitted to the pool path, even
  before any bookkeeping has run (`REQ-CT-4`).

**`US-D3`** — As **anyone**, I can push a due charge through.
- `AC-D3.1` Given a charge due or within lead, When I trigger it, Then it is attempted. Given it is not, Then it is
  refused with `ERR-NothingDue` (`REQ-BL-4`).

**`US-D4`** — As a **group**, a data migration never costs me my contract.
- `AC-D4.1` Given a charge that falls due during a migration pause, When the pause ends, Then the charge is attempted
  before any other due charge, in the first block after the pause and before that block's transactions, and its
  grace end is extended by the pause's length (`REQ-BL-8`, `INV-21`).
- `AC-D4.2` Given a *Suspended* contract whose grace would end during a pause, Then it does not lapse or default
  before its grace end plus the pause's length (`REQ-BL-8`).
- `AC-D4.3` Given a pause that crosses a usage window boundary, Then the windows' boundaries and recorded usage are
  as if there had been no pause (`REQ-PL-5`).

### Epic E — Recurring subscriptions

This epic is the general capability that Epics B and D rely on. The wording is generic: a **merchant** publishes,
a **subscriber** subscribes, a **payee** is paid.

**`US-E1`** — As a **merchant**, I publish a subscription offer.
- `AC-E1.1` Given my inventory, When I publish with conditions (price, billing period, term limit, minimum
  commitment, grace, eligibility), Then the offer exists, and later changes to its conditions affect only new
  subscriptions, except through an amendment (`REQ-SB-1`).

**`US-E2`** — As a **subscriber**, I subscribe, and I am charged up front.
- `AC-E2.1` When I subscribe, Then period 0 is charged to the payee, and the subscription is *Active* with its
  `paid through` one billing period ahead (`REQ-SB-2`).
- `AC-E2.2` Given many subscribers to one standard offer, Then each has its own independent subscription
  (`REQ-SB-3`).

**`US-E3`** — As a **merchant**, due charges are collected without my intervention.
- `AC-E3.1` Given subscriptions falling due, Then they are attempted automatically, a bounded number per block,
  in due order, and the rest stay queued (`REQ-SB-4`, `NFR-3`).
- `AC-E3.2` Given a failed charge, Then the subscription is *Suspended*. If grace elapses unpaid, it *lapses*, or
  is *Defaulted* within its commitment (`REQ-SB-5`, `REQ-SB-9`).

**`US-E4`** — As a **subscriber or merchant**, I end a subscription.
- `AC-E4.1` Cancellation by the subscriber ends it at the later of `paid through` and the commitment end.
  Termination by the merchant ends it at once (`REQ-SB-6`).

**`US-E5`** — As a **dependent system**, I am told when a subscription changes.
- `AC-E5.1` Every start, renewal, suspension, restoration, default, amendment and end notifies registered
  dependants in the same block (`REQ-SB-7`).

### Epic F — Memberships

**`US-F1`** — As a **group**, I issue my own memberships.
- `AC-F1.1` Given my administrative origin and enough free balance, When I issue n memberships, Then n memberships
  enter my stock, owned by my group account, with no allowance, and n deposits are held on my group account
  (`REQ-MI-1`, `REQ-MI-2`, `REQ-MI-D1`).
- `AC-F1.2` *Withdrawn by 0001-A7.*
- `AC-F1.3` Given new identifiers, Then no identifier of any group is reused (`REQ-MI-14`).

**`US-F2`** — As the **collective**, a group's membership limit is enforced by its deposits.
- `AC-F2.1` Given issuance the group account cannot cover, Then it is refused with `ERR-MembershipLimit`, and nothing
  is issued (`REQ-MI-D1`).

**`US-F3`** — As a **group**, releasing a member returns the membership to my stock and leaves nothing behind.
- `AC-F3.1` Given a released membership, Then it is in my stock, owned by my group account, with its rank reset, and
  my deposits are unchanged (`REQ-MI-4`, `REQ-MI-8`).
- `AC-F3.2` Given a retired membership is burnt, Then the item has no attributes left (`REQ-MI-4`).

**`US-F4`** — As a **member**, I can hand my membership on only as my group allows.
- `AC-F4.1` Given my group's policy is *existing members* with the rank *reset*, and the recipient holds a valid
  membership of my group, When I transfer, Then the recipient holds it, its rank is zero, and the group's count of
  assigned memberships and its deposits are unchanged (`REQ-MI-9`, `REQ-MI-15`).
- `AC-F4.2` Given the policy is *existing members* and the recipient holds no valid membership of my group, or the
  recipient is the group account, Then it is refused with `ERR-NotSameGroup`.
- `AC-F4.3` Given any direct item transfer, burn, purchase or swap claim of a membership item, by its holder, a
  delegate or the group account, Then it fails, and the item has not moved (`REQ-MI-10`, `INV-18`).
- `AC-F4.4` Given my group never set a policy, or set *disabled*, When I transfer, Then it is refused with
  `ERR-TransferDisabled`.
- `AC-F4.5` Given the policy is *any account*, When I transfer to an account that holds no membership of my group,
  Then that account holds the membership, in the group's collection, and is a member.
- `AC-F4.6` Given the rank is *kept*, When I transfer, Then the recipient holds the membership with its rank.
- `AC-F4.7` Given a group account acting through its administrative origin, or any other origin but root, When it
  attempts any act `REQ-MI-16` lists, Then the attempt fails and nothing changes.

**`US-F5`** — As a **group**, I retire memberships I no longer need, and get the deposit back after the delay.
- `AC-F5.1` Given a membership in my stock, When I retire it, Then it leaves my stock at once and cannot be
  assigned (`REQ-MI-D2`).
- `AC-F5.2` Given the removal delay has passed, When anyone completes the retirement, Then the item is burnt, its
  attributes are gone, and its deposit is released to my group account. Before that, it is refused with
  `ERR-RetirementNotElapsed`.
- `AC-F5.3` Given an assigned membership, When I retire it, Then it is refused with `ERR-NotRetirable`.

**`US-F6`** — As a **group**, I decide whether and how my memberships may be transferred.
- `AC-F6.1` Given a group that never set a policy, Then its policy reads *disabled*, rank *reset* (`REQ-MI-15`).
- `AC-F6.2` Given my administrative origin, When I set a policy, Then the next transfer follows it, and an event
  names my group and the policy.
- `AC-F6.3` Given any other origin, a member's included, When it sets my policy, Then it is refused with
  `ERR-BadOrigin`.

### Epic G — *Moved to PLAN by 0003-A7*

`US-G1` (with `AC-G1.1`, `AC-G1.2`), `US-G2` (with `AC-G2.1`, `AC-G2.2`) and `US-G3` (with `AC-G3.1`–`AC-G3.4`) are
transition stories. *Moved to PLAN by 0003-A7.*

---

## 7. Surface contracts

### 7.A `CTR-CALL` — calls

Names are examples. Each call's behaviour is in §8.1.

| Party | Calls |
|---|---|
| Collective | `publish_offer`, `withdraw_offer`, `amend_contract`, `amend_offer` (amend origin only), `terminate_contract` |
| Group | `subscribe` (optionally naming a conversion), `cancel`, `switch_offer`, `cancel_switch`, `issue_memberships`, `retire_memberships`, `set_transfer_policy`, and its membership management (assign, release) |
| Member | `set_paying_group`, `transfer_membership` |
| Anyone | `charge_due`, `complete_retirement` |

- `CTR-CALL-1` Every call MUST have a declared weight that bounds its worst case (`NFR-4`).
- `CTR-CALL-2` Every refusal MUST be one of §10's named errors.

### 7.B `CTR-QRY` — queries

- `CTR-QRY-1` These MUST be readable without a transaction: an offer and the open offers; a group's contract
  (§8.3); a group's pool; an account's resolved paying group; whether a group has used its trial; a group's transfer
  policy; and `would_waive(account, estimate)`. `would_waive` MUST return the admission decision with a reason,
  never a bare boolean.

### 7.C `CTR-EVT` — events

- `CTR-EVT-1` Every state change of offers, contracts and memberships MUST emit exactly one event naming the group
  (or, for an offer, the offer): offer published, amended or withdrawn; contract started, charged, suspended,
  restored, defaulted, amended (enacted, with its effective boundary, and in force), ended (with its reason);
  switch or conversion scheduled, cancelled or dropped (with the reason); paying group set; memberships issued;
  membership transferred; transfer policy set; retirement requested or completed.
- `CTR-EVT-2` A pool-path transaction MUST emit at most one event: the payment step's own per-transaction charge
  event. No other per-transaction event is added.

### 7.D `CTR-FEE` — the payment-step boundary

- `CTR-FEE-1` `check(account, estimate)` MUST return a ticket or nothing. It MUST NOT write. For the same
  arguments and the same state it MUST return the same answer.
- `CTR-FEE-2` `charge(ticket, actual)` MUST NOT fail. It MUST charge min(actual, the ticket's estimate) to the
  ticket's contract, in the ticket's window, and return the pool's remainder.
- `CTR-FEE-3` In validation, the payment step MUST call `check` once, and MUST carry its decision (pool or fee) to
  preparation. On the fee path it MUST validate the fee path, and carry that value.
- `CTR-FEE-4` In preparation, on the pool path, it MUST call `check` again. If the answer differs from
  validation's, it MUST reject the transaction as invalid (`ERR-PathMismatch`). It MUST NOT panic, and MUST NOT
  fall back to a fee path it did not validate.
- `CTR-FEE-5` In post-dispatch, on the pool path, it MUST call `charge` with the actual metered weight, unless the
  dispatch reports that it pays no fee. The weight it reports as unspent MUST be its own unspent weight only, never
  the call's (`INV-11`).
- `CTR-FEE-6` Its declared weight MUST cover the worst of both paths: the check plus the charge, or the check plus
  the fee path.

### 7.E `CTR-SUB` — the recurring-subscription capability

- `CTR-SUB-1` Publish an offer with conditions; withdraw it; change conditions, which affects new subscriptions
  only, except through an amendment.
- `CTR-SUB-2` Subscribe (charge period 0, then *Active*). Charge due (with lead, permissionless). Cancel (at the
  later of `paid through` and the commitment end). Terminate (at once, no refund). Schedule a replacement at the
  first due tick at or after the commitment end: at that tick, try the replacement's first charge, and if it fails,
  continue the current subscription as normal (`REQ-CT-7`). Cancel a scheduled replacement before it takes effect.
- `CTR-SUB-3` Read a subscription's state, `paid through`, next due tick, term remaining, commitment end and grace
  end, each in a bounded number of reads.
- `CTR-SUB-4` Notify dependants of every transition, in the same block.
- `CTR-SUB-5` Process due charges automatically, a bounded number per block, oldest due first, and charges held up
  by a migration pause before all others (`REQ-BL-8`).
- `CTR-SUB-6` Amend one subscription's conditions, or an offer's conditions for every live subscription to it,
  each subscription taking effect at its own first due tick at least one billing period after the amendment,
  without moving its anchor and without charging early for that tick. While an amendment is pending for a
  subscription, let the subscriber cancel with its commitment waived (`REQ-CT-8`, `REQ-CT-14`, `REQ-CT-15`).
- `CTR-SUB-7` Not count the ticks of a migration pause against any grace (`REQ-BL-8`).

### 7.F `CTR-MEM` — group bindings and membership facts

- `CTR-MEM-1` Answer "does account A hold a valid membership of group P?" in a bounded number of reads, never
  counting P's group account (`REQ-PC-1`, `REQ-MI-13`).
- `CTR-MEM-2` Enumerate an account's memberships lazily, so that the caller can stop after k, in a deterministic
  order.
- `CTR-MEM-3` Answer "is group P usable?" in at most one read (`REQ-GR-3`).
- `CTR-MEM-4` Name group P's group account, with no read (`REQ-GR-2`).

### 7.G `CTR-CLK` — the clock

- `CTR-CLK-1` One monotonic, non-decreasing tick count, constant within a block, nominally six seconds a tick. Every
  period, anchor, due tick, grace, lead, commitment end, rate window, removal delay and migration pause here is in
  its ticks.

---

## 8. Normative behaviour

### 8.1 Calls

Every call is subject to its origin rule, to `CTR-CALL-2`, and to `REQ-CT-3`.

| Call | Pre-conditions | Post-conditions |
|---|---|---|
| `publish_offer` | Origin per `REQ-OF-1`. Terms valid for the kind (`REQ-OF-2`, `REQ-OF-5`, `REQ-OF-6`) | Offer *open*. Event. Otherwise `ERR-BadOrigin` or `ERR-InvalidTerms` |
| `withdraw_offer` | Collective origin. Offer *open* | Offer *withdrawn*. Contracts unchanged, except as a pending amendment provides (`REQ-OF-3`). Otherwise `ERR-UnknownOffer` or `ERR-OfferWithdrawn` |
| `subscribe` | Group origin. Group usable (`REQ-GR-3`, `REQ-GR-4`). Offer *open* and eligible. No contract record (`REQ-CT-1`). For a trial, no trial used (`REQ-OF-7`), and any named conversion valid (`REQ-CT-13`). Period 0 chargeable | Contract *Active*, anchor = now, period 0 charged (`REQ-BL-1`). A trial marks the group's trial used, and a named conversion is pending. Otherwise `ERR-GroupUnusable`, `ERR-OfferWithdrawn`, `ERR-NotEligible`, `ERR-AlreadyContracted`, `ERR-TrialUsed`, `ERR-InvalidConversion` or `ERR-ChargeFailed`, and nothing changes |
| `cancel` | Group origin. A contract, *Active* or *Suspended* | Effective at the later of `paid through` and the commitment end (`REQ-CT-5`, `REQ-CT-10`); with an amendment pending, at `paid through`, commitment waived (`REQ-CT-15`). *Suspended* outside a commitment, or with an amendment pending: ends now. Otherwise `ERR-NoContract` |
| `switch_offer` | Group origin. *Active* contract. Target offer *open* and eligible. No pending switch, conversion or amendment | Switch pending, effective per `REQ-CT-7`. Otherwise `ERR-SwitchPending`, `ERR-ChangePending`, `ERR-NotEligible` or `ERR-NoContract` |
| `cancel_switch` | Group origin. A pending switch or conversion | Dropped. Event. Otherwise `ERR-NoPendingSwitch` (`REQ-CT-7`, `REQ-CT-13`) |
| `amend_contract` | The amend origin only (`REQ-OF-8`). *Active* or *Suspended* custom contract. New terms valid, same usage and billing periods. No amendment pending for it | Amendment pending, effective at its effective boundary (`REQ-CT-8`, `REQ-CT-14`); the free exit opens (`REQ-CT-15`). A pending switch is cancelled. Otherwise `ERR-BadOrigin`, `ERR-NoContract`, `ERR-ChangePending` or `ERR-InvalidTerms` |
| `amend_offer` | The amend origin only (`REQ-OF-8`). An open standard offer. New terms valid for a standard offer, same usage and billing periods | The offer's terms change for new subscriptions now. For every *Active* or *Suspended* contract made from it, the amendment is pending until that contract's effective boundary, with the free exit (`REQ-CT-8`, `REQ-CT-14`, `REQ-CT-15`). Otherwise `ERR-BadOrigin`, `ERR-UnknownOffer`, `ERR-OfferWithdrawn` or `ERR-InvalidTerms` |
| `charge_due` | Any signed origin. A charge is due, or within lead, or a *Suspended* contract is within grace | Charge attempted, then transition per §5.2. Otherwise `ERR-NothingDue` or `ERR-GraceElapsed` |
| `terminate_contract` | Collective origin. A contract in any state | *Ended* (*terminated*) now; no refund; deposits untouched (`REQ-CT-6`) |
| `set_paying_group` | Signed. A valid membership of the named group, or "none" | Name recorded or cleared (`REQ-PC-2`). Free per `REQ-PC-5`. Otherwise `ERR-NotAMember` |
| `issue_memberships` | Group origin. n ≤ the per-call cap. Deposits coverable | n memberships in the group's stock, n deposits held, fresh identifiers (`REQ-MI-1`, `REQ-MI-14`, `REQ-MI-D1`). Otherwise `ERR-MembershipLimit` or `ERR-TooManyMemberships` |
| `retire_memberships` | Group origin. Each membership in the group's stock | Each leaves the stock, retiring until now + removal delay (`REQ-MI-D2`). Otherwise `ERR-NotRetirable` |
| `complete_retirement` | Any signed origin. A retiring membership whose delay has passed | Burnt, attributes gone, deposit released (`REQ-MI-D2`, `REQ-MI-4`). Otherwise `ERR-RetirementNotElapsed` |
| Assign (the group's member management) | Group origin. A membership in the group's stock | The member holds it, rank zero, member count + 1 |
| Release (the group's member management) | Group origin. The account holds that membership | Back in the stock, rank reset, member count − 1, deposits unchanged (`REQ-MI-8`). Otherwise `ERR-NotAMember` |
| `transfer_membership` | Signed holder of the membership. Recipient per the group's policy (`REQ-MI-9`, `REQ-MI-15`) | The recipient holds it; rank reset or kept per the policy. Otherwise `ERR-NotAMember`, `ERR-TransferDisabled` or `ERR-NotSameGroup` |
| `set_transfer_policy` | Group origin | The group's policy recorded. Event (`REQ-MI-15`). Otherwise `ERR-BadOrigin` |

### 8.2 The transaction path

For every transaction that is not feeless, in this order (`CTR-FEE`):

| Phase | Behaviour |
|---|---|
| Validation | If the origin is not a signed account: fee path. Otherwise compute the estimate (`REQ-PL-6`) and call `check`. A ticket means the pool path: no fee validation, default priority. Nothing means the fee path: validate the fee as the fee path does. Either way, carry the decision. Write nothing |
| Preparation | Pool path: call `check` again. The same ticket means proceed. Anything else means `ERR-PathMismatch`, the transaction is invalid, and no panic. Fee path: prepare the fee with the value validation carried |
| Dispatch | Unchanged |
| Post-dispatch | Pool path: unless the dispatch pays no fee, `charge(ticket, actual)`. Then report only the payment step's own unspent weight. Fee path: as the fee path does |

- `REQ-TX-1` If the fee path fails validation, the transaction MUST be invalid for payment, exactly as the fee path
  alone would make it.
- `REQ-TX-2` Nothing in the transaction path MUST panic or rely on an assertion that state did not change between
  phases (`INV-15`).

A free `set_paying_group` call (`REQ-PC-5`) is feeless, so it does not enter this path.

### 8.3 Queries

| Query | Behaviour |
|---|---|
| `offer(id)`, `open_offers()` | Kind, terms, eligibility, status, and any amendment pending for its contracts |
| `contract(group)` | State, kind, terms, anchor, `paid through`, next due, grace end, commitment end, periods charged, pending cancel, switch, conversion or amendment (with its effective boundary, and whether the free exit is open) |
| `pool(group)` | Allowance, the current window's start and end, usage, remainder, and whether it is usable now with a reason |
| `paying_group(account)` | The resolved group or none, with a reason (`REQ-PC-2`) |
| `trial_used(group)` | Whether the group has started a trial (`REQ-OF-7`) |
| `transfer_policy(group)` | Who may receive the group's memberships, and whether the rank travels (`REQ-MI-15`) |
| `would_waive(account, estimate)` | Pool or fee, with the first failing condition of `REQ-PL-7` |

- `REQ-OB-1` Every query MUST answer from one consistent state, and MUST be pure.

---

## 9. Invariants

**Enforced by** names the layer that MUST keep each invariant: **Meter** (the contract registry and pool meter),
**Step** (the payment step), **Subs** (recurring subscriptions), **Mgr** (the memberships manager), **Runtime** (the
deployment's configuration).

| ID | Invariant | Enforced by |
|---|---|---|
| `INV-1` | Deciding the path of a transaction writes no state. | Meter + Step |
| `INV-2` | For one transaction in one state, preparation reaches the same path and the same ticket as validation. | Meter + Step |
| `INV-3` | A pool is charged min(actual metered weight, estimate) for each pool-path transaction that pays a fee, exactly once, and never the estimate. | Meter + Step |
| `INV-4` | In every usage window, usage ≤ allowance, in both components. | Meter |
| `INV-5` | The current usage window always contains now: start ≤ now < start + usage period. No window starts in the future. | Meter |
| `INV-6` | A transaction that is not feeless is paid exactly once: by the pool or by its payer, never both, never neither. | Step |
| `INV-7` | No transaction takes the pool path unless its group is usable, its contract is *Active* with now < `paid through`, and its signer holds a valid membership of that group. | Meter |
| `INV-8` | A group has at most one contract at a time, regardless of its state. A *Defaulted* contract blocks until its commitment end or a collective termination. | Meter + Subs |
| `INV-9` | A contract's terms never change after subscription, except by an amendment (of the contract, or of the standard offer it was made from) approved by a referendum of the amend origin, effective at the contract's effective boundary, with the anchor and both periods unchanged. | Meter + Subs |
| `INV-10` | Each billing period of a subscription is charged at most once, for exactly its price, to its payee. | Subs |
| `INV-11` | The pool path never reduces the weight a transaction is accounted for in its block. | Step |
| `INV-12` | A fee-path transaction is charged exactly as the fee path alone would charge it. | Step + Runtime |
| `INV-13` | The contract that post-dispatch charges is the one admission named, whatever dispatch changed. | Step + Meter |
| `INV-14` | No period arithmetic overflows or underflows. Every computation saturates or refuses. | Meter + Subs |
| `INV-15` | Nothing in the transaction path panics. | Step + Meter |
| `INV-16` | No membership item carries allowance, weight or expiration state. | Runtime + Mgr |
| `INV-17` | Every membership item is in its own group's collection, held by the group account (stock), by a member, or by the retirement holder while retiring. No membership is held anywhere else, and none exists outside its group's collection. | Mgr + Runtime |
| `INV-18` | No membership item moves, is burnt, sold or swapped except through the memberships manager, and nothing but the manager can mint into, hand over or lock a group's collection. | Mgr + Runtime |
| `INV-19` | Every membership has exactly its recorded deposit held on its group account until it is burnt. | Mgr |
| `INV-20` | No amended term applies to a contract before at least one full billing period has passed since the amendment was enacted, and no charge at amended terms is taken before the contract's effective boundary. | Meter + Subs |
| `INV-21` | A migration pause by itself causes no suspension, lapse or default, and moves no billing boundary, commitment end or usage window. | Subs + Meter |

Every `INV-*` MUST have a test at the layer named, and a runtime-level test through the real transaction
extensions where the layer is Step.

---

## 10. Errors

| ID | When |
|---|---|
| `ERR-BadOrigin` | The origin may not make this call |
| `ERR-InvalidTerms` | An offer or amendment breaks §5.1 or `REQ-CT-8` |
| `ERR-UnknownOffer` | No such offer |
| `ERR-OfferWithdrawn` | The offer is withdrawn, or is a custom offer already accepted |
| `ERR-NotEligible` | A custom offer for another group |
| `ERR-AlreadyContracted` | The group already has a contract, in any state (`REQ-CT-1`) |
| `ERR-ChargeFailed` | Period 0 could not be charged at subscription |
| `ERR-NoContract` | No contract in a state the call accepts |
| `ERR-NothingDue` | No charge is due, within lead, or within grace |
| `ERR-GraceElapsed` | The contract has lapsed or defaulted |
| `ERR-SwitchPending` | A switch is already pending |
| `ERR-ChangePending` | An amendment is pending, so no switch, and no second amendment, may be made |
| `ERR-NoPendingSwitch` | There is no pending switch or conversion to cancel |
| `ERR-InvalidConversion` | A trial's named conversion is not an open, non-trial offer the group is eligible for |
| `ERR-GroupUnusable` | The group is not usable |
| `ERR-TrialUsed` | The group has already started a trial |
| `ERR-NotAMember` | No valid membership of the named group, or the account does not hold that membership |
| `ERR-NotSameGroup` | A membership transfer that the group's policy does not allow to that recipient, or a transfer to the group account |
| `ERR-TransferDisabled` | The group's transfer policy is *disabled*, or it never set one |
| `ERR-MembershipLimit` | The group account cannot cover the deposits of an issuance |
| `ERR-TooManyMemberships` | Beyond the per-call cap |
| `ERR-MembershipIdTaken` | *Withdrawn by 0001-A7.* |
| `ERR-NotRetirable` | The membership is not in the group's stock |
| `ERR-RetirementNotElapsed` | The removal delay has not passed |
| `ERR-PathMismatch` | Transaction validity: preparation disagreed with validation. The transaction is invalid; nothing panics |
| `ERR-Payment` | Transaction validity: the pool path did not apply and the fee path failed |

---

## 11. Non-functional requirements

| ID | Requirement |
|---|---|
| `NFR-1` | Admission MUST read a bounded number of storage items, independent of how many memberships, groups or contracts exist. The bound is a deployment constant, and the worst case is measured. |
| `NFR-2` | The payment step's declared weight increase for a non-member MUST be bounded, measured, and stated in the release notes. It is paid by every signed transaction. |
| `NFR-3` | Automatic charges MUST be bounded per block. A backlog MUST drain in due order, and MUST never make a block overweight. |
| `NFR-4` | Every call, the payment-step path, and the automatic charge processing MUST have weights measured on the reference hardware the deployment uses for its published weights. |
| `NFR-5` | Every stored record MUST have a bounded encoded size. |
| `NFR-6` | Periods from one minute to at least two years, and removal delays up to at least 90 days, MUST be representable without overflow (`INV-14`). |
| `NFR-7` | *Moved to PLAN by 0003-A7.* |
| `NFR-8` | *Moved to PLAN by 0003-A7.* |
| `NFR-9` | Every data migration MUST be a multi-block migration: run in bounded steps across blocks, resumable from a cursor, with each step under a stated per-step weight and proof-size bound that fits the chain's blocks as they are actually built. It MUST be idempotent, MUST have pre- and post-upgrade checks that fail the rehearsal on any mismatch, and MUST be measured on a snapshot of the live chain. A one-block upgrade step is allowed only for constant-size work. |

---

## 12. Deferred scope

| ID | Deferred |
|---|---|
| `DEF-1` | Per-role or per-membership quotas inside a pool |
| `DEF-2` | Rollover of unused allowance |
| `DEF-3` | Immediate, prorated contract changes |
| `DEF-4` | Disputes over a termination |
| `DEF-5` | Paying for overage beyond the allowance, or topping a pool up mid-window |
| `DEF-6` | Refunds on termination or cancellation |
| `DEF-7` | Splitting one transaction's charge between groups |
| `DEF-8` | Staking membership deposits |
| `DEF-9` | Charging groups for issuing pass accounts that are free to their members |

---

## 13. Non-goals

- Reserved or guaranteed capacity, or priority.
- Calendar billing.
- Paying for unsigned, bare or inherent extrinsics, or for cross-chain message execution fees.
- Changing the fee path.
- Moving value to members.
- Expiring memberships.
- A collective membership store.

---

## 14. Open questions

None is open. Resolved, withdrawn and moved questions stay in place with the amendment item that closed them.

| ID | Status |
|---|---|
| `OQ-1` | *Resolved by 0001-A9* |
| `OQ-2` | *Resolved by 0001-A7* |
| `OQ-3` | *Resolved by 0002-A6* |
| `OQ-4` | *Resolved by 0002-A4* |
| `OQ-5` | *Resolved by 0001-A2* |
| `OQ-6` | *Resolved by 0001-A12* |
| `OQ-7` | *Resolved by 0002-A7* |
| `OQ-8` | *Resolved by 0001-A10* |
| `OQ-9` | *Resolved by 0002-A16* |
| `OQ-10` | *Resolved by 0002-A16* |
| `OQ-11` | *Resolved by 0001-A5* |
| `OQ-12` | *Resolved by 0002-A2* |
| `OQ-13` | *Withdrawn by 0001-A7* |
| `OQ-14` | *Resolved by 0002-A16* |
| `OQ-15` | *Resolved by 0001-A8* |
| `OQ-16` | *Resolved by 0002-A16* |
| `OQ-17` | *Withdrawn by 0002-A8* |
| `OQ-18` | *Resolved by 0002-A9; superseded by 0003-A3* |
| `OQ-19` | *Resolved by 0002-A14* |
| `OQ-20` | *Resolved by 0002-A17* |
| `OQ-21` | *Resolved by 0002-A10 and 0002-A19* |
| `OQ-22` | *Resolved by 0002-A11* |
| `OQ-23` | *Resolved by 0003-A3* |
| `OQ-24` | *Resolved by 0003-A1* |

---

## 15. Traceability

| Epic | Stories | Principal requirements | Key invariants |
|---|---|---|---|
| A — Offers | `US-A1`–`US-A5` | `REQ-OF-1`–`REQ-OF-8`, `REQ-CT-6`, `REQ-CT-8`, `REQ-CT-14` | `INV-9`, `INV-20` |
| B — Contracts | `US-B1`–`US-B9` | `REQ-GR-3`, `REQ-GR-4`, `REQ-CT-1`–`REQ-CT-15`, `REQ-BL-1`, `REQ-BL-7`, `REQ-OB-1` | `INV-8`, `INV-9`, `INV-20` |
| C — The pool | `US-C1`–`US-C4` | `REQ-GR-1`, `REQ-GR-2`, `REQ-PL-1`–`REQ-PL-12`, `REQ-PC-1`–`REQ-PC-5`, `REQ-TX-1`, `REQ-TX-2`, `CTR-FEE-*`, `CTR-MEM-*` | `INV-1`–`INV-7`, `INV-11`–`INV-13`, `INV-15` |
| D — Billing | `US-D1`–`US-D4` | `REQ-BL-1`–`REQ-BL-8`, `REQ-CT-4` | `INV-7`, `INV-10`, `INV-14`, `INV-21` |
| E — Recurring subscriptions | `US-E1`–`US-E5` | `REQ-SB-1`–`REQ-SB-14`, `CTR-SUB-*` | `INV-8`–`INV-10`, `INV-14`, `INV-20`, `INV-21` |
| F — Memberships | `US-F1`–`US-F6` | `REQ-MI-1`, `REQ-MI-2`, `REQ-MI-4`, `REQ-MI-6`–`REQ-MI-16`, `REQ-MI-D1`–`REQ-MI-D3` | `INV-16`–`INV-19` |
| G — *Moved to PLAN by 0003-A7* | — | — | — |

**Epic E requirements:**

- `REQ-SB-1` Changing an offer's conditions MUST affect new subscriptions only, except through an amendment
  (`REQ-SB-10`).
- `REQ-SB-2` Subscribing MUST charge period 0 before the subscription exists. A failed charge creates nothing.
- `REQ-SB-3` A standard offer MUST support any number of independent subscriptions. A custom offer MUST support
  exactly one.
- `REQ-SB-4` Due charges MUST be processed automatically, bounded per block, oldest due first. They MUST also be
  triggerable by anyone.
- `REQ-SB-5` Suspension, grace, lapse and default MUST follow §5.2.
- `REQ-SB-6` Cancellation and termination MUST follow §5.2, commitments included.
- `REQ-SB-7` Every transition MUST notify registered dependants in the same block.
- `REQ-SB-8` A scheduled replacement MUST behave as `CTR-SUB-2` states.
- `REQ-SB-9` A subscription's conditions MAY carry a minimum commitment and a term limit. A subscription that lapses
  within its commitment MUST be kept, *Defaulted*, until the commitment end, and MUST be reported as such to
  dependants.
- `REQ-SB-10` A merchant's authorised origin MAY amend one subscription's conditions, or an offer's conditions for
  every live subscription to it, without any subscriber's acceptance, each subscription taking effect at its own
  first due tick at least one billing period after the amendment, without moving its anchor (`CTR-SUB-6`).
- `REQ-SB-11` A price of zero MUST be valid for a subscription, and its charges MUST move nothing (`REQ-BL-7`).
- `REQ-SB-12` A scheduled replacement MAY be cancelled by the subscriber before it takes effect.
- `REQ-SB-13` While an amendment is pending for a subscription, the charge for the due tick it takes effect at MUST
  NOT be attempted before that tick, and the subscriber MAY cancel with its minimum commitment waived, ending at
  `paid through` (`REQ-CT-14`, `REQ-CT-15`).
- `REQ-SB-14` A migration pause MUST NOT count against any subscription's grace, and the charges it held up MUST be
  processed first when it ends (`REQ-BL-8`, `CTR-SUB-5`, `CTR-SUB-7`).

**Epic G requirements:** `REQ-RT-1`–`REQ-RT-8` *Moved to PLAN by 0003-A7.*

Cross-cutting: `NFR-1`–`NFR-6`, `NFR-9` and `CTR-CLK-1` serve every epic.

### Release scope

**Release 1 is everything in §6.** Everything in §12 is out.

**Release 1's exit criteria:**

- every `INV-*` tested at its layer, and through the real transaction extensions where the layer is Step;
- `NFR-9` passed on a fork of the target chain;
- `AC-C2.2` passing;
- `AC-D4.1`–`AC-D4.3` passing in a rehearsal where a charge falls due during a migration.
