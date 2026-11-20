# Pitch Pallet

`fc-pallet-pitch` records time-boxed economic authority over H3 cells.

A pitch is a short-lived claim over the economic membrane of a place: it can be
publicly indexed by raw H3 cell, hidden behind a commitment, gated by a
membership proof, or sealed with only an opaque on-chain residue. The pallet
does not perform H3 geometry. Clients compute H3 cells off-chain and submit
either the raw `u64` cell or a commitment.

This first implementation focuses on the minimal consensus surface:

- claim, amend, join, dissolve, and reap a pitch;
- index public raw-cell pitches for cheap lookup;
- keep private pitches out of the raw-cell index;
- record selective disclosure grants without putting the disclosed secret
  on-chain;
- derive a deterministic sovereign account for each pitch.

Fee routing, VOS/Noir verification, and off-chain residue transport are intended
as runtime integrations layered on top of this base.
