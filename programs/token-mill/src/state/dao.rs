use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct ExclusionList {
    pub bump: u8,
    pub admin: Pubkey,
    pub excluded: Vec<Pubkey>,
}

impl ExclusionList {
    // approximate space: bump + admin + vec len (4) + 128*32
    pub const INIT_SPACE: usize = 1 + 32 + 4 + (32 * 128);
}
