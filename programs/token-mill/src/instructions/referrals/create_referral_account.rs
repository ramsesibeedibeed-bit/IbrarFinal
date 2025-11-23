use anchor_lang::prelude::*;

use crate::{ReferralAccount, TokenMillConfig, REFERRAL_ACCOUNT_PDA_SEED};

#[derive(Accounts)]
#[instruction(referrer: Pubkey)]
pub struct CreateReferralAccount<'info> {
    pub config: Account<'info, TokenMillConfig>,

    #[account(
        init,
        seeds = [REFERRAL_ACCOUNT_PDA_SEED.as_bytes(), config.key().as_ref(), user.key().as_ref()],
        bump,
        payer = user,
        space = 8 + ReferralAccount::INIT_SPACE
    )]
    pub referral_account: Account<'info, ReferralAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateReferralAccount>, referrer: Pubkey) -> Result<()> {
    let acct = &mut ctx.accounts.referral_account;
    // initialize fields
    acct.bump = *ctx.bumps.get("referral_account").ok_or(error!(crate::errors::TokenMillError::InvalidReferralPda))?;
    acct.config = ctx.accounts.config.key();
    acct.referrer = referrer;
    acct.owner = ctx.accounts.user.key();
    acct.pending_lamports = 0u64;
    Ok(())
}
