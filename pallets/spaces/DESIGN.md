# `fc-pallet-spaces`: design note

**Status: draft.** The pallet builds and is tested with a mock verifier. A backend for VOS's STARK
verifier lives on the VOS side (§5). Weights are placeholders and the open questions below are
unresolved.

## 1. Spaces and Communities

Kreivo already has Communities: public organisations whose members, tracks, referenda and treasury
live on chain, and which deploy their programs as contracts. Everything about a community is
visible and auditable by anyone, and that is the point of one.

A **Space** is the counterpart for groups that keep their structure and systems private: companies,
organisations, any group whose members, rules and records are not meant to be public. Its state
lives off chain, in a VOS network of Actors founded for it; the chain holds only what everyone must
agree on: the Space's identity, who governs it, which program it runs, and a gapless sequence of
state roots, each accepted only with a proof that the program produced it.

| | Communities + Contracts | Spaces + VOS Actors |
|---|---|---|
| Who it suits | Groups that want to be fully public and auditable | Groups that keep their structure and systems private |
| Membership | On chain (memberships, ranks) | Off chain, inside the Space's state |
| Programs | Contracts, executed by the chain | VOS Actors, executed off chain; the chain checks proofs of their transitions |
| What the chain sees | Everything | Roots, proofs' public inputs (opaque bytes), and the Space's governance |
| Governance | The community's admin origin and decision method | The Space's authority (any origin, a community's included) |

Spaces will partly supersede Communities: a community that wants privacy can move to a Space (§8);
a community that wants to stay public keeps using Communities and Contracts. Neither replaces the
other.

## 2. What a Space is

- an **id**, assigned in sequence when the Space is registered and never reused. **It is the Space's
  identity**: the VOS network founded for the Space is initiated with it, and every anchor statement
  names it (§3), so a proof made in one Space's network cannot anchor another's;
- an **authority**: the origin that governs it. Like a community's admin origin it can be any origin
  the runtime has: an account, a collective, a community's own origin. One authority may govern
  several Spaces, so the authority is stored with the Space rather than as a reverse map;
- a **program**, the 32-byte commitment the proof verifier recognises (for VOS, the program's
  preprocessed-trace Merkle root). It is an **attribute** of the Space, not its identity: it can be
  switched (by the authority while nothing is bound, by the reset origin otherwise), and every past
  commitment is kept with the first anchor it applied to;
- an **account**, derived from the id as a community's account is (the pallet id's sub-account for
  the Space id): deterministic, existing from registration on, so the Space can hold and spend funds;
- an **origin** of its own, with which the Space acts as itself, and `EnsureSpace`, with which other
  pallets accept it (as `EnsureCommunity` accepts a community's origin);
- a **head**: the root the next transition starts from, the number of the last anchor, and the
  current epoch;
- a gapless sequence of **anchors**, each a state root the chain accepted only with a proof that the
  Space's program moved it from the head's root to that root;
- **epochs**, each recording the root it starts from and the anchor number it starts after;
- **binds**, made by other pallets, recording that something of value now depends on the Space's
  anchored state.

| Call | Origin | Does |
|---|---|---|
| `register(authority, program, genesis)` | `CreateOrigin` | Creates the Space with the next id (in `Registered`), at `genesis`, epoch 0, program version 0 |
| `anchor(space, number, root, proof, public)` | Any signed origin | Verifies the proof against the current program, stores anchor `number`, moves the head to `root` |
| `set_program(space, program)` | The authority while nothing is bound; `ResetOrigin` always | The next anchor on must be a proof of `program`; the previous commitment stays in history |
| `refound(space, genesis)` | The authority | Only with no binds: opens a new epoch at `genesis`; the sequence continues |
| `set_current_head(space, root)` | `ResetOrigin` | Opens a new epoch at `root` whatever the Space holds; the sequence continues |
| `set_authority(space, authority)` | The authority, or `ResetOrigin` | Hands the Space on (the reset origin can recover a lost authority) |
| `dispatch_as_space(space, call)` | The authority | Dispatches `call` with the Space's origin |
| `dispatch_as_account(space, call)` | The authority | Dispatches `call` signed by the Space's account: how the Space spends its funds |
| `SpaceBinds::{bind, unbind, is_bound}` | Other pallets (a trait, no extrinsic) | Records or releases a bind |

| Storage | Key | Value |
|---|---|---|
| `NextSpaceId` | | The id the next Space takes |
| `Spaces` | Space | Authority, current program and its version, bind count |
| `Programs` | (Space, version) | Commitment, first anchor it applies to, block. Never pruned |
| `Heads` | Space | Epoch, last anchor number, root, block |
| `Anchors` | (Space, number) | Root, epoch, program version, block. Never pruned or overwritten |
| `Epochs` | (Space, epoch) | Starting root, base number, block, cause (`Registered`, `Refounded`, `Reset`) |
| `Binds` | (Space, key) | Epoch, last anchor at the time, block |

## 3. What an anchor proves

The pallet builds the statement itself and passes its encoding to the verifier as the program's
**output**:

```rust
AnchorStatement {
    domain: *b"fc-spaces/anchor",
    version: 1,
    chain,      // the genesis hash: no replay across chains
    space,      // the Space id: the identity its VOS network is initiated with
    epoch,      // the head's
    number,     // the head's number + 1
    prev_root,  // the head's root: where the transition starts
    root,       // the anchored root: where it ends
}
```

The verifier must accept a proof only if it binds both the submitted public input and this output.
VOS's backend does it with VOS's convention: a program leaves
`H = blake2b("vos/zk/io" ‖ blake2b("vos/zk/io-field" ‖ public) ‖ blake2b("vos/zk/io-field" ‖ output))`
in its registers at halt, and the proof carries `H`. So a proof verifies only for the Space, epoch,
number and roots it was made for, and the submitter's public input stays opaque to the chain.

Consequences:

- **Anyone may submit an anchor.** The proof is the authority; a relayer cannot change what it
  anchors, only whether it is submitted. The submitter pays.
- **No replay.** A proof names its Space, its anchor number and its starting root; once that number
  is stored, the same proof is a no-op (the stored root matches) and any other proof for that number
  is an `AnchorConflict`. Resubmitting a stored anchor succeeds without verifying, at the cost of
  three reads, so a submitter can retry safely.
- **A moved head invalidates outstanding proofs.** An anchor, a re-founding or a reset changes the
  number, the root or the epoch, and every proof made against the old head fails as `NotBound`.
- **A changed program invalidates outstanding proofs** of the old one (`WrongProgram`). The program
  is not in the statement: the verifier already checks the proof against the Space's current
  commitment, and each anchor records which version that was.

## 4. Anchoring, binds and reset

| | Anchors already stored | The anchor sequence | Proofs made against the old head | Binds |
|---|---|---|---|---|
| `anchor` | Unchanged | Advances by one | Fail (`NotBound`, or a no-op if it is this very anchor) | Unchanged |
| `set_program` (authority, or privileged with binds) | Unchanged, with their program version | Unchanged | Fail (`WrongProgram`) | **Must be none** for the authority (`HasBinds`); carried over when the reset origin switches |
| `refound` (authority) | Unchanged, under their epoch | Continues: the next anchor is `base + 1` | Fail | **Must be none**, or it is refused (`HasBinds`) |
| `set_current_head` (privileged) | Unchanged, under their epoch | Continues: the next anchor is `base + 1` | Fail | **Carried over**, counted in the `HeadSet` event |

- **Continuation while nothing is bound.** A Space's program may need to start over from a new
  genesis (a re-founded state machine, a migrated state). While nothing of value depends on its
  anchored state, the authority can do that alone with `refound`: no vote, and no power over anyone
  else's value. The sequence continues, so every anchor ever made stays where it is and numbers are
  never reused; the epoch tells a reader which run an anchor belongs to.
- **Once something is bound, the authority cannot move the head off its anchored chain.** It can
  still anchor, and it can release binds through the pallet that made them. A state machine that
  must start over after value is bound either takes a new Space (and moves its value through that
  value's own rules) or asks the reset origin.
- **Once something is bound, the program is the rules that value depends on**, so switching it needs
  the reset origin too. Without binds, the authority switches freely, for example to follow a new
  proof format or a fixed program.
- **`set_current_head` is the escape hatch**, the analogue of `Paras::set_current_head`, with which a
  relay chain's governance sets a parachain's head: the earlier heads stay in history, and the chain
  continues from the new one. Here the earlier anchors stay, a new epoch opens at the given root,
  and binds carry over: the reset origin answers for the new root accounting for whatever is bound.
  It cannot rewrite or erase an anchor; it can only choose where the next one starts.
- **Why numbers continue instead of restarting at 1.** Restarting would key anchors by
  (epoch, number) and let two anchors share a number; continuing keeps one total order per Space
  and makes "never overwritten" a property of the storage key. A reader that wants per-run numbers
  subtracts the epoch's `base`.

What the pallet cannot see, and the authority must check before `refound`: that no movement of value
is in flight in another pallet that has not made a bind, and that whoever submitted anchors for the
previous run has stopped.

## 5. Proof systems: the trait here, the backends with their proof systems

frame-contrib holds the Substrate side only: the pallet, the `ProofVerifier` trait and a
`MockVerifier` for tests and benchmarks. A real backend implements `ProofVerifier` in its own
repository, depends on this crate as the source of truth for the trait, and is licensed as its
proof system is. VOS's backend lives in the VOS repository (<https://codeberg.org/Virto/VOS>), next
to its STARK verifier (`vos-pvm-proof-verifier`: Circle STARK over M31, on StarkWare's Stwo), with
its real-proof tests and the adjustments it needs to build for a runtime's `wasm32v1-none` target.
A runtime that wants it adds that crate and sets `Config::Verifier` to it.

What a backend must do, whatever its proof system:

- refuse cheaply first: decoding, then the input/output binding, then the program commitment, and
  only then the expensive verification (a proof of another program should cost its decoding, not a
  verification);
- verify in dispatch, never in `validate`: a STARK takes tens of milliseconds, which would be unpaid
  work in the transaction pool;
- report its cost through `ProofVerifier::weight`, without declaring the proof's bytes as proof size:
  `CheckWeight` already counts the extrinsic's length as PoV, and declaring it again halves how many
  proofs fit a block.

## 6. Where this comes from

It grew out of a feasibility study of attaching VOS's general-purpose STARK verifier to a Kreivo
pallet, and of a spike that ran that verifier inside a minimal Substrate runtime on the `stable2606`
crates. Carried over: the verifier composition and its order of checks (§5), the `wasm32v1-none`
adjustments, verification in dispatch, the weight envelope, binding what the chain cares about into
the program's output, and a trait seam for the proof system. Adapted: one pallet instead of a Spaces
pallet beside a verifier pallet with a program registry, call hooks and notices; the proof gates the
anchor, so anyone may submit it; and epochs, re-founding, binds, reset, sequential ids, the
authority and the program history are new.

## 7. Measured cost (for orientation, not weights)

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

## 8. Migrating a Community to a Space (sketch)

For a community that wants to become private. Nothing here is implemented; it is the path the
pallet is shaped to allow.

1. **Register the Space under the community.** The community's admin origin (through its usual
   decision method) registers a Space whose authority is the community's own origin. The Space gets
   a new id from the sequence; the community keeps its id. From here the community governs the
   Space exactly as it governs anything else.
2. **Found the VOS network** with the new Space id, running a program that imports the community's
   state (members, ranks, records) and commits to it. Its root is the Space's genesis; if the
   import must be redone, the authority re-founds the Space (`refound`) while nothing is bound.
3. **Move value.** The community, through its own account, transfers its assets to the Space's
   account (`space_account`), and the pallet that accounts for them binds them to the Space. From
   then on the Space's head, and its program, move only by proof or by the reset origin; the Space
   spends with `dispatch_as_account`.
4. **Hand over governance.** Optionally, `set_authority` moves the Space from the community's origin
   to one the Space's own members control. The community can stay as a public face, or be wound
   down by its own rules.

Missing pieces: a record that a Space was migrated from a given community, and a decision on whether
on-chain memberships are cleared once they live in the Space.

## 9. Open questions

1. **Who may submit anchors.** Anyone, today. Should a Space be able to restrict submission (to its
   authority, or to a set of keys), for example to control who pays?
2. **Binds on reset.** `set_current_head` keeps every bind. Should it instead require none, or take an
   explicit list of binds to carry, or notify the pallets that made them?
3. **Registration deposit.** Registration takes no deposit; a runtime must restrict `CreateOrigin`
   until it does. A `Consideration` (as other frame-contrib pallets hold) is the likely shape.
4. **The reset origin's reach.** It can set a Space's head, switch its program with binds, and change
   its authority. Should recovering an authority be a separate, narrower origin?
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

## 10. What remains for production

- **Weights.** Benchmark every call on reference hardware, and each backend per pinned policy at the
  worst proof the runtime admits. The current numbers are placeholders.
- **Proof size limits.** `MaxProofLen` should admit the pinned policy's worst proof and nothing larger
  (about 0.75 MB for STANDARD with every chip active; 1.3 MB for a canonical-shape MOBILE proof).
- **Verifier upgradeability.** A backend is runtime code pinned to one proof format; every format
  change needs a runtime upgrade, and Spaces move to the new format with `set_program`. A runtime
  that must change backends without stranding Spaces needs several side by side (keyed by format)
  and an emergency stop that refuses a backend with a known soundness bug.
- **Benchmark helper for a real backend.** A precomputed proof cannot match a statement that names
  the benchmark chain's genesis; the helper needs either a prover at benchmark time or a fixed
  benchmark genesis.
