# Spaces Pallet (`fc-pallet-spaces`)

A **Space** is an anchored, verifiable state machine for groups that keep their structure and
systems private: its state lives off chain, in a VOS network of Actors founded on the Space's id,
and the chain holds its identity, its authority, its program and a gapless chain of state roots,
each accepted only with a proof that the program moved the Space from the previous root to it.
Public, auditable groups keep using Communities and Contracts.

- `register(authority, program, genesis)`: create a Space with the next id (emitted in
  `Registered`), governed by `authority` (any origin, a community's included).
- `anchor(space, number, root, proof, public)`: anchor the next root with a proof. Anyone may
  submit it: the proof is bound to a statement the pallet builds (chain, Space id, epoch, number,
  previous root, new root). Resubmitting a stored anchor is a no-op.
- `set_program(space, program)`: the authority changes the program from the next anchor on; past
  commitments are kept with the anchors they applied to.
- `refound(space, genesis)`: the authority starts the Space's program over from a new genesis, only
  while nothing is bound. The anchor sequence continues; a new epoch opens.
- `set_current_head(space, root)`: a privileged origin sets the Space's head, as
  `Paras::set_current_head` does for a parachain. Earlier anchors stay; binds carry over.
- `set_authority(space, authority)`: the authority, or the privileged origin, hands the Space on.
- `SpaceBinds`: other pallets bind value to a Space's anchored state, which stops `refound`.

The proof system sits behind `Config::Verifier: ProofVerifier`. This crate ships only
`MockVerifier`, for tests and benchmarks; a real backend implements the trait with its proof
system. The backend for VOS's STARK verifier (Circle STARK over M31, on Stwo) lives in the VOS
repository, <https://codeberg.org/Virto/VOS>.

See [`DESIGN.md`](./DESIGN.md) for Spaces and Communities, how anchoring, binds and reset interact,
a migration path from a Community to a Space, the open questions, and what remains before
production (weights are placeholders).

## Tests

    cargo test -p fc-pallet-spaces
    cargo test -p fc-pallet-spaces --features runtime-benchmarks
