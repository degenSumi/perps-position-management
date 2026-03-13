use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex};

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod state;
pub mod utils;

use constants::*;
use errors::*;
use instructions::*;
use state::*;
use utils::*;

declare_id!("5JfvJ18ynvBsZog4g3NgZhKSGob49CMpefhHEX9MbXLV");

#[program]
pub mod position_management_system {
    use super::*;

    pub fn initialize_user(ctx: Context<InitializeUser>) -> Result<()> {
        let user_account = &mut ctx.accounts.user_account;

        user_account.owner = ctx.accounts.user.key();
        user_account.total_collateral = 0;
        user_account.locked_collateral = 0;
        user_account.total_pnl = 0;
        user_account.position_count = 0;
        user_account.position_count_total = 0;
        user_account.bump = ctx.bumps.user_account;

        msg!("User account initialized for: {}", user_account.owner);

        Ok(())
    }

    pub fn open_position(
        ctx: Context<OpenPosition>,
        symbol: String,
        side: Side,
        size: u64,
        leverage: u16,
        expected_price: u64,       // User's expected price
        maximum_slippage_bps: u16, // Max acceptable slippage in basis points (e.g., 50 = 0.5%)
    ) -> Result<()> {
        require!(size > 0, PositionError::InvalidPositionSize);
        require!(
            leverage >= 1 && leverage <= 1000,
            PositionError::InvalidLeverage
        );
        require!(
            symbol.len() <= MAX_SYMBOL_LENGTH,
            PositionError::InvalidSymbol
        );
        require!(
            maximum_slippage_bps <= 10000, // Max 100%
            PositionError::InvalidSlippage
        );

        // Get price from Pyth oracle
        let price_update = &ctx.accounts.price_update;
        let price = price_update.get_price_no_older_than(
            &Clock::get()?,
            MAXIMUM_AGE,
            &get_feed_id_from_hex(&get_price_feed_id(&symbol)?)?,
        )?;

        // Convert Pyth price to our format (assuming 6 decimals for USDT)
        let entry_price = convert_pyth_price_to_u64(&price)?;

        msg!("Oracle price for {}: {}", symbol, entry_price);

        // Slippage protection
        validate_slippage(expected_price, entry_price, maximum_slippage_bps, side)?;

        validate_leverage_and_size(leverage, size, entry_price)?;

        let required_margin = calculate_initial_margin(size, entry_price, leverage)?;

        let position_value = calculate_position_value_for_tiers(size, entry_price)?;
        let tier = get_leverage_tier(leverage, position_value)?;

        let liquidation_price =
            calculate_liquidation_price(entry_price, leverage, side, tier.maintenance_margin_rate)?;

        let position_key = ctx.accounts.position.key();
        let user_key = ctx.accounts.user.key();

        let user_account = &mut ctx.accounts.user_account;
        user_account.position_count_total = user_account
            .position_count_total
            .checked_add(1)
            .ok_or(error!(PositionError::ArithmeticOverflow))?;

        user_account.position_count = user_account
            .position_count
            .checked_add(1)
            .ok_or(error!(PositionError::ArithmeticOverflow))?;

        let available_collateral = user_account
            .total_collateral
            .checked_sub(user_account.locked_collateral)
            .ok_or(error!(PositionError::InsufficientCollateral))?;

        require!(
            available_collateral >= required_margin,
            PositionError::InsufficientCollateral
        );

        user_account.locked_collateral = user_account
            .locked_collateral
            .checked_add(required_margin)
            .ok_or(error!(PositionError::ArithmeticOverflow))?;

        msg!(
            "Opening position for user: {} with position key: {}",
            user_key,
            position_key
        );

        let position = &mut ctx.accounts.position;
        position.owner = user_key;
        position.symbol = symbol.clone();
        position.side = side;
        position.size = size;
        position.entry_price = entry_price;
        position.margin = required_margin;
        position.leverage = leverage;
        position.unrealized_pnl = 0;
        position.realized_pnl = 0;
        position.funding_accrued = 0;
        position.liquidation_price = liquidation_price;
        position.last_update = Clock::get()?.unix_timestamp;
        position.status = PositionStatus::Open;
        position.bump = ctx.bumps.position;

        emit!(PositionOpened {
            position: position_key,
            owner: user_key,
            symbol,
            side,
            size,
            entry_price,
            leverage,
            margin: required_margin,
            timestamp: position.last_update,
        });

        msg!(
            "Position opened: {} with {}x leverage at price {} (expected: {}, slippage: {} bps)",
            size,
            leverage,
            entry_price,
            expected_price,
            maximum_slippage_bps
        );

        Ok(())
    }

    pub fn modify_position(
        ctx: Context<ModifyPosition>,
        new_size: Option<u64>,
        margin_delta: Option<i64>,
    ) -> Result<()> {
        let position_key = ctx.accounts.position.key();
        let position = &mut ctx.accounts.position;
        let user_account = &mut ctx.accounts.user_account;

        require!(
            position.status == PositionStatus::Open,
            PositionError::PositionNotOpen
        );

        let old_size = position.size;
        let old_margin = position.margin;

        if let Some(size) = new_size {
            require!(size > 0, PositionError::InvalidPositionSize);

            validate_leverage_and_size(position.leverage, size, position.entry_price)?;

            let new_required_margin =
                calculate_initial_margin(size, position.entry_price, position.leverage)?;

            if new_required_margin > position.margin {
                let additional_margin = new_required_margin - position.margin;
                let available = user_account.total_collateral - user_account.locked_collateral;

                require!(
                    available >= additional_margin,
                    PositionError::InsufficientCollateral
                );

                user_account.locked_collateral += additional_margin;
            } else {
                let freed_margin = position.margin - new_required_margin;
                user_account.locked_collateral -= freed_margin;
            }

            position.size = size;
            position.margin = new_required_margin;
        }

        if let Some(delta) = margin_delta {
            if delta > 0 {
                let additional_margin = delta as u64;
                let available = user_account.total_collateral - user_account.locked_collateral;

                require!(
                    available >= additional_margin,
                    PositionError::InsufficientCollateral
                );

                position.margin += additional_margin;
                user_account.locked_collateral += additional_margin;

                let position_value = position
                    .size
                    .checked_mul(position.entry_price)
                    .ok_or(error!(PositionError::ArithmeticOverflow))?
                    .checked_div(SUPPORTED_ASSET_DECIMALS)
                    .ok_or(error!(PositionError::ArithmeticOverflow))?;

                position.leverage = (position_value / position.margin).min(1000) as u16;
            } else {
                let remove_amount = (-delta) as u64;

                require!(
                    position.margin > remove_amount,
                    PositionError::CannotRemoveMargin
                );

                let new_margin = position.margin - remove_amount;
                let min_margin = calculate_initial_margin(
                    position.size,
                    position.entry_price,
                    position.leverage,
                )?;

                require!(new_margin >= min_margin, PositionError::CannotRemoveMargin);

                position.margin = new_margin;
                user_account.locked_collateral -= remove_amount;
            }
        }

        position.last_update = Clock::get()?.unix_timestamp;
        position.status = PositionStatus::Open;

        emit!(PositionModified {
            position: position_key,
            old_size,
            new_size: position.size,
            old_margin,
            new_margin: position.margin,
            timestamp: position.last_update,
        });

        msg!("Position modified");

        Ok(())
    }

    pub fn close_position(ctx: Context<ClosePosition>) -> Result<()> {
        let position_key = ctx.accounts.position.key();
        let owner_key = ctx.accounts.owner.key();

        let position = &mut ctx.accounts.position;
        let user_account = &mut ctx.accounts.user_account;

        require!(
            position.status == PositionStatus::Open,
            PositionError::PositionNotOpen
        );

        // Get current price from Pyth oracle
        let price_update = &ctx.accounts.price_update;
        let price = price_update.get_price_no_older_than(
            &Clock::get()?,
            MAXIMUM_AGE,
            &get_feed_id_from_hex(&get_price_feed_id(&position.symbol)?)?,
        )?;

        let final_price = convert_pyth_price_to_u64(&price)?;

        msg!("Closing position at oracle price: {}", final_price);

        let final_pnl = calculate_unrealized_pnl(
            position.size,
            position.entry_price,
            final_price,
            position.side,
        )?;

        let total_pnl = final_pnl
            .checked_add(position.funding_accrued)
            .ok_or(error!(PositionError::ArithmeticOverflow))?;

        position.realized_pnl = total_pnl;

        user_account.locked_collateral = user_account
            .locked_collateral
            .checked_sub(position.margin)
            .ok_or(error!(PositionError::ArithmeticOverflow))?;

        if total_pnl >= 0 {
            user_account.total_collateral = user_account
                .total_collateral
                .checked_add(total_pnl as u64)
                .ok_or(error!(PositionError::ArithmeticOverflow))?;
        } else {
            let loss = (-total_pnl) as u64;
            user_account.total_collateral =
                user_account.total_collateral.checked_sub(loss).unwrap_or(0);
        }

        user_account.total_pnl = user_account
            .total_pnl
            .checked_add(total_pnl)
            .ok_or(error!(PositionError::ArithmeticOverflow))?;

        user_account.position_count = user_account
            .position_count
            .checked_sub(1)
            .ok_or(error!(PositionError::ArithmeticOverflow))?;

        position.status = PositionStatus::Closed;
        position.last_update = Clock::get()?.unix_timestamp;

        emit!(PositionClosed {
            position: position_key,
            owner: owner_key,
            realized_pnl: total_pnl,
            final_price,
            timestamp: position.last_update,
        });

        msg!(
            "Position closed with PnL: {} at price: {}",
            total_pnl,
            final_price
        );

        Ok(())
    }

    pub fn add_collateral(ctx: Context<ModifyUserCollateral>, amount: u64) -> Result<()> {
        let user_account = &mut ctx.accounts.user_account;

        user_account.total_collateral = user_account
            .total_collateral
            .checked_add(amount)
            .ok_or(error!(PositionError::ArithmeticOverflow))?;

        msg!("Added {} collateral", amount);

        Ok(())
    }
}
