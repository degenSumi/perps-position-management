use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::PositionError;

#[derive(Accounts)]
pub struct InitializeUser<'info> {
    #[account(
        init,
        payer = user,
        space = UserAccount::LEN,
        seeds = [b"user", user.key().as_ref()],
        bump
    )]
    pub user_account: Account<'info, UserAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(
        init,
        payer = user,
        space = Position::MAX_SIZE,
        seeds = [
            b"position",
            user.key().as_ref(),
            user_account.position_count_total.to_le_bytes().as_ref()
        ],
        bump
    )]
    pub position: Account<'info, Position>,
    
    #[account(
        mut,
        seeds = [b"user", user.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Account<'info, UserAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ModifyPosition<'info> {
    #[account(
        mut,
        has_one = owner @ PositionError::Unauthorized
    )]
    pub position: Account<'info, Position>,
    
    #[account(
        mut,
        seeds = [b"user", owner.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Account<'info, UserAccount>,
    
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClosePosition<'info> {
    #[account(
        mut,
        has_one = owner @ PositionError::Unauthorized,
        // close = owner
    )]
    pub position: Account<'info, Position>,
    
    #[account(
        mut,
        seeds = [b"user", owner.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Account<'info, UserAccount>,
    
    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ModifyUserCollateral<'info> {
    #[account(
        mut,
        seeds = [b"user", owner.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Account<'info, UserAccount>,
    
    pub owner: Signer<'info>,
}

// Events
#[event]
pub struct PositionOpened {
    pub position: Pubkey,
    pub owner: Pubkey,
    pub symbol: String,
    pub side: Side,
    pub size: u64,
    pub entry_price: u64,
    pub leverage: u16,
    pub margin: u64,
    pub timestamp: i64,
}

#[event]
pub struct PositionModified {
    pub position: Pubkey,
    pub old_size: u64,
    pub new_size: u64,
    pub old_margin: u64,
    pub new_margin: u64,
    pub timestamp: i64,
}

#[event]
pub struct PositionClosed {
    pub position: Pubkey,
    pub owner: Pubkey,
    pub realized_pnl: i64,
    pub timestamp: i64,
}