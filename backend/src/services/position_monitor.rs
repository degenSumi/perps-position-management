use crate::domain::{Position, Side};
use crate::infrastructure::{OracleClient, SolanaClient};
use crate::services::on_chain_types::{deserialize_position_account, OnChainPosition};
use crate::services::{
    LiquidationAlert, LiquidationAlertConfig, LiquidationAlertService, MarginCalculator,
};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use redis::AsyncCommands;
use rust_decimal::Decimal;
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};

/// Price update event
#[derive(Debug, Clone)]
pub struct PriceUpdate {
    pub symbol: String,
    pub price: Decimal,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Position update event
#[derive(Debug, Clone)]
pub struct PositionUpdate {
    pub position_account: Pubkey,
    pub symbol: String,
    pub side: Side,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub mark_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub margin_ratio: Decimal,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub pnl_update_interval_ms: u64,
    pub position_refresh_interval_ms: u64,
    pub maintenance_margin_ratio: Decimal,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            pnl_update_interval_ms: 2000,
            position_refresh_interval_ms: 2000,
            maintenance_margin_ratio: Decimal::from_str_exact("0.025").unwrap(),
        }
    }
}

/// Position Monitor
pub struct PositionMonitor {
    solana_client: Arc<SolanaClient>,
    oracle_client: Arc<RwLock<OracleClient>>,
    rpc_client: Arc<RpcClient>,
    config: MonitorConfig,
    redis_client: redis::Client,

    /// Global position state: position_account -> Position
    positions: Arc<RwLock<HashMap<Pubkey, Position>>>,

    /// Asset-based lookup: asset_symbol -> Vec<position_account>
    positions_by_asset: Arc<RwLock<HashMap<String, Vec<Pubkey>>>>,

    /// User-based lookup: owner -> Vec<position_account>
    positions_by_user: Arc<RwLock<HashMap<Pubkey, Vec<Pubkey>>>>,

    position_update_tx: broadcast::Sender<PositionUpdate>,
    price_update_tx: broadcast::Sender<PriceUpdate>,
    liquidation_service: Arc<LiquidationAlertService>,
    running: Arc<RwLock<bool>>,
}

impl PositionMonitor {
    pub fn new(
        solana_client: Arc<SolanaClient>,
        oracle_client: Arc<RwLock<OracleClient>>,
        config: MonitorConfig,
        redis_url: String,
    ) -> Result<Self> {
        let (position_update_tx, _) = broadcast::channel(1000);
        let (price_update_tx, _) = broadcast::channel(100);

        let redis_client =
            redis::Client::open(redis_url.clone()).context("Failed to create Redis client")?;

        let (liquidation_service, _alert_rx) =
            LiquidationAlertService::new(redis_url, LiquidationAlertConfig::default())?;

        Ok(Self {
            rpc_client: Arc::new(RpcClient::new(solana_client.rpc_url.clone())),
            solana_client,
            oracle_client,
            config,
            redis_client,
            positions: Arc::new(RwLock::new(HashMap::new())),
            positions_by_asset: Arc::new(RwLock::new(HashMap::new())),
            positions_by_user: Arc::new(RwLock::new(HashMap::new())),
            position_update_tx,
            price_update_tx,
            liquidation_service: Arc::new(liquidation_service),
            running: Arc::new(RwLock::new(false)),
        })
    }

    pub fn subscribe_positions(&self) -> broadcast::Receiver<PositionUpdate> {
        self.position_update_tx.subscribe()
    }

    pub fn subscribe_prices(&self) -> broadcast::Receiver<PriceUpdate> {
        self.price_update_tx.subscribe()
    }

    pub fn subscribe_liquidation_alerts(&self) -> broadcast::Receiver<LiquidationAlert> {
        self.liquidation_service.subscribe()
    }

    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(anyhow!("Monitor already running"));
        }
        *running = true;
        drop(running);

        info!("Starting position monitor");

        self.spawn_price_monitor();
        self.spawn_position_refresher();
        self.spawn_pnl_updater();

        Ok(())
    }

    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Position monitor stopped");
    }

    fn spawn_price_monitor(&self) {
        let monitor = self.clone_for_task();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(1000));

            loop {
                if !*monitor.running.read().await {
                    break;
                }

                ticker.tick().await;

                let oracle = monitor.oracle_client.read().await;
                let symbols = oracle.get_symbols();

                for symbol in symbols {
                    match oracle.fetch_price(&symbol).await {
                        Ok(price) => {
                            let update = PriceUpdate {
                                symbol: symbol.clone(),
                                price,
                                timestamp: Utc::now(),
                            };

                            info!("Price update: {} = {}", symbol, price);

                            let _ = monitor.price_update_tx.send(update);

                            if let Err(e) = monitor
                                .liquidation_service
                                .check_liquidations_for_price_update(&symbol, price)
                                .await
                            {
                                error!("Failed to check liquidations for {}: {}", symbol, e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to fetch price for {}: {}", symbol, e);
                        }
                    }
                }
            }

            info!("Price monitor stopped");
        });
    }

    fn spawn_position_refresher(&self) {
        let monitor = self.clone_for_task();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(
                monitor.config.position_refresh_interval_ms,
            ));

            loop {
                if !*monitor.running.read().await {
                    break;
                }

                ticker.tick().await;

                if let Err(e) = monitor.refresh_positions_from_chain().await {
                    error!("Failed to refresh positions: {}", e);
                }
            }

            info!("Position refresher stopped");
        });
    }

    fn spawn_pnl_updater(&self) {
        let monitor = self.clone_for_task();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(monitor.config.pnl_update_interval_ms));

            loop {
                if !*monitor.running.read().await {
                    break;
                }

                ticker.tick().await;

                if let Err(e) = monitor.update_all_pnl().await {
                    error!("Failed to update PnL: {}", e);
                }
            }

            info!("PnL updater stopped");
        });
    }

    async fn refresh_positions_from_chain(&self) -> Result<()> {
        info!("Refreshing positions from chain...");

        let program_id = self.solana_client.program_id;

        let config = RpcProgramAccountsConfig {
            filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new(
                0,
                MemcmpEncodedBytes::Bytes(OnChainPosition::DISCRIMINATOR.to_vec()),
            ))]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                commitment: Some(CommitmentConfig::confirmed()),
                ..Default::default()
            },
            with_context: Some(false),
        };

        // Fetch all position accounts
        // Runs at a small interval
        // Supplement to this would be listening to grpc updates and updating positions accordingly, and keeping this interval bigger
        let accounts = self
            .rpc_client
            .get_program_accounts_with_config(&program_id, config)
            .context("Failed to fetch program accounts")?;

        info!("Found {} position accounts on chain", accounts.len());

        let mut seen_positions = HashMap::new();

        for (pubkey, account) in accounts {
            match deserialize_position_account(pubkey, &account) {
                Ok((position_index, on_chain_position)) => {
                    match on_chain_position.to_domain_position(pubkey, position_index) {
                        Ok(position) => {
                            let position_account = position.position_account;
                            seen_positions.insert(position_account, true);


                            // Check if already exists
                            if self.get_position(position_account).await.is_some() {
                                self.update_position(position).await?;
                            } else {
                                self.add_position(position).await?;
                                info!("Added new position {}", position_account);
                            }
                        }
                        Err(e) => {
                            error!("Failed to convert position at {}: {}", pubkey, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to deserialize position at {}: {}", pubkey, e);
                }
            }
        }

        // Remove stale positions
        let all_positions = self.get_all_positions().await;
        for position in all_positions {
            if !seen_positions.contains_key(&position.position_account) && position.is_open() {
                info!("Removing closed position {}", position.position_account);
                let _ = self.remove_position(position.position_account).await;
            }
        }

        info!("Position refresh completed");

        Ok(())
    }

    // Update unrealized PnL for all open positions in real time
    // As per the requirement, though we could do updates for unrealized PnL only on demand(when asked for a certain position/user)
    async fn update_all_pnl(&self) -> Result<()> {
        let mut positions = self.positions.write().await;
        let oracle = self.oracle_client.read().await;

        for position in positions.values_mut() {
            if !position.is_open() {
                continue;
            }

            let mark_price = match oracle.get_cached_price(&position.symbol).await {
                Some(price) => price,
                None => {
                    debug!("No price available for {}", position.symbol);
                    continue;
                }
            };

            position.mark_price = mark_price;

            match MarginCalculator::calculate_unrealized_pnl(
                position.side,
                position.size,
                mark_price,
                position.entry_price,
            ) {
                Ok(pnl) => {
                    position.unrealized_pnl = pnl;
                    position.last_update = Utc::now();

                    let margin_ratio = MarginCalculator::calculate_margin_ratio(
                        position.margin,
                        position.unrealized_pnl,
                        position.size,
                        position.mark_price,
                    )
                    .unwrap_or(Decimal::ZERO);

                    let update = PositionUpdate {
                        position_account: position.position_account,
                        symbol: position.symbol.clone(),
                        side: position.side,
                        size: position.size,
                        entry_price: position.entry_price,
                        mark_price: position.mark_price,
                        unrealized_pnl: position.unrealized_pnl,
                        margin_ratio,
                        timestamp: Utc::now(),
                    };

                    let _ = self.position_update_tx.send(update);
                }
                Err(e) => {
                    error!(
                        "Failed to calculate PnL for position {}: {}",
                        position.position_account, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Add position to global state
    pub async fn add_position(&self, position: Position) -> Result<()> {
        let position_account = position.position_account;
        let asset_symbol = position.symbol.clone();
        let owner = position.owner;

        // Add to global state
        let mut positions = self.positions.write().await;
        positions.insert(position_account, position.clone());
        drop(positions);

        // Add to asset lookup
        let mut positions_by_asset = self.positions_by_asset.write().await;
        positions_by_asset
            .entry(asset_symbol)
            .or_insert_with(Vec::new)
            .push(position_account);
        drop(positions_by_asset);

        // Add to user lookup
        let mut positions_by_user = self.positions_by_user.write().await;
        positions_by_user
            .entry(owner)
            .or_insert_with(Vec::new)
            .push(position_account);
        drop(positions_by_user);

        // Add to Redis only if its open
        if position.is_open() {
            self.add_to_redis_sorted_set(&position).await?;
        }

        info!("Added position {} to monitor", position_account);

        Ok(())
    }

    /// Update existing position
    pub async fn update_position(&self, position: Position) -> Result<()> {
        let position_account = position.position_account;
        
        let mut positions = self.positions.write().await;
        positions.insert(position_account, position);

        Ok(())
    }

    /// Remove position
    pub async fn remove_position(&self, position_account: Pubkey) -> Result<()> {
        let mut positions = self.positions.write().await;
        let position = positions
            .remove(&position_account)
            .ok_or_else(|| anyhow!("Position not found"))?;
        drop(positions);

        // Remove from asset lookup
        let mut positions_by_asset = self.positions_by_asset.write().await;
        if let Some(accounts) = positions_by_asset.get_mut(&position.symbol) {
            accounts.retain(|acc| *acc != position_account);
        }
        drop(positions_by_asset);

        // Remove from user lookup
        let mut positions_by_user = self.positions_by_user.write().await;
        if let Some(accounts) = positions_by_user.get_mut(&position.owner) {
            accounts.retain(|acc| *acc != position_account);
        }
        drop(positions_by_user);

        // Remove from Redis
        self.remove_from_redis_sorted_set(&position).await?;

        info!("Removed position {} from monitor", position_account);

        Ok(())
    }

    async fn add_to_redis_sorted_set(&self, position: &Position) -> Result<()> {
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to get Redis connection")?;

        let key = match position.side {
            Side::Long => format!("liquidations:{}:long", position.symbol),
            Side::Short => format!("liquidations:{}:short", position.symbol),
        };

        let member = position.position_account.to_string();
        let score = position.liquidation_price.to_string();

        conn.zadd(&key, &member, &score).await?;

        info!(
            "Added {} to Redis sorted set {} with score {}",
            position.position_account, key, score
        );

        Ok(())
    }

    // After a position is closed or liquidated it will be removed from redis sorted set
    async fn remove_from_redis_sorted_set(&self, position: &Position) -> Result<()> {
        let mut conn: redis::aio::MultiplexedConnection = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to get Redis connection")?;

        let key = match position.side {
            Side::Long => format!("liquidations:{}:long", position.symbol),
            Side::Short => format!("liquidations:{}:short", position.symbol),
        };

        let member = position.position_account.to_string();
        conn.zrem(&key, &member).await?;

        Ok(())
    }

    /// Get a specific position by account
    pub async fn get_position(&self, position_account: Pubkey) -> Option<Position> {
        let positions = self.positions.read().await;
        positions.get(&position_account).cloned()
    }

    /// Get positions by user
    pub async fn get_user_positions(&self, owner: &Pubkey) -> Result<Vec<Position>> {
        let positions_by_user = self.positions_by_user.read().await;
        let position_accounts = positions_by_user.get(owner).cloned().unwrap_or_default();
        drop(positions_by_user);

        let positions = self.positions.read().await;
        let user_positions: Vec<Position> = position_accounts
            .iter()
            .filter_map(|account| positions.get(account).cloned())
            .collect();

        Ok(user_positions)
    }

    /// Get positions by asset
    pub async fn get_positions_by_asset(&self, asset_symbol: &str) -> Vec<Position> {
        let positions_by_asset = self.positions_by_asset.read().await;
        let position_accounts = positions_by_asset
            .get(asset_symbol)
            .cloned()
            .unwrap_or_default();
        drop(positions_by_asset);

        let positions = self.positions.read().await;
        position_accounts
            .iter()
            .filter_map(|account| positions.get(account).cloned())
            .collect()
    }

    /// Get all positions
    pub async fn get_all_positions(&self) -> Vec<Position> {
        let positions = self.positions.read().await;
        positions.values().cloned().collect()
    }

    /// Get statistics
    pub async fn get_statistics(&self) -> MonitorStatistics {
        let positions = self.positions.read().await;

        let mut stats = MonitorStatistics::default();
        stats.total_positions = positions.len();

        for position in positions.values() {
            if position.is_open() {
                stats.open_positions += 1;
                stats.total_unrealized_pnl = stats
                    .total_unrealized_pnl
                    .checked_add(position.unrealized_pnl)
                    .unwrap_or(stats.total_unrealized_pnl);
            }
        }

        let positions_by_asset = self.positions_by_asset.read().await;
        stats.assets_monitored = positions_by_asset.len();

        stats
    }

    fn clone_for_task(&self) -> Self {
        Self {
            solana_client: Arc::clone(&self.solana_client),
            oracle_client: Arc::clone(&self.oracle_client),
            rpc_client: Arc::clone(&self.rpc_client),
            config: self.config.clone(),
            redis_client: self.redis_client.clone(),
            positions: Arc::clone(&self.positions),
            positions_by_asset: Arc::clone(&self.positions_by_asset),
            positions_by_user: Arc::clone(&self.positions_by_user),
            position_update_tx: self.position_update_tx.clone(),
            price_update_tx: self.price_update_tx.clone(),
            liquidation_service: Arc::clone(&self.liquidation_service),
            running: Arc::clone(&self.running),
        }
    }

    pub async fn get_cached_price(&self, symbol: &str) -> Option<Decimal> {
        let oracle = self.oracle_client.read().await;
        oracle.get_cached_price(symbol).await
    }

    pub async fn get_monitored_symbols(&self) -> Vec<String> {
        let oracle = self.oracle_client.read().await;
        oracle.get_symbols()
    }

    pub async fn fetch_price(&self, symbol: &str) -> Result<Decimal> {
        let oracle = self.oracle_client.read().await;
        oracle.fetch_price(symbol).await
    }
}

#[derive(Debug, Clone, Default)]
pub struct MonitorStatistics {
    pub total_positions: usize,
    pub open_positions: usize,
    pub assets_monitored: usize,
    pub total_unrealized_pnl: Decimal,
}
