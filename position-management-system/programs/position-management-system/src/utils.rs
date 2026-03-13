use anchor_lang::prelude::*;
use crate::constants::{PRICE_PRECISION, SUPPORTED_ASSET_DECIMALS, get_leverage_tier};
use crate::state::Side;
use crate::errors::PositionError;
use pyth_solana_receiver_sdk::price_update::Price;

/// Maximum age of price data in seconds
pub const MAXIMUM_AGE: u64 = 1;

/// Pyth price feed IDs for different symbols
pub fn get_price_feed_id(symbol: &str) -> Result<String> {
    match symbol {
        "BTC-USD" => Ok("e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43".to_string()),
        "ETH-USD" => Ok("ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace".to_string()),
        "SOL-USD" => Ok("ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d".to_string()),
        _ => Err(error!(PositionError::InvalidSymbol)),
    }
}

/// Convert Pyth price to u64 with 6 decimals
pub fn convert_pyth_price_to_u64(price: &Price) -> Result<u64> {
    // Pyth price has expo (usually -8 for crypto)
    // We want 6 decimals, so adjust accordingly
    let price_i64 = price.price;
    let expo = price.exponent;
    
    // Convert to 6 decimals
    // If expo is -8, multiply by 100 to get to -6
    // If expo is -6, keep as is
    let adjusted_price = if expo < -6 {
        let adjustment = 10i64.pow((-6 - expo) as u32);
        price_i64 / adjustment
    } else if expo > -6 {
        let adjustment = 10i64.pow((expo + 6) as u32);
        price_i64 * adjustment
    } else {
        price_i64
    };

    require!(adjusted_price > 0, PositionError::InvalidPrice);
    
    Ok(adjusted_price as u64)
}


/// Validate slippage between expected price and actual oracle price
pub fn validate_slippage(
    expected_price: u64,
    actual_price: u64,
    max_slippage_bps: u16,
    side: Side,
) -> Result<()> {
    // Calculate price difference
    let price_diff = if actual_price > expected_price {
        actual_price - expected_price
    } else {
        expected_price - actual_price
    };

    // Calculate slippage in basis points (1 bps = 0.01%)
    let slippage_bps = (price_diff as u128)
        .checked_mul(10000)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(expected_price as u128)
        .ok_or(error!(PositionError::ArithmeticOverflow))? as u16;

    msg!("Slippage check: expected={}, actual={}, slippage={}bps, max={}bps", 
        expected_price, actual_price, slippage_bps, max_slippage_bps);

    // Check if slippage exceeds maximum
    require!(
        slippage_bps <= max_slippage_bps,
        PositionError::SlippageExceeded
    );

    // Additional check: ensure price moved in unfavorable direction doesn't exceed limit
    match side {
        Side::Long => {
            // For longs, unfavorable if actual > expected (buying at higher price)
            if actual_price > expected_price {
                let unfavorable_slippage = ((actual_price - expected_price) as u128)
                    .checked_mul(10000)
                    .ok_or(error!(PositionError::ArithmeticOverflow))?
                    .checked_div(expected_price as u128)
                    .ok_or(error!(PositionError::ArithmeticOverflow))? as u16;
                
                require!(
                    unfavorable_slippage <= max_slippage_bps,
                    PositionError::SlippageExceeded
                );
            }
        }
        Side::Short => {
            // For shorts, unfavorable if actual < expected (selling at lower price)
            if actual_price < expected_price {
                let unfavorable_slippage = ((expected_price - actual_price) as u128)
                    .checked_mul(10000)
                    .ok_or(error!(PositionError::ArithmeticOverflow))?
                    .checked_div(expected_price as u128)
                    .ok_or(error!(PositionError::ArithmeticOverflow))? as u16;
                
                require!(
                    unfavorable_slippage <= max_slippage_bps,
                    PositionError::SlippageExceeded
                );
            }
        }
    }

    Ok(())
}

/// Calculate Initial Margin
/// Formula: Initial Margin = (Position Size × Entry Price) / Leverage
pub fn calculate_initial_margin(
    size: u64,
    entry_price: u64,
    leverage: u16,
) -> Result<u64> {
    // Step 1: Calculate position value in base units
    let position_value_base = size
        .checked_mul(entry_price)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(SUPPORTED_ASSET_DECIMALS)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    // Step 2: Divide by leverage
    let margin = position_value_base
        .checked_div(leverage as u64)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;

    msg!("Calculated initial margin: {}", margin);

    msg!("Position value (base units): {}", position_value_base);
    
    // margin has 6 decimals (matches collateral storage)
    Ok(margin)
}

/// Calculate position value for tier validation (returns whole number)
pub fn calculate_position_value_for_tiers(
    size: u64,
    entry_price: u64,
) -> Result<u64> {
    // Calculate as whole USDT for tier validation
    let position_value = size
        .checked_mul(entry_price)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(SUPPORTED_ASSET_DECIMALS)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    Ok(position_value)
}

/// Calculate Average Entry Price
pub fn calculate_average_entry_price(
    old_size: u64,
    old_entry_price: u64,
    additional_size: u64,
    new_entry_price: u64,
) -> Result<u64> {
    let old_value = old_size
        .checked_mul(old_entry_price)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let new_value = additional_size
        .checked_mul(new_entry_price)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let total_value = old_value
        .checked_add(new_value)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let total_size = old_size
        .checked_add(additional_size)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let average_price = total_value
        .checked_div(total_size)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    Ok(average_price)
}

/// Calculate Unrealized PnL
/// Formula (Long): size × (mark_price - entry_price)
/// Formula (Short): size × (entry_price - mark_price)
pub fn calculate_unrealized_pnl(
    size: u64,
    entry_price: u64,
    mark_price: u64,
    side: Side,
) -> Result<i64> {
    let price_diff = match side {
        Side::Long => {
            if mark_price >= entry_price {
                ((mark_price - entry_price) as i64, 1)
            } else {
                ((entry_price - mark_price) as i64, -1)
            }
        },
        Side::Short => {
            if entry_price >= mark_price {
                ((entry_price - mark_price) as i64, 1)
            } else {
                ((mark_price - entry_price) as i64, -1)
            }
        }
    };
    
    // PnL = size × price_diff
    let pnl = (size as i64)
        .checked_mul(price_diff.0)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_mul(price_diff.1)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(SUPPORTED_ASSET_DECIMALS as i64)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    // pnl has 6 decimals
    Ok(pnl)
}

/// Calculate Margin Ratio
/// Formula: (collateral + unrealized_pnl) / position_value
/// Returns ratio in basis points (10000 = 100%)
pub fn calculate_margin_ratio(
    margin: u64,           // Base units (6 decimals)
    unrealized_pnl: i64,   // Base units (6 decimals)
    size: u64,
    mark_price: u64,
) -> Result<u64> {
    let position_value = size
        .checked_mul(mark_price)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(SUPPORTED_ASSET_DECIMALS)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    // position_value has 6 decimals
    
    // Calculate: collateral + unrealized_pnl
    let effective_margin = if unrealized_pnl >= 0 {
        margin
            .checked_add(unrealized_pnl as u64)
            .ok_or(error!(PositionError::ArithmeticOverflow))?
    } else {
        let loss = (-unrealized_pnl) as u64;
        if margin >= loss {
            margin - loss
        } else {
            0
        }
    };
    
    // Margin Ratio = effective_margin / position_value (in basis points)
    let margin_ratio = effective_margin
        .checked_mul(10000)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(position_value)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    Ok(margin_ratio)
}

/// Calculate Liquidation Price (Long)
pub fn calculate_liquidation_price_long(
    entry_price: u64,
    leverage: u16,
    maintenance_margin_rate: u64,
) -> Result<u64> {
    let leverage_u64 = leverage as u64;
    
    let mm_factor = maintenance_margin_rate
        .checked_mul(PRICE_PRECISION)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(10000)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let leverage_factor = PRICE_PRECISION
        .checked_div(leverage_u64)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let adjustment = PRICE_PRECISION
        .checked_sub(leverage_factor)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_add(mm_factor)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let liq_price = entry_price
        .checked_mul(adjustment)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(PRICE_PRECISION)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    Ok(liq_price)
}

/// Calculate Liquidation Price (Short)
pub fn calculate_liquidation_price_short(
    entry_price: u64,
    leverage: u16,
    maintenance_margin_rate: u64,
) -> Result<u64> {
    let leverage_u64 = leverage as u64;
    
    let mm_factor = maintenance_margin_rate
        .checked_mul(PRICE_PRECISION)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(10000)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let leverage_factor = PRICE_PRECISION
        .checked_div(leverage_u64)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let adjustment = PRICE_PRECISION
        .checked_add(leverage_factor)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_sub(mm_factor)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    let liq_price = entry_price
        .checked_mul(adjustment)
        .ok_or(error!(PositionError::ArithmeticOverflow))?
        .checked_div(PRICE_PRECISION)
        .ok_or(error!(PositionError::ArithmeticOverflow))?;
    
    Ok(liq_price)
}

/// Unified liquidation price calculation
pub fn calculate_liquidation_price(
    entry_price: u64,
    leverage: u16,
    side: Side,
    maintenance_margin_rate: u64,
) -> Result<u64> {
    match side {
        Side::Long => calculate_liquidation_price_long(entry_price, leverage, maintenance_margin_rate),
        Side::Short => calculate_liquidation_price_short(entry_price, leverage, maintenance_margin_rate),
    }
}

/// Check if position should be liquidated
pub fn check_liquidation(
    margin: u64,
    unrealized_pnl: i64,
    size: u64,
    mark_price: u64,
    maintenance_margin_rate: u64,
) -> Result<bool> {
    let margin_ratio = calculate_margin_ratio(margin, unrealized_pnl, size, mark_price)?;
    Ok(margin_ratio < maintenance_margin_rate)
}

/// Validate leverage and position size against tier limits
pub fn validate_leverage_and_size(
    leverage: u16,
    size: u64,
    entry_price: u64,
) -> Result<()> {
    // For tier validation, we need whole USDT values
    let position_value = calculate_position_value_for_tiers(size, entry_price)?;
    get_leverage_tier(leverage, position_value)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unrealized_pnl_long() {
        // Long: 1 BTC @ 50k, now 55k
        let size = 100_000_000;
        let entry = 50_000_000_000;
        let mark = 55_000_000_000;
        
        let pnl = calculate_unrealized_pnl(size, entry, mark, Side::Long).unwrap();
        assert_eq!(pnl, 5_000); // 5k USDT profit
    }
    
    #[test]
    fn test_unrealized_pnl_short() {
        // Short: 1 BTC @ 50k, now 45k
        let size = 100_000_000;
        let entry = 50_000_000_000;
        let mark = 45_000_000_000;
        
        let pnl = calculate_unrealized_pnl(size, entry, mark, Side::Short).unwrap();
        assert_eq!(pnl, 5_000); // 5k USDT profit
    }
    
    #[test]
    fn test_average_entry_price() {
        // Existing: 1 BTC @ 50k, Adding: 1 BTC @ 60k
        // Expected average: 55k
        let old_size = 100_000_000;
        let old_price = 50_000_000_000;
        let add_size = 100_000_000;
        let new_price = 60_000_000_000;
        
        let avg = calculate_average_entry_price(old_size, old_price, add_size, new_price).unwrap();
        assert_eq!(avg, 55_000_000_000); // 55k
    }
    
    #[test]
    fn test_margin_ratio() {
        // Margin: 5k, PnL: +2k, Position: 1 BTC @ 55k
        let margin = 5_000;
        let pnl = 2_000;
        let size = 100_000_000;
        let mark_price = 55_000_000_000;
        
        let ratio = calculate_margin_ratio(margin, pnl, size, mark_price).unwrap();
        // (5000 + 2000) / 55000 = 0.127 = 1272 basis points
        assert!(ratio > 1200 && ratio < 1300);
    }
}