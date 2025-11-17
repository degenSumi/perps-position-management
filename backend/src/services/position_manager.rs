use crate::domain::{Position, PositionStatus, Side};
use crate::infrastructure::SolanaClient;
use crate::services::{MarginCalculator, PositionMonitor};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signature,
    system_program,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

// Program ID
const PROGRAM_ID: &str = "9bca4kbDn7uyQWQaqfKpe8hCdbBh6KqJFNbkzwHhieC3";

const DISCRIMINATOR_OPEN_POSITION: [u8; 8] = [135, 128, 47, 77, 15, 152, 240, 49];
const DISCRIMINATOR_CLOSE_POSITION: [u8; 8] = [123, 134, 81, 0, 49, 68, 98, 98];
const DISCRIMINATOR_MODIFY_POSITION: [u8; 8] = [48, 249, 6, 139, 14, 95, 106, 88];
const DISCRIMINATOR_INITIALIZE_USER: [u8; 8] = [111, 17, 185, 250, 60, 122, 38, 254];
const DISCRIMINATOR_ADD_COLLATERAL: [u8; 8] = [127, 82, 121, 42, 161, 176, 249, 206];

pub struct PositionManager {
    solana_client: Arc<SolanaClient>,
    monitor: Arc<PositionMonitor>,  // Shared state with monitor
}

impl PositionManager {
    pub fn new(solana_client: Arc<SolanaClient>, monitor: Arc<PositionMonitor>) -> Self {
        Self {
            solana_client,
            monitor,
        }
    }

    /// Initialize user account on-chain
    pub async fn initialize_user(&self, owner: &Pubkey) -> Result<Signature> {
        info!("Initializing user account for {}", owner);

        let program_id: Pubkey = PROGRAM_ID.parse()?;

        let (user_account, _bump) =
            Pubkey::find_program_address(&[b"user", owner.as_ref()], &program_id);

        debug!("User account PDA: {}", user_account);

        let mut data = Vec::new();
        data.extend_from_slice(&DISCRIMINATOR_INITIALIZE_USER);

        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(user_account, false),
                AccountMeta::new(*owner, true),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        };

        let signature = self.solana_client.send_transaction(&[instruction])?;

        info!("User initialized: {}", signature);
        Ok(signature)
    }

    /// Add collateral to user account
    pub async fn add_collateral(&self, owner: &Pubkey, amount: u64) -> Result<Signature> {
        info!("Adding collateral: {} for {}", amount, owner);

        let program_id: Pubkey = PROGRAM_ID.parse()?;

        let (user_account, _) =
            Pubkey::find_program_address(&[b"user", owner.as_ref()], &program_id);

        debug!("User account PDA: {}", user_account);

        let mut data = Vec::new();
        data.extend_from_slice(&DISCRIMINATOR_ADD_COLLATERAL);
        data.extend_from_slice(&amount.to_le_bytes());

        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(user_account, false),
                AccountMeta::new_readonly(*owner, true),
            ],
            data,
        };

        let signature = self.solana_client.send_transaction(&[instruction])?;

        info!("Collateral added: {}", signature);
        Ok(signature)
    }

    /// Open a new position on-chain
    pub async fn open_position(
        &self,
        owner: Pubkey,
        symbol: String,
        side: Side,
        size: Decimal,
        leverage: u16,
        entry_price: Decimal,
        maintenance_margin_ratio: Decimal,
    ) -> Result<(Position, Signature)> {
        info!(
            "Opening position: {} {:?} {} {}x @ ${}",
            symbol, side, size, leverage, entry_price
        );

        let size_u64 = decimal_to_u64(size, 8)?;
        let entry_price_u64 = decimal_to_u64(entry_price, 6)?;

        let margin = MarginCalculator::calculate_initial_margin(size, entry_price, leverage)?;

        let liquidation_price = MarginCalculator::calculate_liquidation_price(
            side,
            entry_price,
            leverage,
            maintenance_margin_ratio,
        )?;

        let program_id: Pubkey = PROGRAM_ID.parse()?;

        let (user_account, _) =
            Pubkey::find_program_address(&[b"user", owner.as_ref()], &program_id);

        let position_index = self.get_next_position_index(&owner).await?;

        let (position_account, bump) = Pubkey::find_program_address(
            &[b"position", owner.as_ref(), &position_index.to_le_bytes()],
            &program_id,
        );

        info!(
            "Position PDA: {} (index: {}, bump: {})",
            position_account, position_index, bump
        );

        let mut data = Vec::new();
        data.extend_from_slice(&DISCRIMINATOR_OPEN_POSITION);

        let symbol_bytes = symbol.as_bytes();
        data.extend_from_slice(&(symbol_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(symbol_bytes);

        let side_u8 = match side {
            Side::Long => 0u8,
            Side::Short => 1u8,
        };
        data.push(side_u8);

        data.extend_from_slice(&size_u64.to_le_bytes());
        data.extend_from_slice(&leverage.to_le_bytes());
        data.extend_from_slice(&entry_price_u64.to_le_bytes());

        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(position_account, false),
                AccountMeta::new(user_account, false),
                AccountMeta::new(owner, true),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        };

        let signature = self.solana_client.send_transaction(&[instruction])?;

        info!("Position opened on-chain: {}", signature);

        // Create position object
        let position = Position {
            position_index,
            owner,
            position_account,
            symbol: symbol.clone(),
            side,
            size,
            entry_price,
            mark_price: entry_price,
            margin,
            leverage,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            funding_accrued: Decimal::ZERO,
            liquidation_price,
            status: PositionStatus::Open,
            opened_at: Utc::now(),
            last_update: Utc::now(),
            closed_at: None,
        };

        // Register with monitor
        self.monitor.add_position(position.clone()).await?;

        Ok((position, signature))
    }

    /// Modify position on-chain
    pub async fn modify_position(
        &self,
        position_account: Pubkey,
        new_size: Option<Decimal>,
        margin_delta: Option<i64>,
    ) -> Result<Signature> {
        let position = self.get_position(position_account).await?;

        if !position.is_open() {
            return Err(anyhow!("Position is not open"));
        }

        info!(
            "Modifying position {}: new_size={:?}, margin_delta={:?}",
            position_account, new_size, margin_delta
        );

        let program_id: Pubkey = PROGRAM_ID.parse()?;

        let (user_account, _) =
            Pubkey::find_program_address(&[b"user", position.owner.as_ref()], &program_id);

        let mut data = Vec::new();
        data.extend_from_slice(&DISCRIMINATOR_MODIFY_POSITION);

        if let Some(size) = new_size {
            data.push(1);
            let size_u64 = decimal_to_u64(size, 8)?;
            data.extend_from_slice(&size_u64.to_le_bytes());
        } else {
            data.push(0);
        }

        if let Some(delta) = margin_delta {
            data.push(1);
            data.extend_from_slice(&delta.to_le_bytes());
        } else {
            data.push(0);
        }

        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(position.position_account, false),
                AccountMeta::new(user_account, false),
                AccountMeta::new_readonly(position.owner, true),
            ],
            data,
        };

        let signature = self.solana_client.send_transaction(&[instruction])?;

        info!("Position modified on-chain: {}", signature);

        Ok(signature)
    }

    /// Close a position on-chain
    pub async fn close_position(
        &self,
        position_account: Pubkey,
        final_price: Decimal,
    ) -> Result<(Decimal, Signature)> {
        let position = self.get_position(position_account).await?;

        if !position.is_open() {
            return Err(anyhow!("Position is not open"));
        }

        info!("Closing position {} at price ${}", position_account, final_price);

        let realized_pnl = MarginCalculator::calculate_unrealized_pnl(
            position.side,
            position.size,
            final_price,
            position.entry_price,
        )?;

        let total_pnl = realized_pnl
            .checked_add(position.funding_accrued)
            .ok_or_else(|| anyhow!("PnL overflow"))?;

        info!("Closing PnL: {}", total_pnl);

        let program_id: Pubkey = PROGRAM_ID.parse()?;

        let (user_account, _) =
            Pubkey::find_program_address(&[b"user", position.owner.as_ref()], &program_id);

        let mut data = Vec::new();
        data.extend_from_slice(&DISCRIMINATOR_CLOSE_POSITION);

        let final_price_u64 = decimal_to_u64(final_price, 6)?;
        data.extend_from_slice(&final_price_u64.to_le_bytes());

        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(position.position_account, false),
                AccountMeta::new(user_account, false),
                AccountMeta::new(position.owner, true),
            ],
            data,
        };

        let signature = self.solana_client.send_transaction(&[instruction])?;

        info!("Position closed on-chain: {}", signature);

        Ok((total_pnl, signature))
    }

    /// Get position from monitor's shared state
    pub async fn get_position(&self, position_account: Pubkey) -> Result<Position> {
        self.monitor
            .get_position(position_account)
            .await
            .ok_or_else(|| anyhow!("Position not found"))
    }

    /// Get user positions from monitor
    pub async fn get_user_positions(&self, owner: &Pubkey) -> Result<Vec<Position>> {
        self.monitor.get_user_positions(owner).await
    }

    /// Get open positions from monitor
    pub async fn get_open_positions(&self, owner: &Pubkey) -> Result<Vec<Position>> {
        let positions = self.get_user_positions(owner).await?;
        Ok(positions.into_iter().filter(|p| p.is_open()).collect())
    }

    /// Get user account from chain
    pub async fn get_user_account(&self, owner: &Pubkey) -> Result<UserAccountData> {
        let program_id: Pubkey = PROGRAM_ID.parse()?;

        let (user_account, _) =
            Pubkey::find_program_address(&[b"user", owner.as_ref()], &program_id);

        let rpc_client = RpcClient::new(&self.solana_client.rpc_url);
        let account_data = rpc_client
            .get_account_data(&user_account)
            .context("Failed to fetch user account")?;

        let data = &account_data[8..];

        let owner_bytes: [u8; 32] = data[0..32].try_into()?;
        let owner_pubkey = Pubkey::new_from_array(owner_bytes);

        let total_collateral = u64::from_le_bytes(data[32..40].try_into()?);
        let locked_collateral = u64::from_le_bytes(data[40..48].try_into()?);
        let total_pnl = i64::from_le_bytes(data[48..56].try_into()?);
        let position_count = u32::from_le_bytes(data[56..60].try_into()?);
        let position_count_total = u32::from_le_bytes(data[60..64].try_into()?);
        let bump = data[64];

        Ok(UserAccountData {
            owner: owner_pubkey,
            total_collateral,
            locked_collateral,
            total_pnl,
            position_count,
            position_count_total,
            bump,
        })
    }

    /// Get next position index from on-chain user account
    async fn get_next_position_index(&self, owner: &Pubkey) -> Result<u32> {
        match self.get_user_account(owner).await {
            Ok(user_account) => {
                info!(
                    "Fetched position_count_total from chain: {}",
                    user_account.position_count_total
                );
                Ok(user_account.position_count_total)
            }
            Err(e) => {
                warn!(
                    "Could not fetch user account (might not be initialized): {}",
                    e
                );
                Ok(0)
            }
        }
    }
    
    /// Get statistics from monitor
    pub async fn get_statistics(&self) -> Result<PositionStats> {
        let monitor_stats = self.monitor.get_statistics().await;
        
        Ok(PositionStats {
            total_positions: monitor_stats.total_positions,
            open_positions: monitor_stats.open_positions,
            closed_positions: monitor_stats.total_positions - monitor_stats.open_positions,
            total_unrealized_pnl: monitor_stats.total_unrealized_pnl,
            total_realized_pnl: Decimal::ZERO, // Monitor doesn't track realized PnL
        })
    }
}

/// Helper: Convert Decimal to u64 with precision
fn decimal_to_u64(decimal: Decimal, precision: u32) -> Result<u64> {
    let multiplier = 10u64.pow(precision);
    let scaled = decimal
        .checked_mul(Decimal::from(multiplier))
        .ok_or_else(|| anyhow!("Overflow in decimal conversion"))?;

    let value = scaled
        .to_u64()
        .ok_or_else(|| anyhow!("Cannot convert decimal to u64"))?;

    Ok(value)
}

// User account data structure
#[derive(Debug, Clone)]
pub struct UserAccountData {
    pub owner: Pubkey,
    pub total_collateral: u64,
    pub locked_collateral: u64,
    pub total_pnl: i64,
    pub position_count: u32,
    pub position_count_total: u32,
    pub bump: u8,
}

#[derive(Debug, Clone)]
pub struct PositionStats {
    pub total_positions: usize,
    pub open_positions: usize,
    pub closed_positions: usize,
    pub total_unrealized_pnl: Decimal,
    pub total_realized_pnl: Decimal,
}
