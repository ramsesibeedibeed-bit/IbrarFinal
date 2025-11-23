use anchor_lang::prelude::*;

pub const REFERRAL_ACCOUNT_PDA_SEED: &str = "referral";

#[account]
#[derive(Debug, InitSpace)]
pub struct ReferralAccount {
    pub bump: u8,
    pub config: Pubkey,
    pub referrer: Pubkey,
    // the owner/user who created this referral account (seed)
    pub owner: Pubkey,
    // accumulated pending lamports (SOL) for this referrer (from purchases)
    pub pending_lamports: u64,
}

impl ReferralAccount {
    pub const INIT_SPACE: usize = 1 + 32 + 32 + 32 + 8; // bump + config + referrer + owner + pending_lamports
}
