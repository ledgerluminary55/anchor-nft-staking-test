use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::token;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Approve, Mint, MintTo, Revoke, Token, TokenAccount},
};

declare_id!("2pE13XRXtstNEuBZ912ooGAnTQhabLYm57cFJW7tQXvK");

#[program]
pub mod anchor_nft_staking {
    use super::*;

    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        msg!("Inside Anchor version of staking program");
        // Getting clock directly
        let clock = Clock::get().unwrap();

        // CPI to Token program to approve delegation
        let cpi_approve_program = ctx.accounts.token_program.to_account_info();
        let cpi_approve_accounts = Approve {
            to: ctx.accounts.nft_token_account.to_account_info(),
            delegate: ctx.accounts.program_authority.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_approve_ctx = CpiContext::new(cpi_approve_program, cpi_approve_accounts);
        msg!("CPI to approve ix on token program");
        token::approve(cpi_approve_ctx, 1)?;

        let authority_bump = *ctx.bumps.get("program_authority").unwrap();
        msg!("CPI to invoke freeze ix on token program");
        invoke_signed(
            &mpl_token_metadata::instruction::freeze_delegated_account(
                ctx.accounts.metadata_program.key(),
                ctx.accounts.program_authority.key(),
                ctx.accounts.nft_token_account.key(),
                ctx.accounts.nft_edition.key(),
                ctx.accounts.nft_mint.key(),
            ),
            &[
                ctx.accounts.program_authority.to_account_info(),
                ctx.accounts.nft_token_account.to_account_info(),
                ctx.accounts.nft_edition.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.metadata_program.to_account_info(),
            ],
            &[&[b"authority", &[authority_bump]]],
        )?;

        // if ctx.accounts.stake_state.is_initialized {
        //     msg!("Account already initialized");
        //     return err!(StakeError::AccountAlreadyInitialized);
        // }

        ctx.accounts.stake_state.token_account = ctx.accounts.nft_token_account.key();
        ctx.accounts.stake_state.user_pubkey = ctx.accounts.user.key();
        ctx.accounts.stake_state.stake_state = StakeState::Staked;
        ctx.accounts.stake_state.stake_start_time = clock.unix_timestamp;
        ctx.accounts.stake_state.last_stake_redeem = clock.unix_timestamp;
        ctx.accounts.stake_state.is_initialized = true;

        msg!(
            "NFT token account: {:?}",
            ctx.accounts.stake_state.token_account
        );
        msg!("User pubkey: {:?}", ctx.accounts.stake_state.user_pubkey);
        msg!("Stake state: {:?}", ctx.accounts.stake_state.stake_state);
        msg!(
            "Stake start time: {:?}",
            ctx.accounts.stake_state.stake_start_time
        );
        msg!(
            "Time since last redeem: {:?}",
            ctx.accounts.stake_state.last_stake_redeem
        );

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        if !ctx.accounts.stake_state.is_initialized {
            msg!("Account is not initialized");
            return err!(StakeError::UnintializedAccount);
        }

        if ctx.accounts.stake_state.stake_state != StakeState::Staked {
            msg!("Stake account is not staking anything");
            return err!(StakeError::InvalidStakeState);
        }

        // Thaw NFT token account
        msg!("Thawing NFT token account...");
        let delegate_bump = *ctx.bumps.get("program_authority").unwrap();
        invoke_signed(
            &mpl_token_metadata::instruction::thaw_delegated_account(
                ctx.accounts.metadata_program.key(),
                ctx.accounts.program_authority.key(),
                ctx.accounts.nft_token_account.key(),
                ctx.accounts.nft_edition.key(),
                ctx.accounts.nft_mint.key(),
            ),
            &[
                ctx.accounts.program_authority.to_account_info(),
                ctx.accounts.nft_token_account.to_account_info(),
                ctx.accounts.nft_edition.to_account_info(),
                ctx.accounts.nft_mint.to_account_info(),
                ctx.accounts.metadata_program.to_account_info(),
            ],
            &[&[b"authority", &[delegate_bump]]],
        )?;

        msg!("Revoking delegation...");
        // CPI to Token program to revoke delegation of nft token account
        let cpi_revoke_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Revoke {
            source: ctx.accounts.nft_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_revoke_ctx = CpiContext::new(cpi_revoke_program, cpi_accounts);
        msg!("CPI to revoke ix on token program");
        token::revoke(cpi_revoke_ctx)?;

        ctx.accounts.stake_state.stake_state = StakeState::Unstaked;

        msg!(
            "NFT token account: {:?}",
            ctx.accounts.stake_state.token_account
        );
        msg!("User pubkey: {:?}", ctx.accounts.stake_state.user_pubkey);
        msg!("Stake state: {:?}", ctx.accounts.stake_state.stake_state);
        msg!(
            "Stake start time: {:?}",
            ctx.accounts.stake_state.stake_start_time
        );
        msg!(
            "Time since last redeem: {:?}",
            ctx.accounts.stake_state.last_stake_redeem
        );

        Ok(())
    }

    pub fn redeem(ctx: Context<Redeem>) -> Result<()> {
        // Getting clock directly
        let clock = Clock::get()?;
        if !ctx.accounts.stake_state.is_initialized {
            msg!("Account is not initialized");
            return err!(StakeError::UnintializedAccount);
        }

        if ctx.accounts.stake_state.stake_state != StakeState::Staked {
            msg!("Stake account is not staking anything");
            return err!(StakeError::InvalidStakeState);
        }

        msg!(
            "Stake last redeem: {:?}",
            ctx.accounts.stake_state.last_stake_redeem
        );
        msg!("Current time: {:?}", clock.unix_timestamp);
        let unix_time = clock.unix_timestamp - ctx.accounts.stake_state.last_stake_redeem;
        msg!("Seconds since last redeem: {}", unix_time);
        let redeem_amount = 1000000 * unix_time;
        msg!("Elligible redeem amount in lamports: {}", redeem_amount);

        // CPI to Token program to mint tokens
        let auth_bump = *ctx.bumps.get("stake_authority").unwrap();
        let seeds = &[b"mint".as_ref(), &[auth_bump]];
        let signer = &[&seeds[..]];
        let cpi_approve_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = MintTo {
            mint: ctx.accounts.stake_mint.to_account_info(),
            to: ctx.accounts.user_stake_ata.to_account_info(),
            authority: ctx.accounts.stake_authority.to_account_info(),
        };
        let cpi_approve_ctx =
            CpiContext::new_with_signer(cpi_approve_program, cpi_accounts, signer);
        msg!("CPI to mint ix on token program");
        token::mint_to(cpi_approve_ctx, redeem_amount.try_into().unwrap())?;
        ctx.accounts.stake_state.last_stake_redeem = clock.unix_timestamp;
        msg!(
            "Updated last stake time: {:?}",
            ctx.accounts.stake_state.last_stake_redeem
        );

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = user,)]
    pub nft_token_account: Account<'info, TokenAccount>,
    pub nft_mint: Account<'info, Mint>,
    /// CHECK: THis is not dangerous because we don't read or write from this account
    pub nft_edition: AccountInfo<'info>,
    #[account(
        init_if_needed,
        seeds = [user.key().as_ref(), nft_token_account.key().as_ref()],
        bump,
        payer = user,
        space = std::mem::size_of::<UserStakeInfo>() + 8
    )]
    pub stake_state: Account<'info, UserStakeInfo>,
    /// CHECK: only used as a signing PDA
    #[account(mut, seeds = [b"authority".as_ref()], bump)]
    pub program_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub metadata_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = user,)]
    pub nft_token_account: Account<'info, TokenAccount>,
    pub nft_mint: Account<'info, Mint>,
    /// CHECK: THis is not dangerous because we don't read or write from this account
    pub nft_edition: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [user.key().as_ref(), nft_token_account.key().as_ref()],
        bump
    )]
    pub stake_state: Account<'info, UserStakeInfo>,
    /// CHECK: only used as a signing PDA
    #[account(mut, seeds = [b"authority".as_ref()], bump)]
    pub program_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub metadata_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct Redeem<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, token::authority = user,)]
    pub nft_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [user.key().as_ref(), nft_token_account.key().as_ref()],
        bump,
        constraint = *user.key == stake_state.user_pubkey,
        constraint = nft_token_account.key() == stake_state.token_account
    )]
    pub stake_state: Account<'info, UserStakeInfo>,
    #[account(mut)]
    pub stake_mint: Account<'info, Mint>,
    /// CHECK: only used as a signing PDA
    #[account(seeds = [b"mint".as_ref()], bump)]
    pub stake_authority: AccountInfo<'info>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = stake_mint,
        associated_token::authority = user
    )]
    pub user_stake_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(Default, PartialEq)]
pub struct UserStakeInfo {
    pub token_account: Pubkey,
    pub stake_start_time: i64,
    pub last_stake_redeem: i64,
    pub user_pubkey: Pubkey,
    pub stake_state: StakeState,
    pub is_initialized: bool,
}

#[derive(Debug, PartialEq, AnchorDeserialize, AnchorSerialize, Clone)]
pub enum StakeState {
    Staked,
    Unstaked,
}

impl Default for StakeState {
    fn default() -> Self {
        StakeState::Unstaked
    }
}

#[error_code]
pub enum StakeError {
    #[msg("Account already initialized")]
    AccountAlreadyInitialized,
    #[msg("Account not initialized")]
    UnintializedAccount,
    #[msg("Account is not staking anything")]
    InvalidStakeState,
}
