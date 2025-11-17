use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLSnapshot {
    pub id: uuid::Uuid,
    pub position_id: uuid::Uuid,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub funding_accrued: Decimal,
    pub mark_price: Decimal,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealizedPnL {
    pub position_id: uuid::Uuid,
    pub amount: Decimal,
    pub close_price: Decimal,
    pub closed_at: DateTime<Utc>,
}
