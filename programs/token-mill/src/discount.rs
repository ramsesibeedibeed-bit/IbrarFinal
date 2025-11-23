use anchor_lang::prelude::*;

// Wallet-based discount tiers (thresholds in lamports)
const ONE_SOL: u64 = 1_000_000_000u64;
const TIER_THRESHOLDS: [u64; 3] = [ONE_SOL, 10 * ONE_SOL, 50 * ONE_SOL];
// Corresponding discounts in basis points (parts per 10_000)
const TIER_DISCOUNTS_BP: [u128; 4] = [0u128, 1_000u128, 2_500u128, 5_000u128];

pub fn compute_discount_bp(wallet_lamports: u64) -> u128 {
    if wallet_lamports >= TIER_THRESHOLDS[2] {
        return TIER_DISCOUNTS_BP[3];
    }
    if wallet_lamports >= TIER_THRESHOLDS[1] {
        return TIER_DISCOUNTS_BP[2];
    }
    if wallet_lamports >= TIER_THRESHOLDS[0] {
        return TIER_DISCOUNTS_BP[1];
    }
    TIER_DISCOUNTS_BP[0]
}
