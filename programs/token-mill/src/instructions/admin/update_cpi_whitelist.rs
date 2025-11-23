use anchor_lang::prelude::*;
use crate::state::TokenMillConfig;
use crate::errors::TokenMillError;

#[derive(Accounts)]
pub struct UpdateCpiWhitelist<'info> {
    #[account(mut, has_one = authority @ TokenMillError::InvalidAuthority)]
    pub config: Account<'info, TokenMillConfig>,

    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<UpdateCpiWhitelist>, whitelist: Vec<Pubkey>, max_forwarded_accounts: u8) -> Result<()> {
    let cfg = &mut ctx.accounts.config;
    cfg.cpi_whitelist = whitelist;
    cfg.max_forwarded_accounts = max_forwarded_accounts;
    Ok(())
}
