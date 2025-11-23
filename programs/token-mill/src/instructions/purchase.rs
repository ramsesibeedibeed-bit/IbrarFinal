use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};
use anchor_spl::token_2022::{self as token, MintTo};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    constant::PRICES_LENGTH,
    errors::TokenMillError,
    events::TokenMillSwapEvent,
    state::{Market, TokenMillConfig, MARKET_PDA_SEED},
    ReferralAccount,
    REFERRAL_ACCOUNT_PDA_SEED,
};
use crate::discount::compute_discount_bp;

#[event_cpi]
#[derive(Accounts)]
pub struct Purchase<'info> {
    pub config: Account<'info, TokenMillConfig>,

    #[account(mut, has_one = config @ TokenMillError::InvalidConfigAccount, has_one = base_token_mint @ TokenMillError::InvalidMintAccount)]
    pub market: AccountLoader<'info, Market>,

    pub base_token_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: market token account (may be unused for direct mint)
    #[account(mut)]
    pub market_base_token_ata: AccountInfo<'info>,

    #[account(mut, associated_token::mint = base_token_mint, associated_token::authority = buyer, associated_token::token_program = token_program)]
    pub buyer_base_token_ata: InterfaceAccount<'info, TokenAccount>,

    /// The recipient that will receive creator fees (lamports)
    #[account(mut)]
    pub creator: UncheckedAccount<'info>,

    /// Optional referral PDA (if present, referral fees are credited to this PDA)
    #[account(mut)]
    pub referral_account: Option<Account<'info, ReferralAccount>>,

    /// Optional referrer SOL account to receive referral share (fallback)
    #[account(mut)]
    pub referrer: Option<UncheckedAccount<'info>>,

    /// Protocol fee recipient (from config)
    #[account(mut, address = config.protocol_fee_recipient @ TokenMillError::InvalidAuthority)]
    pub protocol_fee_recipient: UncheckedAccount<'info>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    pub system_program: Program<'info, System>,

    pub token_program: Program<'info, token::Token2022>,
}

/// Simple purchase handler. This enforces a SOL transfer from buyer -> market PDA (treasury)
/// before minting base tokens to the buyer. It also immediately distributes fees (creator,
/// protocol, referral) from the market PDA to recipients. Pricing is derived from the
/// market's `ask_prices[0]` as a simple per-token price (placeholder bonding curve).
///
/// NOTE: This implementation makes the following assumptions (documented here):
/// - Fee shares are specified in basis points (parts per 10_000). If your config uses a
///   different scale, adapt the math accordingly.
/// - `market.ask_prices[0]` holds the price-per-token in the same quote units as SOL lamports.
///   In practice you should adapt `compute_price` to your bonding curve and decimals.
pub fn handler(
    ctx: Context<Purchase>,
    swap_amount_type: u8, // 0 = ExactInput (quote lamports), 1 = ExactOutput (base tokens)
    amount: u64,
) -> Result<(u64, u64)> {
    let mut market = ctx.accounts.market.load_mut()?;

    // Validate market PDA ownership: ensure the provided `market` is the expected PDA
    let (expected_market_pubkey, expected_bump) = Pubkey::find_program_address(
        &[MARKET_PDA_SEED.as_bytes(), ctx.accounts.base_token_mint.to_account_info().key.as_ref()],
        ctx.program_id,
    );
    if expected_market_pubkey != ctx.accounts.market.to_account_info().key() {
        return Err(error!(TokenMillError::InvalidMarketPda));
    }
    // bump consistency check
    if expected_bump != market.bump {
        return Err(error!(TokenMillError::InvalidMarketPda));
    }
    // Reject purchases after migration
    if market.is_migrated != 0 {
        return Err(error!(TokenMillError::MarketMigrated));
    }
    // Bonding curve pricing (linear curve): price(s) = base_price + width * s
    // Total cost to mint q tokens from supply s0: cost = base_price*q + width*(2*s0*q + q*q)/2
    // We'll use integer math with u128 to avoid overflow.
    let base_price: u128 = market.ask_prices[0] as u128; // price-per-token at supply 0
    let width: u128 = market.width_scaled as u128; // linear slope per token
    let s0: u128 = market.total_supply as u128;

    // helper: integer sqrt
    fn integer_sqrt(n: u128) -> u128 {
        if n <= 1 { return n; }
        let mut x0 = n / 2;
        let mut x1 = (x0 + n / x0) / 2;
        while x1 < x0 {
            x0 = x1;
            x1 = (x0 + n / x0) / 2;
        }
        x0
    }

    // Compute base_amount (tokens to mint) and quote_amount (lamports to charge)
    let (quote_amount, base_amount) = match swap_amount_type {
        1 => {
            // ExactOutput: amount is desired base tokens q
            let q = amount as u128;
            if q == 0 { return Err(error!(TokenMillError::InvalidAmount)); }
            // cost = base_price*q + width*(2*s0*q + q*q)/2
            let term1 = base_price.checked_mul(q).ok_or(error!(TokenMillError::MathOverflow))?;
            let term2_numer = width
                .checked_mul(2u128.checked_mul(s0).ok_or(error!(TokenMillError::MathOverflow))?.checked_mul(q).ok_or(error!(TokenMillError::MathOverflow))?
                    .checked_add(width.checked_mul(q.checked_mul(q).ok_or(error!(TokenMillError::MathOverflow))?).ok_or(error!(TokenMillError::MathOverflow))?).ok_or(error!(TokenMillError::MathOverflow))?)
                .ok_or(error!(TokenMillError::MathOverflow));
            // To avoid repeated complexity, compute term2 as width*(2*s0*q + q*q)/2 safely
            let two_s0_q = 2u128.checked_mul(s0).ok_or(error!(TokenMillError::MathOverflow))?.checked_mul(q).ok_or(error!(TokenMillError::MathOverflow))?;
            let q_sq = q.checked_mul(q).ok_or(error!(TokenMillError::MathOverflow))?;
            let numerator = width.checked_mul(two_s0_q.checked_add(q_sq).ok_or(error!(TokenMillError::MathOverflow))?).ok_or(error!(TokenMillError::MathOverflow))?;
            let term2 = numerator.checked_div(2u128).ok_or(error!(TokenMillError::MathOverflow))?;
            let cost = term1.checked_add(term2).ok_or(error!(TokenMillError::MathOverflow))?;
            // ensure cost fits u64 lamports (Solana lamports fit in u64)
            if cost == 0 { return Err(error!(TokenMillError::InvalidAmount)); }
            (cost as u64, q as u64)
        }
        0 => {
            // ExactInput: amount is quote lamports; need to solve quadratic for q
            let quote = amount as u128;
            if quote == 0 { return Err(error!(TokenMillError::InvalidAmount)); }
            // Solve a*q^2 + b*q - quote = 0 where:
            // a = width/2
            // b = width*s0 + base_price
            // For integer math, multiply equation by 2: (width)*q^2 + 2*(width*s0 + base_price)*q - 2*quote = 0
            let a = width; // corresponds to (width) after multiply-by-2
            let b = 2u128.checked_mul(width.checked_mul(s0).ok_or(error!(TokenMillError::MathOverflow))?.checked_add(base_price).ok_or(error!(TokenMillError::MathOverflow))?).ok_or(error!(TokenMillError::MathOverflow))?;
            let c = quote.checked_mul(2u128).ok_or(error!(TokenMillError::MathOverflow))?;

            // discriminant = b^2 + 4*a*c
            let disc = b.checked_mul(b).ok_or(error!(TokenMillError::MathOverflow))?.checked_add(4u128.checked_mul(a).ok_or(error!(TokenMillError::MathOverflow))?.checked_mul(c).ok_or(error!(TokenMillError::MathOverflow))?).ok_or(error!(TokenMillError::MathOverflow))?;
            let sqrt_disc = integer_sqrt(disc);
            // positive root: (-b + sqrt_disc) / (2a)
            if a == 0 {
                // width == 0 -> flat price: base_price
                let q = quote.checked_div(base_price).ok_or(error!(TokenMillError::MathOverflow))?;
                if q == 0 { return Err(error!(TokenMillError::InvalidAmount)); }
                return Ok((quote as u64, q as u64));
            }
            let numerator = if sqrt_disc > b { sqrt_disc - b } else { 0 };
            let denom = 2u128.checked_mul(a).ok_or(error!(TokenMillError::MathOverflow))?;
            let q = numerator.checked_div(denom).ok_or(error!(TokenMillError::MathOverflow))?; // floor
            if q == 0 { return Err(error!(TokenMillError::InvalidAmount)); }

            // compute cost for q tokens to ensure not exceeding quote (rounding)
            let term1 = base_price.checked_mul(q).ok_or(error!(TokenMillError::MathOverflow))?;
            let two_s0_q = 2u128.checked_mul(s0).ok_or(error!(TokenMillError::MathOverflow))?.checked_mul(q).ok_or(error!(TokenMillError::MathOverflow))?;
            let q_sq = q.checked_mul(q).ok_or(error!(TokenMillError::MathOverflow))?;
            let numerator2 = width.checked_mul(two_s0_q.checked_add(q_sq).ok_or(error!(TokenMillError::MathOverflow))?).ok_or(error!(TokenMillError::MathOverflow))?;
            let term2 = numerator2.checked_div(2u128).ok_or(error!(TokenMillError::MathOverflow))?;
            let cost = term1.checked_add(term2).ok_or(error!(TokenMillError::MathOverflow))?;

            if cost == 0 { return Err(error!(TokenMillError::InvalidAmount)); }
            (cost as u64, q as u64)
        }
        _ => return Err(error!(TokenMillError::InvalidSwapType)),
    };

    // Compute wallet-based discount (based on buyer's current lamports before transfer)
    let buyer_balance_before = ctx.accounts.buyer.to_account_info().lamports();
    let discount_bp = compute_discount_bp(buyer_balance_before);

    // Transfer SOL from buyer -> market PDA (treasury)
    let market_info = ctx.accounts.market.to_account_info();
    let market_key = market_info.key();
    let market_lamports_before = market_info.lamports();
    let ix = system_instruction::transfer(&ctx.accounts.buyer.key(), market_key, quote_amount);
    invoke(&ix, &[
        ctx.accounts.buyer.to_account_info(),
        market_info.clone(),
        ctx.accounts.system_program.to_account_info(),
    ])?;

    // Verify the market PDA received the SOL payment before minting tokens
    let market_lamports_after = ctx.accounts.market.to_account_info().lamports();
    if market_lamports_after != market_lamports_before.checked_add(quote_amount).ok_or(error!(TokenMillError::MathOverflow))? {
        return Err(error!(TokenMillError::InvalidMarketState));
    }

    // Emit payment confirmation event (before payouts/mint)
    emit_cpi!(crate::events::TokenMillPaymentEvent {
        user: ctx.accounts.buyer.key(),
        market: ctx.accounts.market.key(),
        quote_amount,
        base_amount,
    });

    // Fee calculations (basis points / 10_000)
    let bp_denom: u128 = 10_000;
    let quote_u128 = quote_amount as u128;
    let protocol_bp = ctx.accounts.config.default_protocol_fee_share as u128;
    let referral_bp = ctx.accounts.config.referral_fee_share as u128;

    // protocol fee total from the whole quote_amount
    let protocol_fee_total = quote_u128
        .checked_mul(protocol_bp)
        .ok_or(error!(TokenMillError::MathOverflow))?
        .checked_div(bp_denom)
        .ok_or(error!(TokenMillError::MathOverflow))?;

    // apply wallet-based discount to protocol fees
    let discount_amount = protocol_fee_total
        .checked_mul(discount_bp)
        .ok_or(error!(TokenMillError::MathOverflow))?
        .checked_div(bp_denom)
        .ok_or(error!(TokenMillError::MathOverflow))?;
    let protocol_fee_total_after_discount = protocol_fee_total.checked_sub(discount_amount).ok_or(error!(TokenMillError::MathOverflow))?;

    // referral is a portion of protocol fee after discount
    let referral_fee = protocol_fee_total_after_discount
        .checked_mul(referral_bp)
        .ok_or(error!(TokenMillError::MathOverflow))?
        .checked_div(bp_denom)
        .ok_or(error!(TokenMillError::MathOverflow))?;

    let protocol_fee_net = protocol_fee_total_after_discount.checked_sub(referral_fee).ok_or(error!(TokenMillError::MathOverflow))?;

    // creator fee from market.fees (shares are basis points of total). Staking logic removed.
    let creator_fee = quote_u128
        .checked_mul(market.fees.creator_fee_share as u128)
        .ok_or(error!(TokenMillError::MathOverflow))?
        .checked_div(bp_denom)
        .ok_or(error!(TokenMillError::MathOverflow))?;

    let staking_fee: u128 = 0u128; // staking removed — keep for backwards compatibility in events

    // Track pending creator fees inside market in lamports
    market.fees.pending_creator_fees = market
        .fees
        .pending_creator_fees
        .checked_add(creator_fee as u64)
        .ok_or(error!(TokenMillError::MathOverflow))?;

    // The market owns lamports; distribute protocol and referral immediately and keep creator staking pending
    // Build signer seeds for market PDA
    let bump = market.bump;
    let seeds: &[&[u8]] = &[
        MARKET_PDA_SEED.as_bytes(),
        ctx.accounts.base_token_mint.to_account_info().key.as_ref(),
        &[bump],
    ];

    // Payouts from market PDA: protocol_fee_net -> protocol_fee_recipient
    if protocol_fee_net > 0 {
        let ix = system_instruction::transfer(market_key, &ctx.accounts.protocol_fee_recipient.key(), protocol_fee_net as u64);
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                market_info.clone(),
                ctx.accounts.protocol_fee_recipient.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[seeds],
        )?;
    }

    // Referral payout: prefer crediting a referral PDA so referrer can claim later; otherwise pay referrer directly
    if referral_fee > 0 {
        // Preferred: credit referral PDA so referrer can claim later
        if let Some(referral_acct) = &mut ctx.accounts.referral_account {
            let ix = system_instruction::transfer(market_key, &referral_acct.to_account_info().key(), referral_fee as u64);
            let res = anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &[
                    market_info.clone(),
                    referral_acct.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[seeds],
            );
            if res.is_err() {
                // fallback to protocol fee recipient
                let ix2 = system_instruction::transfer(market_key, &ctx.accounts.protocol_fee_recipient.key(), referral_fee as u64);
                anchor_lang::solana_program::program::invoke_signed(
                    &ix2,
                    &[
                        market_info.clone(),
                        ctx.accounts.protocol_fee_recipient.to_account_info(),
                        ctx.accounts.system_program.to_account_info(),
                    ],
                    &[seeds],
                )?;
            } else {
                // update pending lamports bookkeeping on the referral account object
                referral_acct.pending_lamports = referral_acct
                    .pending_lamports
                    .checked_add(referral_fee as u64)
                    .ok_or(error!(TokenMillError::MathOverflow))?;
            }
        } else if let Some(referrer_acct) = &ctx.accounts.referrer {
            let ix = system_instruction::transfer(market_key, &referrer_acct.key(), referral_fee as u64);
            let res = anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &[
                    market_info.clone(),
                    referrer_acct.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[seeds],
            );
            if res.is_err() {
                // fallback: send to protocol fee recipient to avoid failing the entire tx
                let ix2 = system_instruction::transfer(market_key, &ctx.accounts.protocol_fee_recipient.key(), referral_fee as u64);
                anchor_lang::solana_program::program::invoke_signed(
                    &ix2,
                    &[
                        market_info.clone(),
                        ctx.accounts.protocol_fee_recipient.to_account_info(),
                        ctx.accounts.system_program.to_account_info(),
                    ],
                    &[seeds],
                )?;
            }
        } else {
            // fallback: send referral fee to protocol fee recipient (treasury)
            let ix2 = system_instruction::transfer(market_key, &ctx.accounts.protocol_fee_recipient.key(), referral_fee as u64);
            anchor_lang::solana_program::program::invoke_signed(
                &ix2,
                &[
                    ctx.accounts.market.to_account_info(),
                    ctx.accounts.protocol_fee_recipient.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[seeds],
            )?;
        }
    }

    // Creator payout: transfer creator_fee immediately to `creator` account.
    if creator_fee > 0 {
        let ix = system_instruction::transfer(market_key, &ctx.accounts.creator.key(), creator_fee as u64);
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                market_info.clone(),
                ctx.accounts.creator.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[seeds],
        )?;
    }

    // Mint base tokens to buyer using market PDA as mint authority
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    let cpi_accounts = token::mint_to::Accounts {
        mint: ctx.accounts.base_token_mint.to_account_info(),
        to: ctx.accounts.buyer_base_token_ata.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };

    let cpi_program = ctx.accounts.token_program.to_account_info();

    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
    token::mint_to(cpi_ctx, base_amount)?;

    // Emit event for swap/purchase
    emit_cpi!(TokenMillSwapEvent {
        user: ctx.accounts.buyer.key(),
        market: ctx.accounts.market.key(),
        swap_type: crate::SwapType::Buy,
        base_amount,
        quote_amount,
        referral_token_account: None,
        creator_fee: creator_fee as u64,
        staking_fee: staking_fee as u64,
        protocol_fee: protocol_fee_net as u64,
        referral_fee: referral_fee as u64,
    });

    Ok((base_amount, quote_amount))
}
