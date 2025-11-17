/// Liquidation Alert Service
/// Uses Redis sorted sets to track positions nearing liquidation prices 
/// Optimal range queries for quick and efficient checks
use anyhow::{Result, Context};
use redis::AsyncCommands;
use rust_decimal::Decimal;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::domain::{ Side, Risk };

#[derive(Debug, Clone)]
pub struct LiquidationAlert {
    pub position_account: Pubkey,
    pub symbol: String,
    pub side: Side,
    pub liquidation_price: Decimal,
    pub current_price: Decimal,
    pub risk_type: Risk,
}

#[derive(Debug, Clone)]
pub struct LiquidationAlertConfig {
    pub alert_threshold_pct: Decimal,
}

impl Default for LiquidationAlertConfig {
    fn default() -> Self {
        Self {
            alert_threshold_pct: Decimal::new(10, 2), // 10%
        }
    }
}

pub struct LiquidationAlertService {
    redis_client: redis::Client,
    config: LiquidationAlertConfig,
    alert_tx: broadcast::Sender<LiquidationAlert>,
}

impl LiquidationAlertService {
    pub fn new(
        redis_url: String,
        config: LiquidationAlertConfig,
    ) -> Result<(Self, broadcast::Receiver<LiquidationAlert>)> {
        let redis_client = redis::Client::open(redis_url)?;
        let (alert_tx, alert_rx) = broadcast::channel(1000);
        
        Ok((
            Self {
                redis_client,
                config,
                alert_tx,
            },
            alert_rx,
        ))
    }
    
    /// Check liquidations using Redis range queries
    pub async fn check_liquidations_for_price_update(
        &self,
        symbol: &str,
        current_price: Decimal,
    ) -> Result<Vec<Pubkey>> {
        let threshold = self.config.alert_threshold_pct;
        let mut at_risk_position_accounts = Vec::new();
        
        // Check LONG positions
        let long_lower_bound = current_price * (Decimal::ONE - threshold);
        let long_to_liquidate = self.get_positions_in_range(
            &format!("liquidations:{}:long", symbol),
            current_price,
            u64::MAX.into(),
        ).await?;

        let long_at_risk = self.get_positions_in_range(
            &format!("liquidations:{}:long", symbol),
            long_lower_bound,
            current_price,
        ).await?;
        
        info!("asset: {}, long_at_risk: {:?} current_price: {:?}, count {}", symbol, long_lower_bound, current_price, long_at_risk.len()); 
        info!("asset: {}, current_price: {:?}, count {}", symbol, current_price, long_to_liquidate.len()); 
        
        for position_account_str in long_to_liquidate {
            if let Ok(position_account) = position_account_str.parse::<Pubkey>() {
                at_risk_position_accounts.push(position_account);
                
                if let Ok(liq_price) = self.get_liquidation_price(
                    &format!("liquidations:{}:long", symbol),
                    &position_account_str,
                ).await {
                    self.emit_alert(
                        position_account,
                        symbol,
                        Side::Long,
                        liq_price,
                        current_price,
                        Risk::Liquidated
                    ).await;
                    // Simulate by removing the liquidated position from Redis
                    self.remove_from_redis_sorted_set(symbol, Side::Long, position_account).await?;
                }
            }
        }

        for position_account_str in long_at_risk {
            if let Ok(position_account) = position_account_str.parse::<Pubkey>() {
                at_risk_position_accounts.push(position_account);
                
                if let Ok(liq_price) = self.get_liquidation_price(
                    &format!("liquidations:{}:long", symbol),
                    &position_account_str,
                ).await {
                    self.emit_alert(
                        position_account,
                        symbol,
                        Side::Long,
                        liq_price,
                        current_price,
                        Risk::Liquidating
                    ).await;
                    // Simulate by removing the liquidated position from Redis
                    self.remove_from_redis_sorted_set(symbol, Side::Long, position_account).await?;
                }
            }
        }
        
        // Check SHORT positions
        let short_upper_bound = current_price * (Decimal::ONE + threshold);
        let short_at_liquidation = self.get_positions_in_range(
            &format!("liquidations:{}:short", symbol),
            0.into(),
            current_price,
        ).await?;

        let short_at_risk = self.get_positions_in_range(
            &format!("liquidations:{}:short", symbol),
            current_price,
            short_upper_bound,
        ).await?;
        
        for position_account_str in short_at_liquidation {
            if let Ok(position_account) = position_account_str.parse::<Pubkey>() {
                at_risk_position_accounts.push(position_account);
                
                if let Ok(liq_price) = self.get_liquidation_price(
                    &format!("liquidations:{}:short", symbol),
                    &position_account_str,
                ).await {
                    self.emit_alert(
                        position_account,
                        symbol,
                        Side::Short,
                        liq_price,
                        current_price,
                        Risk::Liquidated
                    ).await;
                    // Simulate by removing the liquidated position from Redis
                    self.remove_from_redis_sorted_set(symbol, Side::Short, position_account).await?;
                }
            }
        }

         for position_account_str in short_at_risk {
            if let Ok(position_account) = position_account_str.parse::<Pubkey>() {
                at_risk_position_accounts.push(position_account);
                
                if let Ok(liq_price) = self.get_liquidation_price(
                    &format!("liquidations:{}:short", symbol),
                    &position_account_str,
                ).await {
                    self.emit_alert(
                        position_account,
                        symbol,
                        Side::Short,
                        liq_price,
                        current_price,
                        Risk::Liquidating
                    ).await;
                    // Simulate by removing the liquidated position from Redis
                    self.remove_from_redis_sorted_set(symbol, Side::Short, position_account).await?;
                }
            }
        }
        
        if !at_risk_position_accounts.is_empty() {
            info!(
                "Found {} positions at risk for {} at price ${:.2}",
                at_risk_position_accounts.len(),
                symbol,
                current_price
            );
        }
        
        Ok(at_risk_position_accounts)
    }
    
    /// Get positions in liquidation price range using ZRANGEBYSCORE
    async fn get_positions_in_range(
        &self,
        key: &str,
        min_score: Decimal,
        max_score: Decimal,
    ) -> Result<Vec<String>> {
        let mut conn = self.redis_client
            .get_multiplexed_async_connection()
            .await?;
        
        let min = min_score.to_string();
        let max = max_score.to_string();
        
        let position_accounts: Vec<String> = conn
            .zrangebyscore(key, &min, &max)
            .await
            .context("Failed to query Redis sorted set")?;
        
        Ok(position_accounts)
    }
    
    /// Get liquidation price for a position
    async fn get_liquidation_price(&self, key: &str, member: &str) -> Result<Decimal> {
        let mut conn = self.redis_client
            .get_multiplexed_async_connection()
            .await?;
        
        let score: Option<String> = conn.zscore(key, member).await?;
        
        score
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow::anyhow!("Failed to get liquidation price"))
    }
    
    // After a position is closed or liquidated it will be removed from redis sorted set
    async fn remove_from_redis_sorted_set(&self, symbol: &str, side: Side, position_account: Pubkey) -> Result<()> {
        let mut conn: redis::aio::MultiplexedConnection = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to get Redis connection")?;

        let key = match side {
            Side::Long => format!("liquidations:{}:long", symbol),
            Side::Short => format!("liquidations:{}:short", symbol),
        };

        let member = position_account.to_string();
        conn.zrem(&key, &member).await?;

        Ok(())
    }

    /// Emit liquidation alert
    async fn emit_alert(
        &self,
        position_account: Pubkey,
        symbol: &str,
        side: Side,
        liquidation_price: Decimal,
        current_price: Decimal,
        risk_type: Risk
    ) {
        let alert: LiquidationAlert = LiquidationAlert {
            position_account,  
            symbol: symbol.to_string(),
            side,
            liquidation_price,
            current_price,
            risk_type: risk_type
        };
        
        warn!(
            "{:?} ALERT: {:?} {:?} position {} - Current: ${:.2}, Liquidation: ${:.2}",
            risk_type, symbol, side, position_account, current_price, liquidation_price
        );
        
        let _ = self.alert_tx.send(alert);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<LiquidationAlert> {
        self.alert_tx.subscribe()
    }
}
