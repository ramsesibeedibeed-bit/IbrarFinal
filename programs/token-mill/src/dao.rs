use anchor_lang::prelude::*;
use crate::state::ExclusionList;
use crate::errors::TokenMillError;

#[derive(Accounts)]
pub struct InitExclusion<'info> {
    #[account(init, payer = admin, space = ExclusionList::INIT_SPACE, seeds = [b"exclusion", admin.key().as_ref()], bump)]
    pub exclusion: Account<'info, ExclusionList>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateExclusion<'info> {
    #[account(mut, has_one = admin @ TokenMillError::InvalidAuthority)]
    pub exclusion: Account<'info, ExclusionList>,

    pub admin: Signer<'info>,
}

pub fn init_handler(ctx: Context<InitExclusion>) -> Result<()> {
    let mut excl = ctx.accounts.exclusion;
    excl.admin = ctx.accounts.admin.key();
    excl.bump = *ctx.bumps.get("exclusion").ok_or(error!(TokenMillError::InvalidMarketPda))?;
    Ok(())
}

pub fn update_handler(ctx: Context<UpdateExclusion>, add: bool, addr: Pubkey) -> Result<()> {
    let mut excl = ctx.accounts.exclusion;
    if add {
        if excl.excluded.len() >= 128 { return Err(error!(TokenMillError::InvalidMarket)); }
        if !excl.excluded.contains(&addr) {
            excl.excluded.push(addr);
        }
    } else {
        excl.excluded.retain(|x| *x != addr);
    }
    Ok(())
}
