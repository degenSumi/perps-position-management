use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub position_index: u32,
    pub owner: Pubkey,
    pub position_account: Pubkey,
    pub symbol: String,
    pub side: Side,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub mark_price: Decimal,
    pub margin: Decimal,
    pub leverage: u16,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub funding_accrued: Decimal,
    pub liquidation_price: Decimal,
    pub status: PositionStatus,
    pub opened_at: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

impl Position {
    pub fn is_open(&self) -> bool {
        matches!(self.status, PositionStatus::Open | PositionStatus::Opening)
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.status, PositionStatus::Closed)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Side {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Risk {
    Liquidated,
    Liquidating,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PositionStatus {
    Opening,
    Open,
    Modifying,
    Closing,
    Closed,
}