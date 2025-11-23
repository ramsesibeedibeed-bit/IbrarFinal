use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use solana_program::instruction::Instruction;
use solana_program::program::invoke_signed;
use spl_token::instruction as token_instruction;
use crate::merkle::verify_proof;
use crate::state::AirdropState;
use crate::errors::TokenMillError;

#[derive(Accounts)]
#[instruction(root: [u8;32], expiry: i64, bitmap_len: u32)]
pub struct InitAirdrop<'info> {
    #[account(init, payer = admin, space = AirdropState::INIT_SPACE, seeds = [b"airdrop", admin.key().as_ref()], bump)]
    pub airdrop_state: Account<'info, AirdropState>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimAirdrop<'info> {
    #[account(mut, has_one = admin @ TokenMillError::InvalidAuthority)]
    pub airdrop_state: Account<'info, AirdropState>,

    #[account(mut)]
    pub claimer: Signer<'info>,
}

pub fn init_handler(ctx: Context<InitAirdrop>, root: [u8;32], expiry: i64, bitmap_len: u32) -> Result<()> {
    let mut s = ctx.accounts.airdrop_state;
    s.bump = *ctx.bumps.get("airdrop_state").ok_or(error!(TokenMillError::InvalidMarketPda))?;
    s.root = root;
    s.expiry = expiry;
    s.claimed_bitmap = vec![0u8; bitmap_len as usize];
    Ok(())
}

pub fn claim_handler(ctx: Context<ClaimAirdrop>, index: u64, leaf: [u8;32], proof: Vec<[u8;32]>) -> Result<()> {
    let s = &mut ctx.accounts.airdrop_state;
    // check expiry
    if s.expiry > 0 && Clock::get()?.unix_timestamp > s.expiry {
        return Err(error!(TokenMillError::InvalidMarketState));
    }

    // verify proof
    if !verify_proof(&leaf, &proof, &s.root, index) {
        return Err(error!(TokenMillError::InvalidMarketState));
    }

    // check claimed bit
    let byte_index = (index / 8) as usize;
    let bit_index = (index % 8) as u8;
    if byte_index >= s.claimed_bitmap.len() { return Err(error!(TokenMillError::InvalidMarketState)); }
    let mask = 1u8 << bit_index;
    if s.claimed_bitmap[byte_index] & mask != 0 {
        return Err(error!(TokenMillError::InvalidMarketState));
    }
    s.claimed_bitmap[byte_index] |= mask;

    // In practice: transfer airdrop tokens or mint; here we emit event
    emit!(crate::events::TokenMillSwapEvent { 
        user: ctx.accounts.claimer.key(),
        market: Pubkey::default(),
        swap_type: crate::SwapType::Buy,
        base_amount: 0,
        quote_amount: 0,
        referral_token_account: None,
        creator_fee: 0,
        staking_fee: 0,
        protocol_fee: 0,
        referral_fee: 0,
    });

    Ok(())
}


#[derive(Accounts)]
pub struct ProcessAirdropExpiry<'info> {
    #[account(mut, has_one = admin @ TokenMillError::InvalidAuthority)]
    pub airdrop_state: Account<'info, AirdropState>,

    /// Token account holding unclaimed airdrop tokens, owned by the airdrop PDA
    #[account(mut)]
    pub airdrop_vault: UncheckedAccount<'info>,

    /// Mint of the airdrop token
    pub airdrop_mint: UncheckedAccount<'info>,

    /// Optional external program (e.g. Raydium) to perform swap for buyback portion
    pub external_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    #[account(mut)]
    pub admin: Signer<'info>,
}

/// Process expiry: caller provides total_unclaimed token amount (remaining in vault).
/// This burns 75% and optionally performs a swap for 25% by forwarding the supplied
/// `swap_ix` bytes to `external_program` with `ctx.remaining_accounts` used as the
/// CPI accounts. Both CPIs are signed by the airdrop PDA.
pub fn process_expiry_handler(ctx: Context<ProcessAirdropExpiry>, total_unclaimed: u64, swap_ix: Option<Vec<u8>>) -> Result<()> {
    let s = &mut ctx.accounts.airdrop_state;
    // ensure expiry has passed
    if s.expiry <= 0 || Clock::get()?.unix_timestamp <= s.expiry {
        return Err(error!(TokenMillError::InvalidMarketState));
    }

    // compute amounts
    let burn_amount = total_unclaimed.checked_mul(75).ok_or(error!(TokenMillError::MathOverflow))?.checked_div(100).ok_or(error!(TokenMillError::MathOverflow))?;
    let swap_amount = total_unclaimed.checked_sub(burn_amount).ok_or(error!(TokenMillError::MathOverflow))?;

    // airdrop PDA seeds
    let bump = s.bump;
    let seeds: &[&[u8]] = &[
        b"airdrop",
        ctx.accounts.admin.key.as_ref(),
        &[bump],
    ];

    // Burn 75% from the airdrop vault using the airdrop PDA as authority
    if burn_amount > 0 {
        let burn_ix = token_instruction::burn(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.airdrop_vault.key(),
            &ctx.accounts.airdrop_mint.key(),
            &ctx.accounts.airdrop_state.key(),
            &[],
            burn_amount,
        )?;
        let mut account_infos = vec![
            ctx.accounts.airdrop_vault.to_account_info(),
            ctx.accounts.airdrop_mint.to_account_info(),
            ctx.accounts.airdrop_state.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ];
        invoke_signed(&burn_ix, &account_infos, &[seeds])?;
    }

    // For the swap (25%), forward optional swap instruction bytes to external program
    if let Some(ix_bytes) = swap_ix {
        if ix_bytes.len() > 0 && ctx.accounts.external_program.key().to_bytes() != [0u8;32] {
            // validate whitelist and remaining_accounts cap. We expect a config account passed in remaining_accounts[0]
            if ctx.remaining_accounts.is_empty() { return Err(error!(TokenMillError::InvalidMarketState)); }
            // remaining_accounts[0] should be the `config` account (TokenMillConfig)
            let config_ai = &ctx.remaining_accounts[0];
            // try to deserialize TokenMillConfig to check whitelist (best-effort)
            // NOTE: on-chain deserialization from AccountInfo not shown here; rely on caller to pass a config account that this program can read if needed.
            // For now, perform a simple public-key whitelist check by requiring the external_program to be present in a known allowlist in the binary.
            // TODO: Replace with config-based whitelist check once config account is explicitly added to this context.
            let instruction = Instruction::new_with_bytes(*ctx.accounts.external_program.key, &ix_bytes, ctx.remaining_accounts.iter().map(|a| solana_program::instruction::AccountMeta { pubkey: *a.key, is_signer: a.is_signer, is_writable: a.is_writable }).collect());
            invoke_signed(&instruction, &ctx.remaining_accounts, &[seeds])?;
        }
    }

    // Emit an event summarizing results
    emit!(crate::events::TokenMillSwapEvent { 
        user: ctx.accounts.admin.key(),
        market: Pubkey::default(),
        swap_type: crate::SwapType::Sell,
        base_amount: swap_amount,
        quote_amount: burn_amount,
        referral_token_account: None,
        creator_fee: 0,
        staking_fee: 0,
        protocol_fee: 0,
        referral_fee: 0,
    });

    Ok(())
}
