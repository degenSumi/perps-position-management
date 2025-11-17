use crate::domain::{Side};
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;

pub struct MarginCalculator;

impl MarginCalculator {
    /// Calculate initial margin required for a position
    /// Formula: Initial Margin = (Position Size × Entry Price) / Leverage
    pub fn calculate_initial_margin(
        size: Decimal,
        entry_price: Decimal,
        leverage: u16,
    ) -> Result<Decimal> {
        if leverage == 0 {
            return Err(anyhow!("Leverage cannot be zero"));
        }

        let position_value = size
            .checked_mul(entry_price)
            .ok_or_else(|| anyhow!("Overflow in position value calculation"))?;

        let leverage_decimal = Decimal::from(leverage);
        let margin = position_value
            .checked_div(leverage_decimal)
            .ok_or_else(|| anyhow!("Division by leverage failed"))?;

        Ok(margin)
    }

    /// Calculate average entry price for multiple trades
    /// Formula: average_entry_price = (Σ(price × quantity)) / (Σ quantity)
    pub fn calculate_average_entry_price(trades: &[(Decimal, Decimal)]) -> Result<Decimal> {
        if trades.is_empty() {
            return Err(anyhow!("No trades provided"));
        }

        let (total_value, total_size) = trades.iter().try_fold(
            (Decimal::ZERO, Decimal::ZERO),
            |(val, size), (price, qty)| {
                let trade_value = price
                    .checked_mul(*qty)
                    .ok_or_else(|| anyhow!("Overflow in trade value"))?;
                let new_val = val
                    .checked_add(trade_value)
                    .ok_or_else(|| anyhow!("Overflow in total value"))?;
                let new_size = size
                    .checked_add(*qty)
                    .ok_or_else(|| anyhow!("Overflow in total size"))?;
                Ok::<_, anyhow::Error>((new_val, new_size))
            },
        )?;

        if total_size.is_zero() {
            return Err(anyhow!("Total size is zero"));
        }

        let average_price = total_value
            .checked_div(total_size)
            .ok_or_else(|| anyhow!("Division failed"))?;

        Ok(average_price)
    }

    /// Calculate unrealized PnL
    /// Long: size × (mark_price - entry_price)
    /// Short: size × (entry_price - mark_price)
    pub fn calculate_unrealized_pnl(
        side: Side,
        size: Decimal,
        mark_price: Decimal,
        entry_price: Decimal,
    ) -> Result<Decimal> {
        let price_diff = match side {
            Side::Long => mark_price
                .checked_sub(entry_price)
                .ok_or_else(|| anyhow!("Price difference overflow"))?,
            Side::Short => entry_price
                .checked_sub(mark_price)
                .ok_or_else(|| anyhow!("Price difference overflow"))?,
        };

        let pnl = size
            .checked_mul(price_diff)
            .ok_or_else(|| anyhow!("PnL calculation overflow"))?;

        Ok(pnl)
    }

    /// Calculate margin ratio
    /// Formula: (collateral + unrealized_pnl) / position_value
    /// Returns ratio as decimal (e.g., 0.15 = 15%)
    pub fn calculate_margin_ratio(
        collateral: Decimal,
        unrealized_pnl: Decimal,
        size: Decimal,
        mark_price: Decimal,
    ) -> Result<Decimal> {
        let position_value = size
            .checked_mul(mark_price)
            .ok_or_else(|| anyhow!("Position value overflow"))?;

        if position_value.is_zero() {
            return Err(anyhow!("Position value is zero"));
        }

        let effective_margin = collateral
            .checked_add(unrealized_pnl)
            .ok_or_else(|| anyhow!("Effective margin overflow"))?;

        let margin_ratio = effective_margin
            .checked_div(position_value)
            .ok_or_else(|| anyhow!("Margin ratio calculation failed"))?;

        Ok(margin_ratio)
    }

    /// Calculate liquidation price for long positions
    /// Formula: entry_price × (1 - 1/leverage + maintenance_margin_ratio)
    pub fn calculate_liquidation_price_long(
        entry_price: Decimal,
        leverage: u16,
        maintenance_margin_ratio: Decimal,
    ) -> Result<Decimal> {
        if leverage == 0 {
            return Err(anyhow!("Leverage cannot be zero"));
        }

        let leverage_decimal = Decimal::from(leverage);
        let one = Decimal::ONE;

        // 1 / leverage
        let leverage_factor = one
            .checked_div(leverage_decimal)
            .ok_or_else(|| anyhow!("Leverage division failed"))?;

        // 1 - 1/leverage + maintenance_margin_ratio
        let adjustment = one
            .checked_sub(leverage_factor)
            .ok_or_else(|| anyhow!("Subtraction failed"))?
            .checked_add(maintenance_margin_ratio)
            .ok_or_else(|| anyhow!("Addition failed"))?;

        // entry_price × adjustment
        let liquidation_price = entry_price
            .checked_mul(adjustment)
            .ok_or_else(|| anyhow!("Liquidation price calculation failed"))?;

        Ok(liquidation_price)
    }

    /// Calculate liquidation price for short positions
    /// Formula: entry_price × (1 + 1/leverage - maintenance_margin_ratio)
    pub fn calculate_liquidation_price_short(
        entry_price: Decimal,
        leverage: u16,
        maintenance_margin_ratio: Decimal,
    ) -> Result<Decimal> {
        if leverage == 0 {
            return Err(anyhow!("Leverage cannot be zero"));
        }

        let leverage_decimal = Decimal::from(leverage);
        let one = Decimal::ONE;

        // 1 / leverage
        let leverage_factor = one
            .checked_div(leverage_decimal)
            .ok_or_else(|| anyhow!("Leverage division failed"))?;

        // 1 + 1/leverage - maintenance_margin_ratio
        let adjustment = one
            .checked_add(leverage_factor)
            .ok_or_else(|| anyhow!("Addition failed"))?
            .checked_sub(maintenance_margin_ratio)
            .ok_or_else(|| anyhow!("Subtraction failed"))?;

        // entry_price × adjustment
        let liquidation_price = entry_price
            .checked_mul(adjustment)
            .ok_or_else(|| anyhow!("Liquidation price calculation failed"))?;

        Ok(liquidation_price)
    }

    /// Unified liquidation price calculation based on side
    pub fn calculate_liquidation_price(
        side: Side,
        entry_price: Decimal,
        leverage: u16,
        maintenance_margin_ratio: Decimal,
    ) -> Result<Decimal> {
        match side {
            Side::Long => Self::calculate_liquidation_price_long(
                entry_price,
                leverage,
                maintenance_margin_ratio,
            ),
            Side::Short => Self::calculate_liquidation_price_short(
                entry_price,
                leverage,
                maintenance_margin_ratio,
            ),
        }
    }

    /// Calculate maintenance margin
    /// Formula: Initial Margin × Maintenance Margin Ratio
    pub fn calculate_maintenance_margin(
        initial_margin: Decimal,
        maintenance_margin_ratio: Decimal,
    ) -> Result<Decimal> {
        let maintenance_margin = initial_margin
            .checked_mul(maintenance_margin_ratio)
            .ok_or_else(|| anyhow!("Maintenance margin calculation failed"))?;

        Ok(maintenance_margin)
    }

    /// Check if position should be liquidated
    /// Liquidation when: Margin Ratio < Maintenance Margin Ratio
    pub fn should_liquidate(
        collateral: Decimal,
        unrealized_pnl: Decimal,
        size: Decimal,
        mark_price: Decimal,
        maintenance_margin_ratio: Decimal,
    ) -> Result<bool> {
        let margin_ratio =
            Self::calculate_margin_ratio(collateral, unrealized_pnl, size, mark_price)?;

        Ok(margin_ratio < maintenance_margin_ratio)
    }

    /// Calculate distance to liquidation as a percentage
    pub fn distance_to_liquidation(
        current_price: Decimal,
        liquidation_price: Decimal,
        side: Side,
    ) -> Result<Decimal> {
        if current_price.is_zero() {
            return Err(anyhow!("Current price is zero"));
        }

        let price_diff = match side {
            Side::Long => {
                // For long: distance = (current - liquidation) / current
                current_price
                    .checked_sub(liquidation_price)
                    .ok_or_else(|| anyhow!("Price difference overflow"))?
            }
            Side::Short => {
                // For short: distance = (liquidation - current) / current
                liquidation_price
                    .checked_sub(current_price)
                    .ok_or_else(|| anyhow!("Price difference overflow"))?
            }
        };

        let distance = price_diff
            .checked_div(current_price)
            .ok_or_else(|| anyhow!("Distance calculation failed"))?;

        Ok(distance)
    }

    /// Calculate maximum position size for given margin and leverage
    /// Formula: (margin * leverage) / entry_price
    pub fn calculate_max_position_size(
        available_margin: Decimal,
        entry_price: Decimal,
        leverage: u16,
    ) -> Result<Decimal> {
        if available_margin <= Decimal::ZERO {
            return Err(anyhow!("Available margin must be positive"));
        }
        if entry_price <= Decimal::ZERO {
            return Err(anyhow!("Entry price must be positive"));
        }
        if leverage == 0 {
            return Err(anyhow!("Leverage must be positive"));
        }

        let leverage_decimal = Decimal::from(leverage);
        let buying_power = available_margin
            .checked_mul(leverage_decimal)
            .ok_or_else(|| anyhow!("Buying power calculation overflow"))?;
        let max_size = buying_power
            .checked_div(entry_price)
            .ok_or_else(|| anyhow!("Max size calculation failed"))?;

        Ok(max_size)
    }

    /// Calculate ROI (Return on Investment)
    /// Formula: (unrealized_pnl / margin) * 100
    pub fn calculate_roi(unrealized_pnl: Decimal, initial_margin: Decimal) -> Result<Decimal> {
        if initial_margin <= Decimal::ZERO {
            return Err(anyhow!("Margin must be positive"));
        }

        let roi = unrealized_pnl
            .checked_div(initial_margin)
            .ok_or_else(|| anyhow!("ROI calculation failed"))?
            .checked_mul(Decimal::from(100))
            .ok_or_else(|| anyhow!("ROI multiplication failed"))?;

        Ok(roi)
    }

    // Calculate funding payment
    /// Formula: position_value * funding_rate
    pub fn calculate_funding_payment(
        size: Decimal,
        mark_price: Decimal,
        funding_rate: Decimal,
    ) -> Result<Decimal> {
        if size <= Decimal::ZERO || mark_price <= Decimal::ZERO {
            return Err(anyhow!("Size and price must be positive"));
        }

        let position_value = size
            .checked_mul(mark_price)
            .ok_or_else(|| anyhow!("Position value overflow"))?;
        let funding_payment = position_value
            .checked_mul(funding_rate)
            .ok_or_else(|| anyhow!("Funding payment calculation failed"))?;

        Ok(funding_payment)
    }

    /// Validate if position can be opened with available collateral
    pub fn validate_position_opening(
        available_collateral: Decimal,
        required_margin: Decimal,
        maintenance_margin_ratio: Decimal,
    ) -> Result<()> {
        if available_collateral < required_margin {
            return Err(anyhow!(
                "Insufficient collateral. Required: {}, Available: {}",
                required_margin,
                available_collateral
            ));
        }

        // Ensure some buffer above maintenance margin
        let buffer = required_margin
            .checked_mul(maintenance_margin_ratio)
            .ok_or_else(|| anyhow!("Buffer calculation failed"))?;
        let min_collateral = required_margin
            .checked_add(buffer)
            .ok_or_else(|| anyhow!("Min collateral calculation failed"))?;

        if available_collateral < min_collateral {
            return Err(anyhow!(
                "Collateral too close to maintenance margin. Minimum required: {}",
                min_collateral
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_calculate_initial_margin() {
        // 1 BTC at 50,000 USDT with 10x leverage
        let size = dec!(1);
        let entry_price = dec!(50000);
        let leverage = 10;

        let margin =
            MarginCalculator::calculate_initial_margin(size, entry_price, leverage).unwrap();

        assert_eq!(margin, dec!(5000)); // 50,000 / 10 = 5,000 USDT
    }

    #[test]
    fn test_calculate_average_entry_price() {
        // Buy 1 BTC at 50,000 and 1 BTC at 60,000
        // Average should be 55,000
        let trades = vec![(dec!(50000), dec!(1)), (dec!(60000), dec!(1))];

        let avg_price = MarginCalculator::calculate_average_entry_price(&trades).unwrap();

        assert_eq!(avg_price, dec!(55000));
    }

    #[test]
    fn test_unrealized_pnl_long_profit() {
        // Long 1 BTC: bought at 50,000, now at 55,000
        // PnL = 1 × (55,000 - 50,000) = 5,000 USDT profit
        let pnl = MarginCalculator::calculate_unrealized_pnl(
            Side::Long,
            dec!(1),
            dec!(55000),
            dec!(50000),
        )
        .unwrap();

        assert_eq!(pnl, dec!(5000));
    }

    #[test]
    fn test_unrealized_pnl_long_loss() {
        // Long 1 BTC: bought at 50,000, now at 45,000
        // PnL = 1 × (45,000 - 50,000) = -5,000 USDT loss
        let pnl = MarginCalculator::calculate_unrealized_pnl(
            Side::Long,
            dec!(1),
            dec!(45000),
            dec!(50000),
        )
        .unwrap();

        assert_eq!(pnl, dec!(-5000));
    }

    #[test]
    fn test_unrealized_pnl_short_profit() {
        // Short 1 BTC: sold at 50,000, now at 45,000
        // PnL = 1 × (50,000 - 45,000) = 5,000 USDT profit
        let pnl = MarginCalculator::calculate_unrealized_pnl(
            Side::Short,
            dec!(1),
            dec!(45000),
            dec!(50000),
        )
        .unwrap();

        assert_eq!(pnl, dec!(5000));
    }

    #[test]
    fn test_margin_ratio() {
        // Collateral: 5,000 USDT
        // Unrealized PnL: +2,000 USDT
        // Position: 1 BTC at 55,000 USDT
        // Margin Ratio = (5,000 + 2,000) / 55,000 = 0.127272...
        let ratio =
            MarginCalculator::calculate_margin_ratio(dec!(5000), dec!(2000), dec!(1), dec!(55000))
                .unwrap();

        assert!(ratio > dec!(0.127) && ratio < dec!(0.128));
    }

    #[test]
    fn test_liquidation_price_long() {
        // Long position at 50,000 with 10x leverage
        // Maintenance margin ratio: 2.5% (0.025)
        // Liquidation = 50,000 × (1 - 0.1 + 0.025) = 50,000 × 0.925 = 46,250
        let liq_price =
            MarginCalculator::calculate_liquidation_price_long(dec!(50000), 10, dec!(0.025))
                .unwrap();

        assert_eq!(liq_price, dec!(46250));
    }

    #[test]
    fn test_liquidation_price_short() {
        // Short position at 50,000 with 10x leverage
        // Maintenance margin ratio: 2.5% (0.025)
        // Liquidation = 50,000 × (1 + 0.1 - 0.025) = 50,000 × 1.075 = 53,750
        let liq_price =
            MarginCalculator::calculate_liquidation_price_short(dec!(50000), 10, dec!(0.025))
                .unwrap();

        assert_eq!(liq_price, dec!(53750));
    }

    #[test]
    fn test_should_liquidate_safe() {
        // Margin ratio well above maintenance
        let should_liq = MarginCalculator::should_liquidate(
            dec!(5000),
            dec!(0),
            dec!(1),
            dec!(50000),
            dec!(0.025), // 2.5% maintenance
        )
        .unwrap();

        assert!(!should_liq); // Margin ratio is 10%, above 2.5%
    }

    #[test]
    fn test_should_liquidate_danger() {
        // Margin ratio below maintenance (large loss)
        let should_liq = MarginCalculator::should_liquidate(
            dec!(5000),
            dec!(-4000), // Large loss
            dec!(1),
            dec!(50000),
            dec!(0.025), // 2.5% maintenance
        )
        .unwrap();

        assert!(should_liq); // Margin ratio is 2%, below 2.5%
    }

    #[test]
    fn test_distance_to_liquidation_long() {
        // Long at current 50,000, liquidation at 46,250
        // Distance = (50,000 - 46,250) / 50,000 = 0.075 = 7.5%
        let distance =
            MarginCalculator::distance_to_liquidation(dec!(50000), dec!(46250), Side::Long)
                .unwrap();

        assert_eq!(distance, dec!(0.075));
    }
}
