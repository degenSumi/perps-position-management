use anyhow::Result;
use perpetual_backend::{create_router};
use perpetual_backend::api::handlers::AppState;
use perpetual_backend::infrastructure::{OracleClient, SolanaClient};
use perpetual_backend::services::{MonitorConfig, PositionMonitor, PositionManager};
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main(flavor = "multi_thread", worker_threads = 8)]
async fn main() -> Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();
    
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("Starting Perpetual Futures Backend");

    // Load configuration from environment
    let program_id = std::env::var("PROGRAM_ID")
        .expect("PROGRAM_ID not set in .env")
        .parse()
        .expect("Invalid PROGRAM_ID format");
    
    let private_key = std::env::var("SOLANA_PRIVATE_KEY")
        .expect("SOLANA_PRIVATE_KEY not set in .env");
    
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .expect("Invalid PORT");

    info!("Configuration:");
    info!("  Program ID: {}", program_id);
    info!("  RPC URL: {}", rpc_url);
    info!("  Redis URL: {}", redis_url);
    info!("  Port: {}", port);

    // Initialize Solana payer keypair from private key
    let payer: Arc<Keypair> = Arc::new(Keypair::from_base58_string(&private_key));
    info!("  Payer: {}", payer.pubkey());
    
    let solana_client = Arc::new(SolanaClient::new(
        program_id,
        payer,
        rpc_url,
    ));
    info!("solana client initialized");

    // Initialize Oracle client
    let oracle_client = Arc::new(RwLock::new(
        OracleClient::new_hermes().with_mainnet_defaults(),
    ));
    info!("Oracle client initialized");

    // Initialize Position Monitor
    let monitor: Arc<PositionMonitor> = Arc::new(
        PositionMonitor::new(
            Arc::clone(&solana_client),
            oracle_client,
            MonitorConfig::default(),
            redis_url,
        )?
    );
    info!("Position monitor created");

    // Start monitoring in background
    let monitor_clone = Arc::clone(&monitor);
    tokio::spawn(async move {
        if let Err(e) = monitor_clone.start().await {
            tracing::error!("Failed to start monitor: {}", e);
        }
    });
    info!("Position monitor started (background tasks)");

    // Initialize Position Manager with monitor reference
    let position_manager = Arc::new(PositionManager::new(
        Arc::clone(&solana_client),
        Arc::clone(&monitor),  // Shared state
    ));

    // Create app state
    let state = AppState {
        monitor: Arc::clone(&monitor),
        position_manager,
    };

    // Create router with middleware
    let app = create_router(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    // Start HTTP server
    let addr = format!("0.0.0.0:{}", port);
    info!("HTTP server starting on {}", addr);
    info!("WebSocket available at ws://{}/ws", addr);
    info!("");
    info!("Ready to accept connections!");
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
