use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct AirdropState {
    pub bump: u8,
    /// merkle root (32 bytes)
    pub root: [u8; 32],
    /// unix expiry timestamp
    pub expiry: i64,
    /// claimed bitmap stored as bytes (bitset)
    pub claimed_bitmap: Vec<u8>,
}

impl AirdropState {
    // bump + root + expiry + vec len (4) + bitmap (let caller decide size)
    pub const INIT_SPACE: usize = 1 + 32 + 8 + 4 + 256;
}
