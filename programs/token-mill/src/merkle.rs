use anchor_lang::prelude::*;
use solana_program::keccak::{hashv};

pub fn verify_proof(leaf: &[u8], proof: &Vec<[u8;32]>, root: &[u8;32], index: u64) -> bool {
    let mut computed = hashv(&[leaf]).0;
    let mut idx = index;
    for p in proof.iter() {
        if (idx & 1) == 0 {
            // computed || p
            computed = hashv(&[&computed, p]).0;
        } else {
            computed = hashv(&[p, &computed]).0;
        }
        idx >>= 1;
    }
    &computed == root
}
