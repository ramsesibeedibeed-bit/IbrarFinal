use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use crate::state::Market;
use crate::errors::TokenMillError;
use solana_program::program::invoke_signed;
use solana_program::instruction::Instruction;
use solana_program::pubkey::Pubkey;
use spl_token::instruction::AuthorityType as SplAuthorityType;

#[event]
pub struct AuthorityRevokedEvent {
    pub market: Pubkey,
    pub revoked_by: Pubkey,
}

#[derive(Accounts)]
pub struct RevokeAuthorities<'info> {
    #[account(mut, has_one = config @ TokenMillError::InvalidConfigAccount)]
    pub market: AccountLoader<'info, Market>,

    /// The base token mint whose authorities will be revoked
    #[account(mut)]
    pub base_mint: UncheckedAccount<'info>,

    /// The quote token mint whose authorities will be revoked
    #[account(mut)]
    pub quote_mint: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<RevokeAuthorities>) -> Result<()> {
    let mut market = ctx.accounts.market.load_mut()?;
    // restrict revocation to program-controlled mints; we record revocation flags
    // Build market PDA seeds so the program can sign for PDAs that were previously authorities
    let bump = market.bump;
    let seeds: &[&[u8]] = &[
        crate::state::MARKET_PDA_SEED.as_bytes(),
        market.base_token_mint.as_ref(),
        &[bump],
    ];

    // Revoke mint authority for base mint
    let base_mint_key = ctx.accounts.base_mint.key();
    let revoke_base_ix = spl_token::instruction::set_authority(
        &ctx.accounts.token_program.key(),
        &base_mint_key,
        None,
        SplAuthorityType::MintTokens,
        &ctx.accounts.market.key(),
        &[],
    )?;
    invoke_signed(
        &revoke_base_ix,
        &[
            ctx.accounts.base_mint.to_account_info(),
            ctx.accounts.market.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
        &[seeds],
    )?;

    // Revoke freeze authority for base mint
    let revoke_freeze_ix = spl_token::instruction::set_authority(
        &ctx.accounts.token_program.key(),
        &base_mint_key,
        None,
        SplAuthorityType::FreezeAccount,
        &ctx.accounts.market.key(),
        &[],
    )?;
    invoke_signed(
        &revoke_freeze_ix,
        &[
            ctx.accounts.base_mint.to_account_info(),
            ctx.accounts.market.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ],
        &[seeds],
    )?;

    // Also revoke for quote mint (if distinct)
    if ctx.accounts.quote_mint.key() != base_mint_key {
        let quote_mint_key = ctx.accounts.quote_mint.key();
        let revoke_quote_ix = spl_token::instruction::set_authority(
            &ctx.accounts.token_program.key(),
            &quote_mint_key,
            None,
            SplAuthorityType::MintTokens,
            &ctx.accounts.market.key(),
            &[],
        )?;
        invoke_signed(
            &revoke_quote_ix,
            &[
                ctx.accounts.quote_mint.to_account_info(),
                ctx.accounts.market.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            &[seeds],
        )?;

        let revoke_quote_freeze_ix = spl_token::instruction::set_authority(
            &ctx.accounts.token_program.key(),
            &quote_mint_key,
            None,
            SplAuthorityType::FreezeAccount,
            &ctx.accounts.market.key(),
            &[],
        )?;
        invoke_signed(
            &revoke_quote_freeze_ix,
            &[
                ctx.accounts.quote_mint.to_account_info(),
                ctx.accounts.market.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            &[seeds],
        )?;
    }

    market.mint_revoked = 1u8;
    market.freeze_revoked = 1u8;

    emit!(AuthorityRevokedEvent { market: ctx.accounts.market.key(), revoked_by: ctx.accounts.authority.key() });

    Ok(())
}
