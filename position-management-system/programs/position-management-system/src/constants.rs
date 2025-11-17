use anchor_lang::prelude::*;

pub const PRICE_PRECISION: u64 = 1_000_000;
pub const SUPPORTED_ASSET_DECIMALS: u64 = 1_000_000;
pub const MAX_SYMBOL_LENGTH: usize = 32;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct LeverageTier {
    pub max_leverage: u16,
    pub initial_margin_rate: u64,        // in basis points (e.g., 500 = 5%)
    pub maintenance_margin_rate: u64,    // in basis points (e.g., 250 = 2.5%)
    pub max_position_size: u64,
}

pub const LEVERAGE_TIERS: [LeverageTier; 5] = [
    LeverageTier {
        max_leverage: 20,
        initial_margin_rate: 500,        // 5.0%
        maintenance_margin_rate: 250,    // 2.5%
        max_position_size: u64::MAX,
    },
    LeverageTier {
        max_leverage: 50,
        initial_margin_rate: 200,        // 2.0%
        maintenance_margin_rate: 100,    // 1.0%
        max_position_size: 100_000 * PRICE_PRECISION,
    },
    LeverageTier {
        max_leverage: 100,
        initial_margin_rate: 100,        // 1.0%
        maintenance_margin_rate: 50,     // 0.5%
        max_position_size: 50_000 * PRICE_PRECISION,
    },
    LeverageTier {
        max_leverage: 500,
        initial_margin_rate: 50,         // 0.5%
        maintenance_margin_rate: 25,     // 0.25%
        max_position_size: 20_000 * PRICE_PRECISION,
    },
    LeverageTier {
        max_leverage: 1000,
        initial_margin_rate: 20,         // 0.2%
        maintenance_margin_rate: 10,     // 0.1%
        max_position_size: 5_000 * PRICE_PRECISION,
    },
];

pub fn get_leverage_tier(leverage: u16, position_size: u64) -> Result<LeverageTier> {
    for tier in &LEVERAGE_TIERS {
        if leverage <= tier.max_leverage && position_size <= tier.max_position_size {
            return Ok(*tier);
        }
    }
    Err(error!(crate::errors::PositionError::LeverageExceeded))
}
