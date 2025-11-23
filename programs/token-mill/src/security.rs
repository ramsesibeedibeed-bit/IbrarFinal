use anchor_lang::prelude::*;

pub fn assert_pda(seeds: &[&[u8]], program_id: &Pubkey, account: &Pubkey) -> Result<()> {
    let (derived, _bump) = Pubkey::find_program_address(seeds, program_id);
    if derived != *account {
        return Err(error!(crate::errors::TokenMillError::InvalidMarketPda));
    }
    Ok(())
}

pub fn assert_owner(account_info: &AccountInfo, owner: &Pubkey) -> Result<()> {
    if account_info.owner != owner {
        return Err(error!(crate::errors::TokenMillError::InvalidAuthority));
    }
    Ok(())
}
