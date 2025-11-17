use crate::domain::{Position, PositionStatus, Side};
use anyhow::{Result, anyhow, Context};
use rust_decimal::Decimal;
use solana_sdk::{
    pubkey::Pubkey,
    account::Account,
};

use chrono::Utc;

use anchor_lang::{AnchorDeserialize, AnchorSerialize};

/// On-chain Side enum
#[derive(AnchorDeserialize, AnchorSerialize, Debug, Clone, Copy, PartialEq)]
pub enum OnChainSide {
    Long,
    Short,
}

/// On-chain PositionStatus enum
#[derive(AnchorDeserialize, AnchorSerialize, Debug, Clone, Copy, PartialEq)]
pub enum OnChainPositionStatus {
    Opening,
    Open,
    Modifying,
    Closing,
    Closed,
}

/// On-chain Position account structure
#[derive(AnchorDeserialize, AnchorSerialize, Debug)]
pub struct OnChainPosition {
    pub owner: Pubkey,
    pub symbol: String,
    pub side: OnChainSide,
    pub size: u64,
    pub entry_price: u64,
    pub margin: u64,
    pub leverage: u16,
    pub unrealized_pnl: i64,
    pub realized_pnl: i64,
    pub funding_accrued: i64,
    pub liquidation_price: u64,
    pub last_update: i64,
    pub status: OnChainPositionStatus,
    pub bump: u8,
}

impl OnChainPosition {
    pub const DISCRIMINATOR: [u8; 8] = [
        0xaa, 0xbc, 0x8f, 0xe4, 0x7a, 0x40, 0xf7, 0xd0,
    ];
    /// Convert to domain Position model
    pub fn to_domain_position(&self, position_account: Pubkey, position_index: u32) -> Result<Position> {
        // Convert side
        let side = match self.side {
            OnChainSide::Long => Side::Long,
            OnChainSide::Short => Side::Short,
        };
        
        // Convert status
        let status = match self.status {
            OnChainPositionStatus::Opening => PositionStatus::Opening,
            OnChainPositionStatus::Open => PositionStatus::Open,
            OnChainPositionStatus::Modifying => PositionStatus::Modifying,
            OnChainPositionStatus::Closing => PositionStatus::Closing,
            OnChainPositionStatus::Closed => PositionStatus::Closed,
        };
        
        // Convert fixed-point numbers to Decimal (considering 6 decimals)
        let size_decimal = Decimal::new(self.size as i64, 6);
        let entry_price_decimal = Decimal::new(self.entry_price as i64, 6);
        let margin_decimal = Decimal::new(self.margin as i64, 6);
        let unrealized_pnl_decimal = Decimal::new(self.unrealized_pnl, 6);
        let realized_pnl_decimal = Decimal::new(self.realized_pnl, 6);
        let funding_accrued_decimal = Decimal::new(self.funding_accrued, 6);
        let liquidation_price_decimal = Decimal::new(self.liquidation_price as i64, 6);
        
        // Map symbol
        // let oracle_symbol = self.symbol.replace("-USDT", "-USD");

        fn normalize_symbol(symbol: &str) -> String {
            for stable in ["USDT", "USDC", "DAI"].iter() {
                if let Some(base) = symbol.strip_suffix(&format!("-{}", stable)) {
                        return format!("{}-USD", base);
                    }
                }
                symbol.to_string()
        }

        let oracle_symbol = normalize_symbol(&self.symbol);
        
        Ok(Position {
            position_index,
            owner: self.owner,
            position_account,
            symbol: oracle_symbol,
            side,
            size: size_decimal,
            entry_price: entry_price_decimal,
            mark_price: entry_price_decimal,
            margin: margin_decimal,
            leverage: self.leverage,
            unrealized_pnl: unrealized_pnl_decimal,
            realized_pnl: realized_pnl_decimal,
            funding_accrued: funding_accrued_decimal,
            liquidation_price: liquidation_price_decimal,
            status,
            opened_at: chrono::DateTime::from_timestamp(self.last_update, 0)
                .unwrap_or_else(|| Utc::now()),
            last_update: chrono::DateTime::from_timestamp(self.last_update, 0)
                .unwrap_or_else(|| Utc::now()),
            closed_at: if status == PositionStatus::Closed {
                Some(chrono::DateTime::from_timestamp(self.last_update, 0)
                    .unwrap_or_else(|| Utc::now()))
            } else {
                None
            },
        })
    }
}

/// On-chain UserAccount structure
#[derive(AnchorDeserialize, AnchorSerialize, Debug)]
pub struct OnChainUserAccount {
    pub owner: Pubkey,
    pub total_collateral: u64,
    pub locked_collateral: u64,
    pub total_pnl: i64,
    pub position_count: u32,
    pub position_count_total: u32,
    pub bump: u8,
}

/// Deserialize Position account from Solana account data
pub fn deserialize_position_account(
    pubkey: Pubkey,
    account: &Account,
) -> Result<(u32, OnChainPosition)> {
    let data = &account.data;
    
    if data.len() < 8 {
        return Err(anyhow!("Account data too small"));
    }
    
    // Skip 8-byte discriminator and deserialize
    let position = OnChainPosition::deserialize(&mut &data[8..])
        .context("Failed to deserialize Position")?;
    
    Ok((0, position))
}
