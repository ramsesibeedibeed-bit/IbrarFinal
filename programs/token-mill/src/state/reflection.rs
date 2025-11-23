use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct ReflectionState {
    pub bump: u8,
    /// total tokens reserved for reflection (in base token units)
    pub total_reflection_pool: u64,
    /// accumulated per-share (scaled by SCALE)
    pub per_share: u128,
    /// scale factor for per_share fixed-point
    pub scale: u128,
    /// last settlement timestamp
    pub last_settlement: i64,
}

impl ReflectionState {
    pub const INIT_SPACE: usize = 1 + 8 + 16 + 16 + 8; // bump + u64 + u128 + u128 + i64
}

#[account]
#[derive(Debug, InitSpace)]
pub struct ReflectionLedger {
    pub owner: Pubkey,
    pub last_per_share: u128,
}

impl ReflectionLedger {
    pub const INIT_SPACE: usize = 32 + 16;
}

#[account]
#[derive(Debug, InitSpace)]
pub struct BuybackState {
    pub bump: u8,
    pub total_buyback_lamports: u64,
    pub total_buyback_tokens: u64,
}

impl BuybackState {
    pub const INIT_SPACE: usize = 1 + 8 + 8;
}
