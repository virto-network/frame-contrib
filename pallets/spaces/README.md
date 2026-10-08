# Spaces Pallet (`fc-pallet-spaces`)

A **Space** is an anchored, verifiable state machine: a program, named by the 32-byte commitment a
proof verifier recognises, and a gapless chain of state roots, each accepted only with a proof that
the program moved the Space from the previous root to it.

- `register(space, program, genesis)`: create a Space; the registering origin's account owns it.
- `anchor(space, number, root, proof, public)`: anchor the next root with a proof. Anyone may
  submit it: the proof is bound to a statement the pallet builds (chain, Space, epoch, number,
  previous root, new root). Resubmitting a stored anchor is a no-op.
- `refound(space, genesis)`: the owner starts the Space's program over from a new genesis, only
  while nothing is bound. The anchor sequence continues; a new epoch opens.
- `set_current_head(space, root)`: a privileged origin sets the Space's head, as
  `Paras::set_current_head` does for a parachain. Earlier anchors stay; binds carry over.
- `SpaceBinds`: other pallets bind value to a Space's anchored state, which stops `refound`.

The proof system sits behind `Config::Verifier: ProofVerifier`. Tests use `MockVerifier`. VOS's STARK
verifier (Circle STARK over M31, on Stwo; <https://codeberg.org/Virto/VOS>) is `vos::VosVerifier`,
behind the `vos-verifier` feature, and builds for `wasm32v1-none` with the `[patch]` on Stwo at the
workspace root.

See [`DESIGN.md`](./DESIGN.md) for how anchoring, binds and reset interact, the open questions, and
what remains before production (weights are placeholders).

## Tests

    cargo test -p fc-pallet-spaces                          # mock verifier
    cargo test -p fc-pallet-spaces --features vos-verifier  # plus a real VOS proof (fixtures/)
