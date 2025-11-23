use anchor_lang::prelude::*;
use crate::state::{ReflectionState, BuybackState, Market};
use crate::errors::TokenMillError;
use solana_program::instruction::Instruction;
use solana_program::program::invoke_signed;

#[event]
pub struct BuybackEvent {
    pub market: Pubkey,
    pub lamports_spent: u64,
    pub tokens_bought: u64,
}

#[derive(Accounts)]
pub struct PerformBuyback<'info> {
    #[account(mut, has_one = config @ TokenMillError::InvalidConfigAccount)]
    pub market: AccountLoader<'info, Market>,

    /// Config account referenced by the market
    pub config: Account<'info, crate::state::TokenMillConfig>,

    #[account(mut)]
    pub reflection_state: Account<'info, ReflectionState>,

    #[account(mut)]
    pub buyback_state: Account<'info, BuybackState>,

    #[account(mut)]
    pub payer: Signer<'info>,

    /// Optional external DEX program to perform the SOL->token swap via CPI
    pub external_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
pub fn handler(ctx: Context<PerformBuyback>, lamports: u64, swap_ix: Option<Vec<u8>>) -> Result<()> {
    // If a swap instruction payload is provided, forward it as a CPI signed by market PDA.
    let market = ctx.accounts.market.load()?;
    let bump = market.bump;
    let signer_seeds: &[&[u8]] = &[
        crate::state::MARKET_PDA_SEED.as_bytes(),
        market.base_token_mint.as_ref(),
        &[bump],
    ];

    if let Some(ix_bytes) = swap_ix {
        if ix_bytes.len() > 0 && ctx.accounts.external_program.key().to_bytes() != [0u8;32] {
            // validate whitelist and remaining accounts cap
            if !ctx.accounts.config.cpi_whitelist.contains(&ctx.accounts.external_program.key()) {
                return Err(error!(TokenMillError::UnauthorizedMarket));
            }
            if ctx.remaining_accounts.len() > ctx.accounts.config.max_forwarded_accounts as usize {
                return Err(error!(TokenMillError::InvalidMarketState));
            }

            let instruction = Instruction::new_with_bytes(*ctx.accounts.external_program.key, &ix_bytes, ctx.remaining_accounts.iter().map(|a| solana_program::instruction::AccountMeta { pubkey: *a.key, is_signer: a.is_signer, is_writable: a.is_writable }).collect());
            invoke_signed(&instruction, &ctx.remaining_accounts, &[signer_seeds])?;

            // After successful CPI swap, caller is expected to credit the reflection pool via separate CPI or off-chain accounting.
            // Here we do not assume the token amount; instead, update buyback lamports and emit event with 0 tokens_bought (unknown).
            let mut bb = ctx.accounts.buyback_state.load_mut()?;
            bb.total_buyback_lamports = bb.total_buyback_lamports.checked_add(lamports).ok_or(error!(TokenMillError::MathOverflow))?;
            emit!(BuybackEvent { market: ctx.accounts.market.key(), lamports_spent: lamports, tokens_bought: 0 });
            return Ok(());
        }
    }

    // Fallback: simulate swap by converting lamports -> tokens using market.ask_prices[0]
    let price = market.ask_prices[0];
    if price == 0 { return Err(error!(TokenMillError::InvalidPrice)); }

    // tokens_bought = lamports / price
    let tokens_bought = lamports.checked_div(price).ok_or(error!(TokenMillError::MathOverflow))?;

    // Update reflection pool
    let mut reflection = ctx.accounts.reflection_state.load_mut()?;
    reflection.total_reflection_pool = reflection.total_reflection_pool.checked_add(tokens_bought).ok_or(error!(TokenMillError::MathOverflow))?;
    // per_share increase = tokens_bought * SCALE / total_supply
    let scale: u128 = reflection.scale;
    let total_supply = market.total_supply as u128;
    if total_supply > 0 {
        let incr = (tokens_bought as u128).checked_mul(scale).ok_or(error!(TokenMillError::MathOverflow))?.checked_div(total_supply).ok_or(error!(TokenMillError::MathOverflow))?;
        reflection.per_share = reflection.per_share.checked_add(incr).ok_or(error!(TokenMillError::MathOverflow))?;
    }

    // record buyback
    let mut bb = ctx.accounts.buyback_state.load_mut()?;
    bb.total_buyback_lamports = bb.total_buyback_lamports.checked_add(lamports).ok_or(error!(TokenMillError::MathOverflow))?;
    bb.total_buyback_tokens = bb.total_buyback_tokens.checked_add(tokens_bought).ok_or(error!(TokenMillError::MathOverflow))?;

    emit!(BuybackEvent { market: ctx.accounts.market.key(), lamports_spent: lamports, tokens_bought });

    Ok(())
}
