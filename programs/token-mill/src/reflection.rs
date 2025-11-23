use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self as token, Transfer};
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

use crate::state::{ReflectionState, ReflectionLedger, Market, ExclusionList};
use crate::state::TokenMillConfig;
use crate::errors::TokenMillError;
use anchor_lang::prelude::Clock;
use crate::errors::TokenMillError;

#[derive(Accounts)]
pub struct ClaimReflection<'info> {
    #[account(mut)]
    pub reflection_state: Account<'info, ReflectionState>,

    #[account(mut, has_one = owner)]
    pub ledger: Account<'info, ReflectionLedger>,

    #[account(mut)]
    pub exclusion_list: Account<'info, ExclusionList>,

    #[account(mut, token::mint = market_base_mint, token::authority = market)]
    pub market_base_token_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, token::mint = market_base_mint, token::authority = user)]
    pub user_token_ata: InterfaceAccount<'info, TokenAccount>,

    pub market_base_mint: AccountInfo<'info>,

    #[account(mut)]
    pub market: AccountLoader<'info, Market>,

    pub owner: Signer<'info>,

    pub token_program: Program<'info, TokenInterface>,
}

pub fn handler(ctx: Context<ClaimReflection>) -> Result<u64> {
    // Check exclusion
    let excl = &ctx.accounts.exclusion_list;
    for pk in excl.excluded.iter() {
        if *pk == ctx.accounts.owner.key() {
            return Err(error!(TokenMillError::UnauthorizedMarket));
        }
    }

    // compute owed = user_balance * (per_share - last_per_share) / scale
    let reflection = ctx.accounts.reflection_state.load()?;
    let mut ledger = ctx.accounts.ledger.load_mut()?;
    let per_share = reflection.per_share;
    let scale = reflection.scale;
    let last = ledger.last_per_share;
    if per_share <= last { return Ok(0); }
    let delta = per_share.checked_sub(last).ok_or(error!(TokenMillError::MathOverflow))?;

    let user_balance = ctx.accounts.user_token_ata.amount as u128;
    let owed = user_balance.checked_mul(delta).ok_or(error!(TokenMillError::MathOverflow))?.checked_div(scale).ok_or(error!(TokenMillError::MathOverflow))?;
    if owed == 0 { return Ok(0); }

    // transfer owed tokens from market ATA to user ATA using market PDA as authority
    let bump = ctx.accounts.market.load()?.bump;
    let seeds: &[&[u8]] = &[crate::state::MARKET_PDA_SEED.as_bytes(), ctx.accounts.market_base_mint.key.as_ref(), &[bump]];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    let cpi_accounts = token::transfer::Accounts { 
        from: ctx.accounts.market_base_token_ata.to_account_info(),
        to: ctx.accounts.user_token_ata.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
    token::transfer(cpi_ctx, owed as u64)?;

    // update ledger
    ledger.last_per_share = per_share;

    Ok(owed as u64)
}

#[derive(Accounts)]
pub struct SettleReflection<'info> {
    #[account(mut, has_one = config @ TokenMillError::InvalidConfigAccount)]
    pub market: AccountLoader<'info, Market>,

    pub config: Account<'info, TokenMillConfig>,

    #[account(mut)]
    pub reflection_state: Account<'info, ReflectionState>,

    /// Only the config authority may trigger manual settlements
    pub authority: Signer<'info>,
}

/// Manually settle newly-accrued reflection tokens into the per-share accounting.
/// `added_tokens` should equal the amount of base tokens that were added to the
/// reflection pool (e.g. from buyback CPI result). The caller is responsible for
/// ensuring the token transfer to the market vault occurred (off-chain or via CPI).
pub fn settle_handler(ctx: Context<SettleReflection>, added_tokens: u64) -> Result<()> {
    let cfg = &ctx.accounts.config;
    if ctx.accounts.authority.key() != cfg.authority {
        return Err(error!(TokenMillError::InvalidAuthority));
    }

    let mut reflection = ctx.accounts.reflection_state.load_mut()?;
    let market = ctx.accounts.market.load()?;
    let total_supply = market.total_supply as u128;
    if total_supply == 0 { return Err(error!(TokenMillError::InvalidMarketState)); }

    // Increase the reflection pool and per-share
    reflection.total_reflection_pool = reflection.total_reflection_pool.checked_add(added_tokens).ok_or(error!(TokenMillError::MathOverflow))?;
    let scale: u128 = reflection.scale;
    let incr = (added_tokens as u128).checked_mul(scale).ok_or(error!(TokenMillError::MathOverflow))?.checked_div(total_supply).ok_or(error!(TokenMillError::MathOverflow))?;
    reflection.per_share = reflection.per_share.checked_add(incr).ok_or(error!(TokenMillError::MathOverflow))?;
    reflection.last_settlement = Clock::get()?.unix_timestamp;

    Ok(())
}
