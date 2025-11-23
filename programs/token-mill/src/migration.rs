use anchor_lang::prelude::*;
use solana_program::instruction::Instruction;
use solana_program::program::invoke_signed;
use crate::state::{Market, BuybackState};
use crate::errors::TokenMillError;

#[event]
pub struct MigrationEvent {
    pub market: Pubkey,
    pub triggered_by: Pubkey,
    pub total_buyback_lamports: u64,
}

#[derive(Accounts)]
pub struct PerformMigration<'info> {
    #[account(mut, has_one = config @ TokenMillError::InvalidConfigAccount)]
    pub market: AccountLoader<'info, Market>,

    /// Config account referenced by the market
    pub config: Account<'info, crate::state::TokenMillConfig>,

    #[account(mut)]
    pub buyback_state: Account<'info, BuybackState>,

    #[account(mut)]
    /// The creator account to receive the creator payout
    pub creator: UncheckedAccount<'info>,

    pub authority: Signer<'info>,

    /// Optional: the Raydium program (or other on-chain program) to CPI for LP creation / burning.
    /// If present, instruction bytes for the target program may be provided as instruction args
    /// and this program will invoke them signed by the market PDA.
    pub external_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
pub fn handler(ctx: Context<PerformMigration>, force: bool, create_lp_ix: Option<Vec<u8>>, burn_lp_ix: Option<Vec<u8>>) -> Result<()> {
    let mut market = ctx.accounts.market.load_mut()?;
    let bb = ctx.accounts.buyback_state.load()?;

    // threshold: 60_000 SOL in lamports
    let threshold: u128 = 60_000u128.checked_mul(1_000_000_000u128).unwrap();
    let total_buyback = bb.total_buyback_lamports as u128;

    let ready = total_buyback >= threshold;
    if !ready && !force {
        return Err(error!(TokenMillError::InvalidMarketState));
    }

    // mark migrated
    market.is_migrated = 1u8;

    // Compute creator payout: pay any pending creator fees plus a fixed creator bonus (~200 SOL)
    let pending_creator = market.fees.pending_creator_fees as u128;
    let fixed_bonus: u128 = 200u128.checked_mul(1_000_000_000u128).ok_or(error!(TokenMillError::MathOverflow))?;
    let mut creator_payout_u128 = pending_creator.checked_add(fixed_bonus).ok_or(error!(TokenMillError::MathOverflow))?;

    // Cap payout to available lamports in market PDA to avoid failing the tx
    let market_lamports = ctx.accounts.market.to_account_info().lamports() as u128;
    if creator_payout_u128 > market_lamports {
        creator_payout_u128 = market_lamports; // pay as much as possible
    }

    let creator_payout: u64 = creator_payout_u128 as u64;
    if creator_payout > 0 {
        // zero out pending creator fees (or subtract what we paid)
        if creator_payout_u128 >= pending_creator {
            market.fees.pending_creator_fees = 0u64;
        } else {
            // partial payment: reduce pending creator fees
            let remaining = pending_creator.checked_sub(creator_payout_u128).ok_or(error!(TokenMillError::MathOverflow))?;
            market.fees.pending_creator_fees = remaining as u64;
        }

        // market PDA pays creator
        let bump = market.bump;
        let seeds: &[&[u8]] = &[
            crate::state::MARKET_PDA_SEED.as_bytes(),
            market.base_token_mint.as_ref(),
            &[bump],
        ];
        let ix = solana_program::system_instruction::transfer(&ctx.accounts.market.to_account_info().key(), &ctx.accounts.creator.key(), creator_payout);
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.market.to_account_info(),
                ctx.accounts.creator.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[seeds],
        )?;
    }

    // If the caller supplied Raydium / external program instructions (as raw bytes), we forward
    // them as CPIs signed by the market PDA. Validate the external program against the config
    // whitelist and cap the number of remaining accounts.
    let bump = market.bump;
    let signer_seeds: &[&[u8]] = &[
        crate::state::MARKET_PDA_SEED.as_bytes(),
        market.base_token_mint.as_ref(),
        &[bump],
    ];

    // Helper to forward an instruction payload to the provided external program id
    let forward_cpi = |prog: &AccountInfo, ix_bytes: Vec<u8>| -> Result<()> {
        if ix_bytes.is_empty() { return Ok(()); }
        let instruction = Instruction::new_with_bytes(*prog.key, &ix_bytes, ctx.remaining_accounts.iter().map(|a| solana_program::instruction::AccountMeta { pubkey: *a.key, is_signer: a.is_signer, is_writable: a.is_writable }).collect());
        invoke_signed(&instruction, &ctx.remaining_accounts, &[signer_seeds])?;
        Ok(())
    };

    // enforce whitelist
    if ctx.accounts.external_program.key().to_bytes() != [0u8;32] {
        if !ctx.accounts.config.cpi_whitelist.contains(&ctx.accounts.external_program.key()) {
            return Err(error!(TokenMillError::UnauthorizedMarket));
        }
        // cap remaining_accounts size
        if ctx.remaining_accounts.len() > ctx.accounts.config.max_forwarded_accounts as usize {
            return Err(error!(TokenMillError::InvalidMarketState));
        }
    }

    if let Some(ix_bytes) = create_lp_ix {
        if ctx.accounts.external_program.key().to_bytes() != [0u8;32] {
            forward_cpi(&ctx.accounts.external_program.to_account_info(), ix_bytes)?;
        }
    }

    if let Some(ix_bytes) = burn_lp_ix {
        if ctx.accounts.external_program.key().to_bytes() != [0u8;32] {
            forward_cpi(&ctx.accounts.external_program.to_account_info(), ix_bytes)?;
        }
    }

    emit!(MigrationEvent { market: ctx.accounts.market.key(), triggered_by: ctx.accounts.authority.key(), total_buyback_lamports: bb.total_buyback_lamports });

    Ok(())
}
