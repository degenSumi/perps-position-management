use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Long,
    Short,
}

impl Side {
    pub fn multiplier(&self) -> i64 {
        match self {
            Side::Long => 1,
            Side::Short => -1,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum PositionStatus {
    Opening,
    Open,
    Modifying,
    Closing,
    Closed,
}

#[account]
pub struct Position {
    pub owner: Pubkey,
    pub symbol: String,
    pub side: Side,
    pub size: u64,
    pub entry_price: u64,
    pub margin: u64,
    pub leverage: u16,              // u16 to support up to 1000x
    pub unrealized_pnl: i64,
    pub realized_pnl: i64,
    pub funding_accrued: i64,
    pub liquidation_price: u64,
    pub last_update: i64,
    pub status: PositionStatus,
    pub bump: u8,
}

impl Position {
    pub const MAX_SIZE: usize = 8 +      // discriminator
        32 +       // owner
        4 + 32 +   // symbol (String with max 32 chars)
        1 +        // side
        8 +        // size
        8 +        // entry_price
        8 +        // margin
        2 +        // leverage (u16)
        8 +        // unrealized_pnl
        8 +        // realized_pnl
        8 +        // funding_accrued
        8 +        // liquidation_price
        8 +        // last_update
        1 +        // status
        1;         // bump
}

#[account]
pub struct UserAccount {
    pub owner: Pubkey,
    pub total_collateral: u64,
    pub locked_collateral: u64,
    pub total_pnl: i64,
    pub position_count: u32, // number of open positions
    pub position_count_total: u32, // index for next position, accounts for closed positions
    pub bump: u8,
}

impl UserAccount {
    pub const LEN: usize = 8 +
        32 +   // owner
        8 +    // total_collateral
        8 +    // locked_collateral
        8 +    // total_pnl
        4 +    // position_count
        4 +    // position_count_total
        1;     // bump
}