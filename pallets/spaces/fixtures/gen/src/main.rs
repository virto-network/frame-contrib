//! Proves a fixture for `fc-pallet-spaces`'s real-verifier test, with VOS's own prover.
//!
//!     fc-spaces-fixture-gen <dir> <name> <output-hex>
//!
//! The guest is a straight-line PVM program in the shape of VOS's own io-binding benchmark guest
//! (`pvm/proof/verifier-wasm-bench`): a short ladder of `Add64` steps, then the four words of
//! `H = compute_io_hash(public, output)` assembled into registers φ9..φ12, where VOS's verifier
//! reads a program's public input/output hash, then `Trap`. `output` is the encoded
//! `AnchorStatement` the pallet expects, so the proof is bound to one anchor of one Space.
//!
//! It proves nothing about a state transition: the guest only commits to the statement. It
//! exercises the whole verification path (decoding, the io-hash binding, the program commitment
//! and the STARK) with a real proof.
//!
//! Writes `<name>.proof` (postcard `Proof`), `<name>.commitment` (the 32-byte program commitment),
//! `<name>.public` and `<name>.output`.

use std::path::Path;

use vos_pvm::instruction::Opcode;
use vos_pvm::interpreter::Interpreter;
use vos_pvm::PVM_REGISTER_COUNT;
use vos_pvm_proof::core::tracing::TracingPvm;
use vos_pvm_proof::{
    production_pcs_config, program_commitment_of_proof, prove_with_config, PcsPolicy, SideNote,
};
use vos_pvm_proof_verifier::{verify_standalone_with_options, Proof};

/// The public input every fixture binds.
const PUBLIC: &[u8] = b"fc-pallet-spaces real-verifier fixture";
/// `Add64` steps before the io-hash is assembled.
const LADDER: usize = 1019;
/// The log-size cap the test pins.
const MAX_LOG_SIZE: u32 = 18;

/// `blake2b_256(domain || parts..)`.
fn b2(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut s = blake2b_simd::Params::new().hash_length(32).to_state();
    s.update(domain);
    for p in parts {
        s.update(p);
    }
    s.finalize().as_bytes().try_into().unwrap()
}

/// VOS's `compute_io_hash`.
fn compute_io_hash(public: &[u8], output: &[u8]) -> [u8; 32] {
    let ph = b2(b"vos/zk/io-field", &[public]);
    let oh = b2(b"vos/zk/io-field", &[output]);
    b2(b"vos/zk/io", &[&ph, &oh])
}

fn side_note(h: [u8; 32]) -> SideNote {
    let mut code = Vec::new();
    let mut bitmask = Vec::new();
    let (mut ra, mut rb, mut rd): (u8, u8, u8) = (0, 1, 2);
    for _ in 0..LADDER {
        code.push(Opcode::Add64 as u8);
        code.push(ra | (rb << 4));
        code.push(rd);
        bitmask.extend_from_slice(&[1, 0, 0]);
        ra = (ra + 1) % 9;
        rb = (rb + 1) % 9;
        rd = (rd + 1) % 9;
    }
    const V: u8 = 6;
    const U: u8 = 7;
    const S: u8 = 8;
    for (i, word) in h.chunks_exact(8).enumerate() {
        let t = 9 + i as u8;
        let w = u64::from_le_bytes(word.try_into().unwrap());
        let (hi, lo) = ((w >> 32) as u32, w as u32);
        code.push(Opcode::LoadImm as u8);
        code.push(U);
        code.extend_from_slice(&hi.to_le_bytes());
        bitmask.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
        code.push(Opcode::ShloLImm64 as u8);
        code.push(S | (U << 4));
        code.push(32);
        bitmask.extend_from_slice(&[1, 0, 0]);
        code.push(Opcode::LoadImm as u8);
        code.push(V);
        code.extend_from_slice(&lo.to_le_bytes());
        bitmask.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
        code.push(Opcode::ShloLImm64 as u8);
        code.push(U | (V << 4));
        code.push(32);
        bitmask.extend_from_slice(&[1, 0, 0]);
        code.push(Opcode::ShloRImm64 as u8);
        code.push(V | (U << 4));
        code.push(32);
        bitmask.extend_from_slice(&[1, 0, 0]);
        code.push(Opcode::Add64 as u8);
        code.push(S | (V << 4));
        code.push(t);
        bitmask.extend_from_slice(&[1, 0, 0]);
    }
    code.push(Opcode::Trap as u8);
    bitmask.push(1);

    let mut regs = [0u64; PVM_REGISTER_COUNT];
    for (i, r) in regs.iter_mut().enumerate().take(13) {
        *r = (i as u64) + 1;
    }
    let pvm = Interpreter::new(
        code.clone(),
        bitmask.clone(),
        vec![],
        regs,
        vec![0u8; 64 * 1024],
        100_000_000,
        16,
    );
    let mut tracing = TracingPvm::new_conformance(pvm);
    let _exit = tracing.run();
    SideNote::new(tracing.into_trace(), code, bitmask)
}

fn prove(dir: &Path, name: &str, output: &[u8]) {
    let h = compute_io_hash(PUBLIC, output);
    let mut sn = side_note(h);
    let proof = prove_with_config(&mut sn, production_pcs_config()).expect("proving failed");
    assert_eq!(proof.public_io_hash(), h, "the io-hash window does not hold H");
    let max_log = proof.log_sizes.iter().copied().max().unwrap_or(0);
    assert!(max_log <= MAX_LOG_SIZE, "log size {max_log} above the cap");

    let commitment = program_commitment_of_proof(&proof);
    let bytes = postcard::to_allocvec(&proof).expect("postcard");
    let p: Proof = postcard::from_bytes(&bytes).expect("decode");
    verify_standalone_with_options(p, commitment, MAX_LOG_SIZE, &PcsPolicy::STANDARD)
        .expect("native verify failed");

    let commitment: [u8; 32] = commitment.into();
    std::fs::write(dir.join(format!("{name}.proof")), &bytes).unwrap();
    std::fs::write(dir.join(format!("{name}.commitment")), commitment).unwrap();
    std::fs::write(dir.join(format!("{name}.public")), PUBLIC).unwrap();
    std::fs::write(dir.join(format!("{name}.output")), output).unwrap();
    eprintln!("{name}: {} bytes, max log size {max_log}", bytes.len());
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&a[0]);
    std::fs::create_dir_all(&dir).unwrap();
    let name = a[1].clone();
    let output = hex::decode(&a[2]).expect("output hex");
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || prove(&dir, &name, &output))
        .unwrap()
        .join()
        .unwrap();
}
