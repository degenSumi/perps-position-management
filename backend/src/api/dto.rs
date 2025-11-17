use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

use crate::domain::{Side, PositionStatus, Risk};
use solana_sdk::pubkey::Pubkey;

// Request DTOs
#[derive(Debug, Deserialize)]
pub struct OpenPositionRequest {
    pub owner: String,
    pub symbol: String,
    pub side: Side,
    pub size: Decimal,
    pub leverage: u16,
    pub entry_price: Decimal,
    pub maintenance_margin_ratio: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct ModifyPositionRequest {
    pub new_size: Option<Decimal>,
    pub margin_delta: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ClosePositionRequest {
    pub final_price: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct InitializeUserRequest {
    pub owner: String,
}

#[derive(Debug, Deserialize)]
pub struct AddCollateralRequest {
    pub amount: u64,
}

// Response DTOs
#[derive(Debug, Serialize)]
pub struct OpenPositionResponse {
    pub position: PositionDto,
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct ModifyPositionResponse {
    pub signature: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ClosePositionResponse {
    pub pnl: Decimal,
    pub signature: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct InitializeUserResponse {
    pub signature: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AddCollateralResponse {
    pub signature: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct UserAccountDto {
    pub owner: String,
    pub total_collateral: u64,
    pub locked_collateral: u64,
    pub available_collateral: u64,
    pub total_pnl: i64,
    pub position_count: u32,
    pub position_count_total: u32,
}

/// Position response DTO
#[derive(Debug, Serialize, Deserialize)]
pub struct PositionDto {
    pub position_account: String,
    pub owner: String,
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

impl From<crate::domain::Position> for PositionDto {
    fn from(pos: crate::domain::Position) -> Self {
        Self {
            position_account: pos.position_account.to_string(),
            owner: pos.owner.to_string(),
            symbol: pos.symbol,
            side: pos.side,
            size: pos.size,
            entry_price: pos.entry_price,
            mark_price: pos.mark_price,
            margin: pos.margin,
            leverage: pos.leverage,
            unrealized_pnl: pos.unrealized_pnl,
            realized_pnl: pos.realized_pnl,
            funding_accrued: pos.funding_accrued,
            liquidation_price: pos.liquidation_price,
            status: pos.status,
            opened_at: pos.opened_at,
            last_update: pos.last_update,
            closed_at: pos.closed_at,
        }
    }
}

/// List positions query parameters
#[derive(Debug, Deserialize)]
pub struct ListPositionsQuery {
    pub owner: Option<String>,
    pub symbol: Option<String>,
    pub status: Option<PositionStatus>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Statistics response
#[derive(Debug, Serialize)]
pub struct StatisticsDto {
    pub total_positions: usize,
    pub open_positions: usize,
    pub assets_monitored: usize,
    pub total_unrealized_pnl: Decimal,
}

/// Price update DTO
#[derive(Debug, Serialize)]
pub struct PriceDto {
    pub symbol: String,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Position update DTO (for WebSocket)
#[derive(Debug, Serialize)]
pub struct PositionUpdateDto {
    pub position_account: String,
    pub symbol: String,
    pub side: Side,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub mark_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub margin_ratio: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Liquidation alert DTO
#[derive(Debug, Serialize)]
pub struct LiquidationAlertDto {
    pub risk_type: Risk,
    pub(crate) position_account: Pubkey,
    pub symbol: String,
    pub side: Side,
    pub liquidation_price: Decimal,
    pub current_price: Decimal,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

/// Success response
#[derive(Debug, Serialize)]
pub struct SuccessResponse<T> {
    pub data: T,
}
