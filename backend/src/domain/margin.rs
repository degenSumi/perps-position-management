use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginRequirement {
    pub initial_margin: Decimal,
    pub maintenance_margin: Decimal,
    pub margin_ratio: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationThreshold {
    pub liquidation_price: Decimal,
    pub maintenance_margin_ratio: Decimal,
    pub distance_to_liquidation: Decimal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LeverageTier {
    pub max_leverage: u16,
    pub initial_margin_rate: u64,
    pub maintenance_margin_rate: u64,
    pub max_position_size: u64,
}
