# Proposal: permissionless, proof-verified execution for Spaces

**Status:** draft for discussion. Nothing here is implemented.
**Extends:** [`DESIGN.md`](./DESIGN.md) §3 (what an anchor proves) and §9 (open questions 1 and 5).

## 1. The problem

A Space's state moves only by proof (`anchor`). Its *effects* on the chain do not. When a Space must
pay someone, move an asset or call another pallet, it does so through `dispatch_as_account` or
`dispatch_as_space`, and only its **authority** can call those. So every effect a Space's program
decides must be carried out by whoever holds the authority. That has three costs:

- **The authority's keys become operational keys.** Whoever runs the Space's effects must hold an
  origin able to spend all of the Space's funds, online, all the time.
- **Trust comes back in through the side door.** The program's transition is proven, but the effect
  that follows is whatever the authority signs. Nothing on chain ties a payment to the transition
  that decided it.
- **Throughput is bounded by the authority's keys.** Effects go out as fast as those keys can sign
  and pay, and no faster.

## 2. The proposal in one paragraph

A Space's program, besides moving the state root, may **emit effects**, written as **XCM programs**.
The anchor commits to them. **Anyone** may then submit an effect, with proof that the Space's
anchored state emitted it. The pallet verifies the proof, executes the XCM with the Space as its
origin, and records that the effect ran so it never runs twice. The submitter pays the fee and holds
no authority: it can choose only *when* an effect runs, never *what* it does.

## 3. XCM as the language of effects

A Space's program runs off chain, in a VOS network, and cannot name a runtime's calls without being
rebuilt for every runtime. XCM is the chain's own portable language for "do this, as this origin":

- **Versioned and stable.** A `VersionedXcm` survives runtime upgrades. A program written against a
  call index would break on the next upgrade that reorders a pallet.
- **Already safe-by-construction for assets.** `WithdrawAsset`, `DepositAsset`, `TransferAsset` and
  `BuyExecution` express the common effects (pay, move, fee) without a raw `Transact`.
- **Bounded.** The executor weighs a message before running it, and a runtime's barrier and filters
  decide what a given origin may do, including whether `Transact` is allowed at all.
- **Cross-chain for free.** The same effect can target another parachain through the same executor.

**The Space's location.** Each Space gets an XCM location under the runtime, for example
`(PalletInstance(<spaces>), GeneralIndex(<space id>))`. A converter maps that location to the
Space's account for assets, and to the Space's origin (`EnsureSpace`) for anything that accepts it.
An effect therefore runs exactly as `dispatch_as_account` or `dispatch_as_space` would today, but
gated by a proof instead of the authority.

## 4. What the anchor commits to

`AnchorStatement` (DESIGN.md §3) gains one field:

```rust
AnchorStatement {
    domain: *b"fc-spaces/anchor",
    version: 2,          // was 1
    chain, space, epoch, number, prev_root, root,
    effects,             // Merkle root of the effects this transition emitted, or the empty root
}
```

`effects` is the root of a binary Merkle tree whose leaves are
`blake2_256(space ‖ number ‖ index ‖ encoded VersionedXcm)`, in the order the program emitted them.
The pallet stores it beside the anchored root, so each anchor grows by 32 bytes. The proof already
binds the whole statement, so it binds `effects` too: a relayer cannot add, drop or reorder an effect
without breaking the proof.

## 5. The calls

| Call | Origin | Does |
|---|---|---|
| `anchor(space, number, root, effects, proof, public)` | Any signed | As today, and stores `effects` with the anchor |
| `execute(space, number, index, message, inclusion)` | Any signed | Checks `message` is leaf `index` of anchor `number`'s `effects` (`inclusion` is the Merkle path), checks it has not run, executes it with the Space's location as origin, records it as run |
| `anchor_and_execute(space, number, root, effects, proof, public, messages)` | Any signed | Both in one extrinsic, when the effects are few: no inclusion proof needed, since the messages are hashed against `effects` directly |

**Replay.** The pallet keeps, per anchor, a bitmap of which effects have run. An effect runs at most
once. A failed execution (an XCM error) is recorded with its error and is **not** retried
automatically: whether a failed payment may be re-attempted is the program's decision, made by
emitting a new effect in a later transition.

**Ordering.** Effects of one anchor may run in any order, unless the program marks the anchor
`ordered`, in which case `execute` accepts only the lowest effect not yet run. Effects of a later
anchor never wait for an earlier one: a program that needs that waits before emitting.

**Expiry.** A program may give an effect a deadline (a block number in the leaf). After it, the
effect can no longer run, and its non-execution can be proven on chain. That is what lets a program
release whatever it had set aside for that effect, instead of waiting for it forever.

## 6. Two kinds of proof behind the same calls

The calls do not care how the anchor was proven. `Config::Verifier` (DESIGN.md §5) already
abstracts it, and two backends make sense:

- **A validity proof** (VOS's STARK): the strongest. The effect is exactly what the program
  computed from the previous anchored state.
- **An attestation**: the Space's program commitment is a public key, and the "proof" is that key's
  signature over the statement. The chain then trusts whoever holds the key to have run the program,
  but the effects are still bound to an anchored transition, run at most once, and submitted by
  anyone. It is the stepping stone for a Space whose program cannot yet be proven at an acceptable
  cost.

## 7. What it costs, and how storage stays flat

- **The proof is call data.** It is verified in dispatch and discarded. State keeps only what DESIGN.md
  §2 already keeps (the head, each anchor's root), plus the effects root and the bitmap.
- **Execution costs what the XCM weighs**, plus a Merkle path (about 32 bytes per level) for `execute`.
- **Proof size is the binding constraint**, as DESIGN.md §7 measures: about 0.5 MB for a STANDARD
  proof against a block limit of a few MB. `anchor_and_execute` amortises one proof over every effect
  of a transition. Beyond that, aggregation or recursion (one proof for many transitions) is the
  lever, and it belongs to the proof system, not to this pallet.
- **Fees.** The submitter pays. A runtime may refund it from the Space's account through a
  `BuyExecution` the program itself emits, so a Space can pay for its own effects without the
  submitter holding anything of the Space's.

## 8. What changes for the authority

Nothing it can do today is removed. `dispatch_as_account` and `dispatch_as_space` stay for governance
and emergencies. What changes is that routine effects no longer need it: a Space whose effects all go
through `execute` can keep its authority offline, behind a collective or a referendum, and still pay
out continuously.

## 9. Open questions

0. **XCM, or a simpler message format.** This draft uses XCM for effects (§3). A simpler messaging
   protocol of the pallet's own, proposed earlier in the Spaces design discussion, would be smaller to
   verify and to weigh, at the cost of XCM's versioning, its executor's barriers and its reach to other
   chains. This is to be settled in discussion before anything else here is built.
1. **Which XCM instructions a Space may emit.** Assets-only (no `Transact`) by default, with `Transact`
   opt-in per Space by its authority? Or left entirely to the runtime's barrier?
2. **The bitmap's lifetime.** Kept forever (flat per anchor, but it grows with the anchor count), or
   prunable once every effect of an anchor has run or expired?
3. **Failed effects.** Record and never retry (as above), or allow one retry within the deadline?
4. **Submission rights.** Same as DESIGN.md §9.1: should a Space be able to restrict who may submit
   `execute`, for example to control fee spend?
5. **Statement version.** Version 2 changes the statement every backend binds. Should version 1
   anchors (no effects) stay accepted side by side for existing Spaces, or should `set_program` move
   each Space across?
