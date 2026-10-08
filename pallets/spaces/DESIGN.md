# `fc-pallet-spaces`: design note

**Status: draft.** The pallet builds and is tested with a mock verifier and with VOS's STARK verifier
on a real proof. Weights are placeholders and the open questions below are unresolved.

## 1. What a Space is

A **Space** is an anchored, verifiable state machine on chain:

- a **program**, named by the 32-byte commitment the proof verifier recognises (for VOS, the
  program's preprocessed-trace Merkle root: VOS calls it the program's identity);
- a **head**: the root the next transition starts from, the number of the last anchor, and the
  current epoch;
- a gapless sequence of **anchors**, each a state root the chain accepted only with a proof that the
  program moved the Space from the head's root to it;
- **epochs**, each recording the root it starts from and the anchor number it starts after;
- **binds**, made by other pallets, recording that something of value now depends on the Space's
  anchored state.

The chain never interprets the Space's state or the program's public input. It checks that a proof
of the right program binds a statement the pallet builds, and it orders the results.

| Call | Origin | Does |
|---|---|---|
| `register(space, program, genesis)` | `RegisterOrigin` (its success is the owner) | Creates the Space at `genesis`, epoch 0, anchor number 0 |
| `anchor(space, number, root, proof, public)` | Any signed origin | Verifies the proof, stores anchor `number`, moves the head to `root` |
| `refound(space, genesis)` | The owner | Only with no binds: opens a new epoch at `genesis`; the sequence continues |
| `set_current_head(space, root)` | `ResetOrigin` | Opens a new epoch at `root` whatever the Space holds; the sequence continues |
| `SpaceBinds::{bind, unbind, is_bound}` | Other pallets (a trait, no extrinsic) | Records or releases a bind |

| Storage | Key | Value |
|---|---|---|
| `Spaces` | Space | Owner, program, bind count |
| `Heads` | Space | Epoch, last anchor number, root, block |
| `Anchors` | (Space, number) | Root, epoch, block. Never pruned or overwritten |
| `Epochs` | (Space, epoch) | Starting root, base number, block, cause (`Registered`, `Refounded`, `Reset`) |
| `Binds` | (Space, key) | Epoch, last anchor at the time, block |

## 2. What an anchor proves

The pallet builds the statement itself and passes its encoding to the verifier as the program's
**output**:

```rust
AnchorStatement {
    domain: *b"fc-spaces/anchor",
    version: 1,
    chain,      // the genesis hash: no replay across chains
    space,
    epoch,      // the head's
    number,     // the head's number + 1
    prev_root,  // the head's root: where the transition starts
    root,       // the anchored root: where it ends
}
```

A VOS program binds its inputs and output by leaving
`H = blake2b("vos/zk/io" ‖ blake2b("vos/zk/io-field" ‖ public) ‖ blake2b("vos/zk/io-field" ‖ output))`
in registers φ9..φ12 at halt, and the proof carries `H`. The pallet recomputes `H` from the
submitted `public` and the statement, so a proof verifies only for the Space, epoch, number and roots
it was made for. The submitter's `public` stays opaque: only the program decodes it.

Consequences:

- **Anyone may submit an anchor.** The proof is the authority; a relayer cannot change what it
  anchors, only whether it is submitted. The submitter pays.
- **No replay.** A proof names its anchor number and its starting root; once that number is stored,
  the same proof is a no-op (the stored root matches) and any other proof for that number is an
  `AnchorConflict`. Resubmitting a stored anchor succeeds without verifying, at the cost of three
  reads, so an adapter can retry safely.
- **A moved head invalidates outstanding proofs.** An anchor, a re-founding or a reset changes the
  number, the root or the epoch, and every proof made against the old head fails as `NotBound`.

## 3. Anchoring, binds and reset

| | Anchors already stored | The anchor sequence | Proofs made against the old head | Binds |
|---|---|---|---|---|
| `anchor` | Unchanged | Advances by one | Fail (`NotBound`, or a no-op if it is this very anchor) | Unchanged |
| `refound` (owner) | Unchanged, under their epoch | Continues: the next anchor is `base + 1` | Fail | **Must be none**, or it is refused (`HasBinds`) |
| `set_current_head` (privileged) | Unchanged, under their epoch | Continues: the next anchor is `base + 1` | Fail | **Carried over**, counted in the `HeadSet` event |

- **Continuation while nothing is bound.** A Space's program may need to start over from a new
  genesis (a re-founded state machine, a migrated state). While nothing of value depends on its
  anchored state, the owner can do that alone with `refound`: no vote, and no power over anyone
  else's value. The sequence continues, so every anchor ever made stays where it is and numbers are
  never reused; the epoch tells a reader which run an anchor belongs to.
- **Once something is bound, the owner cannot move the head off its anchored chain.** It can still
  anchor, and it can release binds through the pallet that made them. A state machine that must
  start over after value is bound either takes a new Space (and moves its value through that value's
  own rules) or asks the reset origin.
- **`set_current_head` is the escape hatch**, the analogue of `Paras::set_current_head`, with which a
  relay chain's governance sets a parachain's head: the earlier heads stay in history, and the chain
  continues from the new one. Here the earlier anchors stay, a new epoch opens at the given root,
  and binds carry over: the reset origin answers for the new root accounting for whatever is bound.
  It cannot rewrite or erase an anchor; it can only choose where the next one starts.
- **Why numbers continue instead of restarting at 1.** Restarting would key anchors by
  (epoch, number) and let two anchors share a number; continuing keeps one total order per Space
  and makes "never overwritten" a property of the storage key. A reader that wants per-run numbers
  subtracts the epoch's `base`.

What the pallet cannot see, and the owner must check before `refound`: that no movement of value is
in flight in another pallet that has not made a bind, and that whoever submitted anchors for the
previous run has stopped.

## 4. Where this comes from

It grew out of a feasibility study of attaching VOS's general-purpose STARK verifier
(<https://codeberg.org/Virto/VOS>, `vos-pvm-proof-verifier`: Circle STARK over M31, on StarkWare's
Stwo) to a Kreivo pallet, and of a spike that ran that verifier inside a minimal Substrate runtime
on the `stable2606` crates.

**Carried over:**

- **The verifier composition**, as VOS's own prover extension composes it: postcard-decode the proof
  with no trailing bytes, compare its io-hash with one recomputed through `sp-io`'s BLAKE2b-256,
  compare its program commitment **before** the verifier's own preflight (a proof of another program
  is then refused for the price of decoding it, about 1 ms instead of 11-13 ms), then
  `verify_standalone_with_options` under a pinned PCS policy and log-size cap (`vos::VosVerifier`).
- **Building for a runtime.** The wasm builder targets `wasm32v1-none`, for which the verifier needs
  Stwo from a fork that makes `dashmap` optional (a `[patch]` at the workspace root) and `getrandom`
  0.2 with a registered source that always fails. Both are in this workspace.
- **Verification in dispatch, never in `validate`.** A STARK takes about 20 ms in the runtime's wasm
  executor; in the transaction pool that work would be unpaid, on every validation.
- **The weight envelope**: about 55 ns of `ref_time` per proof byte, an upper envelope set by the
  largest (canonical-shape) proof, with **no** declared proof size for the proof's bytes:
  `CheckWeight` already counts the extrinsic's length as PoV, and declaring it again halves how many
  proofs fit a block.
- **Binding what the chain cares about into the program's output**, built by the pallet, so the
  proof commits to the chain, the Space and the transition; the public input stays opaque.
- **A trait seam for the proof system**, in the shape of a `ProofVerifier` with a program id, so the
  pallet does not depend on VOS unless a runtime asks for it.

**Adapted:**

- **One pallet, not two.** The study proposed a Spaces pallet beside a separate verifier pallet with a
  versioned program registry, call hooks dispatched under a program origin, replay sets and a notice
  queue. Here the verifier is a backend behind a trait, and the only thing a proof does is anchor:
  no hooks, no registry, no notices.
- **One program commitment per Space**, not a versioned allowlist (see the open questions).
- **The proof gates the anchor.** In the study, anchors were signed by a key and proofs referred to
  anchored roots. Here a Space is defined by proven transitions, so every anchor carries a proof
  and anyone may submit it.
- **Epochs, re-founding, binds and reset** are new: they come from the need to restart a Space's
  state machine without erasing its history, and to stop that once value depends on it.

## 5. Measured cost (for orientation, not weights)

From the spike, on an Apple M2 Pro, inside a minimal runtime's wasm executor (`frame-omni-bencher`,
20 repeats). **Not reference hardware.**

| Proof (VOS policy) | Postcard size | Verify |
|---|---|---|
| STANDARD | ≈ 0.51 MB | ≈ 22 ms |
| MOBILE | ≈ 0.86 MB | ≈ 25 ms |
| Canonical shape (all chips) | ≈ 1.29 MB | ≈ 67 ms |
| Tampered (refused) | ≈ 0.51 MB | ≈ 17 ms |

Peak heap 5-17 MiB, 513-640 logical stack items, under the relay chain's PVF executor configuration
too. **PoV binds before time**: at Kreivo's block limits, about five STANDARD proofs fit a block's
normal share. At Kreivo's current byte fee, the length fee dominates: about 1.7 KSM per STANDARD
proof, almost all of it for the bytes.

## 6. Open questions

1. **Program upgrades.** A Space has one program commitment, fixed at registration. A VOS commitment
   depends on the proof's execution shape and on VOS's proof format, so a new prover release or a
   new shape needs a new commitment. Options: a set of commitments per Space (VOS's allowlist of
   canonical shapes), a `set_program` call (by the owner while nothing is bound, by the reset origin
   otherwise), or a new Space per program version.
2. **Who may submit anchors.** Anyone, today. Should a Space be able to restrict submission (to its
   owner, or to a set of keys), for example to control who pays?
3. **Binds on reset.** `set_current_head` keeps every bind. Should it instead require none, or take an
   explicit list of binds to carry, or notify the pallets that made them?
4. **Registration deposit.** Registration takes no deposit; a runtime must restrict
   `RegisterOrigin` until it does. A `Consideration` (as other frame-contrib pallets hold) is the
   likely shape.
5. **Fees.** About 1.7 KSM per STANDARD proof at list price makes routine anchoring expensive. Fee
   relief for Spaces (for example a gas tank that charges the bytes), batching several transitions
   into one proof, or a smaller proof system are the levers; which one is acceptable is a runtime
   decision.
6. **The entering state.** For a program that takes a private witness, VOS does not yet pin the
   entering memory image, so the transition's starting point is bound only through the statement
   the program returns. A program must take `prev_root` from its inputs and refuse to return a
   statement whose `prev_root` it did not start from; the verifier cannot check that for it.
7. **Soundness posture.** VOS documents its STARK at about 96 bits of conjectured security, with its
   own list of what a proof does not guarantee. An independent review of the verifier should come
   before a Space guards real value.
8. **License.** `vos-pvm-proof-verifier`, `vos-pvm-proof` and `vos-pvm-proof-derive` are Apache-2.0,
   but `vos-pvm-proof` also pulls `vos-agent-sdk` and `vos-protocol`, which inherit VOS's workspace
   licence, AGPL-3.0-or-later. This pallet is GPL-3.0-only. With the `vos-verifier` feature on, a
   runtime links AGPL code; whether that is acceptable, or whether VOS can make the verifier's
   dependency graph Apache-2.0 throughout, needs an answer before a runtime enables it.

## 7. What remains for production

- **Weights.** Benchmark `register`, `anchor`, `refound` and `set_current_head` on reference
  hardware, and the verifier per pinned policy at the worst proof the runtime admits (the largest
  `MaxProofLen`, all chips active, the log-size cap). The current numbers are placeholders.
- **Proof size limits.** `MaxProofLen` should admit the pinned policy's worst proof and nothing larger
  (about 0.75 MB for STANDARD with every chip active; 1.3 MB for a canonical-shape MOBILE proof).
  Pin STANDARD unless proving cost forbids it: its proofs are 40 % smaller, so 40 % cheaper.
- **Verifier upgradeability.** The verifier is runtime code pinned to one VOS proof format; every
  format change needs a runtime upgrade. A runtime that must change it without stranding Spaces
  needs several backends side by side (a tuple of verifiers keyed by format) and a way to move a
  Space from one to the next, plus an emergency stop that refuses a backend with a known soundness
  bug.
- **The `wasm32v1-none` adjustments upstream**, so the `[patch]` on Stwo can go.
- **A published, versioned VOS verifier crate**, so this pallet can be released (it is
  `publish = false` while it depends on a git revision).
- **Benchmark helper for a real backend.** A precomputed proof cannot match a statement that names
  the benchmark chain's genesis; the helper needs either a prover at benchmark time or a fixed
  benchmark genesis.
