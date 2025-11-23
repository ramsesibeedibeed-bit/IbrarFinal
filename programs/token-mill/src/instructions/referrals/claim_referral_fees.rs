use anchor_lang::prelude::*;
use anchor_lang::solana_program::{system_instruction};
use anchor_spl::token_2022 as token;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{events::TokenMillReferralFeeClaimEvent, ReferralAccount, REFERRAL_ACCOUNT_PDA_SEED};

#[event_cpi]
#[derive(Accounts)]
pub struct ClaimReferralFees<'info> {
    #[account(has_one = referrer)]
    pub referral_account: Account<'info, ReferralAccount>,

    pub quote_token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = quote_token_mint,
        associated_token::authority = referral_account,
        associated_token::token_program = quote_token_program
    )]
    pub referral_account_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = quote_token_mint,
        associated_token::authority = referrer,
        associated_token::token_program = quote_token_program
    )]
    pub referrer_quote_token_ata: InterfaceAccount<'info, TokenAccount>,

    pub referrer: Signer<'info>,
    pub quote_token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimReferralFees>) -> Result<()> {
    let mut distributed: u64 = 0;

    // Build referral PDA signer seeds
    let bump = ctx.accounts.referral_account.bump;
    let seeds: &[&[u8]] = &[
        REFERRAL_ACCOUNT_PDA_SEED.as_bytes(),
        ctx.accounts.referral_account.config.as_ref(),
        ctx.accounts.referral_account.owner.as_ref(),
        &[bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    // 1) Claim pending lamports (SOL) if any
    let pending_lamports = ctx.accounts.referral_account.pending_lamports;
    if pending_lamports > 0 {
        let ix = system_instruction::transfer(
            &ctx.accounts.referral_account.to_account_info().key(),
            &ctx.accounts.referrer.key(),
            pending_lamports,
        );
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.referral_account.to_account_info(),
                ctx.accounts.referrer.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer_seeds,
        )?;
        distributed = distributed.checked_add(pending_lamports).ok_or(error!(crate::errors::TokenMillError::MathOverflow))?;
        // clear pending
        ctx.accounts.referral_account.pending_lamports = 0u64;
    }

    // 2) Claim any quote-token balance from referral ATA -> referrer ATA
    // transfer tokens if the referral ATA has balance
    let from_amount = ctx.accounts.referral_account_quote_token_ata.amount;
    if from_amount > 0 {
        let cpi_accounts = token::transfer::Accounts {
            from: ctx.accounts.referral_account_quote_token_ata.to_account_info(),
            to: ctx.accounts.referrer_quote_token_ata.to_account_info(),
            authority: ctx.accounts.referral_account.to_account_info(),
        };
        let cpi_program = ctx.accounts.quote_token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        token::transfer(cpi_ctx, from_amount)?;
        // Note: distributed is lamports-focused; for tokens we don't add to distributed
    }

    emit_cpi!(TokenMillReferralFeeClaimEvent {
        referrer: ctx.accounts.referrer.key(),
        quote_token_mint: ctx.accounts.quote_token_mint.key(),
        fees_distributed: distributed,
    });

    Ok(())
}
