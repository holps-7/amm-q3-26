use anchor_lang::prelude::*;

use crate::{error::AmmError, state::Config};

#[derive(Accounts)]
pub struct Lock<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Account<'info, Config>,
}

impl<'info> Lock<'info> {
    pub fn lock(&mut self) -> Result<()> {
        self.require_authority()?;
        require!(!self.config.locked, AmmError::PoolLocked);
        self.config.locked = true;
        Ok(())
    }

    pub fn unlock(&mut self) -> Result<()> {
        self.require_authority()?;
        require!(self.config.locked, AmmError::DefaultError);
        self.config.locked = false;
        Ok(())
    }

    fn require_authority(&self) -> Result<()> {
        let authority = self.config.authority.ok_or(AmmError::NoAuthoritySet)?;
        require_keys_eq!(
            authority,
            self.authority.key(),
            AmmError::InvalidAuthority
        );
        Ok(())
    }
}
