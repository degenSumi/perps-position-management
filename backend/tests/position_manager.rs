use anyhow::Result;
use perpetual_backend::domain::Side;
use perpetual_backend::infrastructure::{OracleClient, SolanaClient};
use perpetual_backend::services::{MonitorConfig, PositionManager, PositionMonitor};
use rust_decimal_macros::dec;
use solana_sdk::signer::Signer;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

#[tokio::test(flavor = "multi_thread")]
async fn test_open_position_on_chain() -> Result<()> {
    dotenvy::dotenv().ok();
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();

    info!("Starting Position Manager Integration Test");

    let program_id = Pubkey::from_str("9bca4kbDn7uyQWQaqfKpe8hCdbBh6KqJFNbkzwHhieC3")?;
    let rpc_url = "https://api.devnet.solana.com".to_string();

    let private_key =
        std::env::var("SOLANA_PRIVATE_KEY").expect("SOLANA_PRIVATE_KEY not set in .env");
    let payer: Arc<Keypair> = Arc::new(Keypair::from_base58_string(&private_key));

    info!("Test Wallet: {}", payer.pubkey());
    info!(
        "WARNING: Ensure wallet has SOL: solana airdrop 2 {} --url devnet",
        payer.pubkey()
    );
    info!("");

    let solana_client = Arc::new(SolanaClient::new(program_id, Arc::clone(&payer), rpc_url));

    // Initialize Oracle client
    let oracle_client = Arc::new(RwLock::new(
        OracleClient::new_hermes().with_mainnet_defaults(),
    ));
    info!("Oracle client initialized");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());

    // Initialize Position Monitor
    let monitor: Arc<PositionMonitor> = Arc::new(PositionMonitor::new(
        Arc::clone(&solana_client),
        oracle_client,
        MonitorConfig::default(),
        redis_url,
    )?);
    info!("Position monitor created");

    // Create position manager
    let manager = PositionManager::new(Arc::clone(&solana_client), Arc::clone(&monitor));

    // Step 1: Initialize user account (only needs to be done once)
    info!("Initializing user account...");

    match manager.initialize_user(&payer.pubkey()).await {
        Ok(sig) => info!("User initialized: {}", sig),
        Err(e) => {
            if e.to_string().contains("already in use") {
                info!("User account already initialized");
            } else {
                info!("Initialization skipped or failed: {}", e);
            }
        }
    }

    info!("\nStep 2: Add Collateral");
    let collateral_amount = 1_000_000 * 1_000_000; // 1,000,000 USDT
    let sig = manager
        .add_collateral(&payer.pubkey(), collateral_amount)
        .await?;
    info!("Collateral added: {}", sig);

    // Wait a bit for confirmation

    // Step 2: Open a position
    info!("Opening BTC Long position...");

    let (position, signature) = manager
        .open_position(
            payer.pubkey(),
            "BTC-USD".to_string(),
            Side::Long,
            dec!(0.1),   // 0.1 BTC
            10,          // 10x leverage
            dec!(98000), // Entry price $98,000
            dec!(0.025), // 2.5% maintenance margin
        )
        .await?;

    info!("Position opened!");
    info!("   Transaction: {}", signature);
    info!("   Position ID: {}", position.position_account);
    info!("   Position Account: {}", position.position_account);
    info!("   Symbol: {}", position.symbol);
    info!("   Side: {:?}", position.side);
    info!("   Size: {}", position.size);
    info!("   Entry Price: ${}", position.entry_price);
    info!("   Leverage: {}x", position.leverage);
    info!("   Margin: ${}", position.margin);
    info!("   Liquidation Price: ${}", position.liquidation_price);

    // Step 3: Verify position was stored locally
    let retrieved = manager.get_position(position.position_account).await?;
    assert_eq!(retrieved.symbol, "BTC-USD");
    assert_eq!(retrieved.size, dec!(0.1));

    info!("Position verified in local state");

    // Step 4: Get all user positions
    let user_positions = manager.get_user_positions(&payer.pubkey()).await?;
    info!("User has {} total positions", user_positions.len());

    // Step 5: Get open positions only
    let open_positions = manager.get_open_positions(&payer.pubkey()).await?;
    info!("User has {} open positions", open_positions.len());

    // Step 6: Get statistics
    let stats = manager.get_statistics().await?;
    info!("Statistics:");
    info!("   Total positions: {}", stats.total_positions);
    info!("   Open positions: {}", stats.open_positions);
    info!("   Closed positions: {}", stats.closed_positions);

    info!("All tests passed!");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_modify_position_on_chain() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();

    dotenvy::dotenv().ok();

    info!("Testing Position Modification");

    let program_id = Pubkey::from_str("9bca4kbDn7uyQWQaqfKpe8hCdbBh6KqJFNbkzwHhieC3")?;
    let rpc_url = "https://api.devnet.solana.com".to_string();

    let private_key =
        std::env::var("SOLANA_PRIVATE_KEY").expect("SOLANA_PRIVATE_KEY not set in .env");
    let payer: Arc<Keypair> = Arc::new(Keypair::from_base58_string(&private_key));

    info!("Test Wallet: {}", payer.pubkey());
    info!(
        "WARNING: Ensure wallet has SOL: solana airdrop 2 {} --url devnet",
        payer.pubkey()
    );
    info!("");

    let solana_client = Arc::new(SolanaClient::new(program_id, Arc::clone(&payer), rpc_url));

    // Initialize Oracle client
    let oracle_client = Arc::new(RwLock::new(
        OracleClient::new_hermes().with_mainnet_defaults(),
    ));
    info!("Oracle client initialized");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());

    // Initialize Position Monitor
    let monitor: Arc<PositionMonitor> = Arc::new(PositionMonitor::new(
        Arc::clone(&solana_client),
        oracle_client,
        MonitorConfig::default(),
        redis_url,
    )?);
    info!("Position monitor created");

    // Create position manager
    let manager = PositionManager::new(Arc::clone(&solana_client), Arc::clone(&monitor));

    // First, open a position
    info!("Opening position...");
    let (position, _) = manager
        .open_position(
            payer.pubkey(),
            "ETH-USD".to_string(),
            Side::Long,
            dec!(1.0),
            10,
            dec!(3500),
            dec!(0.025),
        )
        .await?;

    info!("Position opened: {}", position.position_account);

    // Now modify it
    info!("Modifying position size...");
    let signature = manager
        .modify_position(
            position.position_account,
            Some(dec!(2.0)), // Double the size to 2.0 ETH
            None,            // No margin change
        )
        .await?;

    info!("Position modified: {}", signature);

    info!("Position modification test passed!");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_close_position_on_chain() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();

    dotenvy::dotenv().ok();

    info!("Testing Position Closing");
    let program_id = Pubkey::from_str("9bca4kbDn7uyQWQaqfKpe8hCdbBh6KqJFNbkzwHhieC3")?;
    let rpc_url = "https://api.devnet.solana.com".to_string();

    let private_key =
        std::env::var("SOLANA_PRIVATE_KEY").expect("SOLANA_PRIVATE_KEY not set in .env");
    let payer: Arc<Keypair> = Arc::new(Keypair::from_base58_string(&private_key));

    info!("Test Wallet: {}", payer.pubkey());
    info!(
        "WARNING: Ensure wallet has SOL: solana airdrop 2 {} --url devnet",
        payer.pubkey()
    );
    info!("");

    let solana_client = Arc::new(SolanaClient::new(program_id, Arc::clone(&payer), rpc_url));

    // Initialize Oracle client
    let oracle_client = Arc::new(RwLock::new(
        OracleClient::new_hermes().with_mainnet_defaults(),
    ));
    info!("Oracle client initialized");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());

    // Initialize Position Monitor
    let monitor: Arc<PositionMonitor> = Arc::new(PositionMonitor::new(
        Arc::clone(&solana_client),
        oracle_client,
        MonitorConfig::default(),
        redis_url,
    )?);
    info!("Position monitor created");

    // Create position manager
    let manager = PositionManager::new(Arc::clone(&solana_client), Arc::clone(&monitor));

    // Open a position
    info!("Opening position...");
    let (position, _) = manager
        .open_position(
            payer.pubkey(),
            "SOL-USD".to_string(),
            Side::Short,
            dec!(10.0),
            5,
            dec!(240),
            dec!(0.025),
        )
        .await?;

    info!("Position opened: {}", position.position_account);

    // Close the position at a different price (simulating profit)
    info!("Closing position...");
    let close_price = dec!(230); // Price dropped, short profits
    let (pnl, signature) = manager
        .close_position(position.position_account, close_price)
        .await?;

    info!("Position closed: {}", signature);
    info!("Realized PnL: ${}", pnl);

    info!("Position close test passed!");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_full_position_lifecycle() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();
    dotenvy::dotenv().ok();

    info!("Testing Full Position Lifecycle");
    info!("===================================");

    let program_id = Pubkey::from_str("9bca4kbDn7uyQWQaqfKpe8hCdbBh6KqJFNbkzwHhieC3")?;
    let rpc_url = "https://api.devnet.solana.com".to_string();

    let private_key =
        std::env::var("SOLANA_PRIVATE_KEY").expect("SOLANA_PRIVATE_KEY not set in .env");
    let payer: Arc<Keypair> = Arc::new(Keypair::from_base58_string(&private_key));

    info!("Test Wallet: {}", payer.pubkey());
    info!(
        "WARNING: Ensure wallet has SOL: solana airdrop 2 {} --url devnet",
        payer.pubkey()
    );
    info!("");

    let solana_client = Arc::new(SolanaClient::new(program_id, Arc::clone(&payer), rpc_url));

    // Initialize Oracle client
    let oracle_client = Arc::new(RwLock::new(
        OracleClient::new_hermes().with_mainnet_defaults(),
    ));
    info!("Oracle client initialized");

    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());

    // Initialize Position Monitor
    let monitor: Arc<PositionMonitor> = Arc::new(PositionMonitor::new(
        Arc::clone(&solana_client),
        oracle_client,
        MonitorConfig::default(),
        redis_url,
    )?);
    info!("Position monitor created");

    // Create position manager
    let manager = PositionManager::new(Arc::clone(&solana_client), Arc::clone(&monitor));

    // Step 1: Initialize user account (only needs to be done once)
    info!("Initializing user account...");

    // match manager.initialize_user(&payer.pubkey()).await {
    //     Ok(sig) => info!("User initialized: {}", sig),
    //     Err(e) => {
    //         if e.to_string().contains("already in use") {
    //             info!("User account already initialized");
    //         } else {
    //             info!("Failed to initialize user: {}", e);
    //         }
    //     }
    // }

    // Wait a bit for confirmation
    // tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    info!("\nStep 2: Add Collateral");
    let collateral_amount = 1_000_000 * 1_000_000; // 1,000,000 USDT
    let sig = manager
        .add_collateral(&payer.pubkey(), collateral_amount)
        .await?;
    info!("Collateral added: {}", sig);

    // 2. Open position
    info!("Step 2: Opening BTC Long position...");
    let (position, sig) = manager
        .open_position(
            payer.pubkey(),
            "BTC-USD".to_string(),
            Side::Long,
            dec!(1),
            10,
            dec!(50000),
            dec!(0.0025),
        )
        .await?;

    info!("   Opened: {} (tx: {})", position.position_account, sig);
    info!(
        "   Margin: ${}, Liq Price: ${}",
        position.margin, position.liquidation_price
    );

    // 3. Modify position
    info!("Step 3: Increasing position size...");
    let sig = manager
        .modify_position(
            position.position_account,
            Some(dec!(0.1)), // Increase to 0.1 BTC
            None,
        )
        .await?;
    info!("   Modified: {}", sig);

    // 4. Close position
    info!("Step 5: Closing position...");
    let (pnl, sig) = manager
        .close_position(position.position_account, dec!(99000))
        .await?;
    info!("   Closed: {}", sig);
    info!("   Final PnL: ${}", pnl);

    // 5. Final statistics
    info!("Step 6: Final statistics:");
    let stats = manager.get_statistics().await?;
    info!("   Total positions: {}", stats.total_positions);
    info!("   Open positions: {}", stats.open_positions);
    info!("   Closed positions: {}", stats.closed_positions);
    info!("   Total realized PnL: ${}", stats.total_realized_pnl);

    info!("");
    info!("Full lifecycle test passed!");

    Ok(())
}
