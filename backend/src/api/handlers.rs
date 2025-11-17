use axum::{
    extract::{Path, Query, State},
    Json,
};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

use crate::api::{dto::*, errors::ApiError};
use crate::services::{PositionManager, PositionMonitor};
use rust_decimal::Decimal;
use std::sync::Arc;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub monitor: Arc<PositionMonitor>,
    pub position_manager: Arc<PositionManager>,
}

/// GET /health - Health check
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "perpetual-backend"
    }))
}

/// GET /positions - List all positions
pub async fn list_positions(
    State(state): State<AppState>,
    Query(query): Query<ListPositionsQuery>,
) -> Result<Json<Vec<PositionDto>>, ApiError> {
    let mut positions = if let Some(symbol) = query.symbol {
        state.monitor.get_positions_by_asset(&symbol).await
    } else {
        state.monitor.get_all_positions().await
    };

    // Filter by owner if provided
    if let Some(owner_str) = query.owner {
        if let Ok(owner_pubkey) = owner_str.parse::<solana_sdk::pubkey::Pubkey>() {
            positions.retain(|p| p.owner == owner_pubkey);
        }
    }

    // Filter by status if provided
    if let Some(status) = query.status {
        positions.retain(|p| p.status == status);
    }

    // Apply pagination
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100).min(1000); // Max 1000

    let paginated: Vec<PositionDto> = positions
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(PositionDto::from)
        .collect();

    Ok(Json(paginated))
}

/// GET /positions/:id - Get specific position
pub async fn get_position(
    State(state): State<AppState>,
    Path(id): Path<Pubkey>,
) -> Result<Json<PositionDto>, ApiError> {
    let position = state
        .monitor
        .get_position(id)
        .await
        .ok_or_else(|| ApiError::NotFound(format!("Position {} not found", id)))?;

    Ok(Json(PositionDto::from(position)))
}

/// GET /positions/by-asset/:symbol - Get positions for specific asset
pub async fn get_positions_by_asset(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<Vec<PositionDto>>, ApiError> {
    let positions = state.monitor.get_positions_by_asset(&symbol).await;

    let dtos: Vec<PositionDto> = positions
        .into_iter()
        .map(PositionDto::from)
        .collect();

    Ok(Json(dtos))
}

/// GET /statistics - Get monitoring statistics
pub async fn get_statistics(
    State(state): State<AppState>,
) -> Result<Json<StatisticsDto>, ApiError> {
    let stats = state.monitor.get_statistics().await;

    let dto = StatisticsDto {
        total_positions: stats.total_positions,
        open_positions: stats.open_positions,
        assets_monitored: stats.assets_monitored,
        total_unrealized_pnl: stats.total_unrealized_pnl,
    };

    Ok(Json(dto))
}

/// GET /prices - Get current prices for all monitored assets
pub async fn get_prices(
    State(state): State<AppState>,
) -> Result<Json<Vec<PriceDto>>, ApiError> {
    let symbols = state.monitor.get_monitored_symbols().await;

    let mut prices = Vec::new();
    for symbol in symbols {
        if let Some(price) = state.monitor.get_cached_price(&symbol).await {
            prices.push(PriceDto {
                symbol,
                price,
                timestamp: chrono::Utc::now(),
            });
        }
    }

    Ok(Json(prices))
}

/// GET /prices/:symbol - Get current price for specific asset
pub async fn get_price(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<PriceDto>, ApiError> {
    let price = state
        .monitor
        .get_cached_price(&symbol)
        .await
        .ok_or_else(|| ApiError::NotFound(format!("Price for {} not found", symbol)))?;

    Ok(Json(PriceDto {
        symbol,
        price,
        timestamp: chrono::Utc::now(),
    }))
}


/// POST /positions/open - Open new position
pub async fn open_position(
    State(state): State<AppState>,
    Json(payload): Json<OpenPositionRequest>,
) -> Result<Json<OpenPositionResponse>, ApiError> {
    let owner = Pubkey::from_str(&payload.owner)
        .map_err(|e| ApiError::BadRequest(format!("Invalid owner pubkey: {}", e)))?;

    let (position, signature) = state
        .position_manager
        .open_position(
            owner,
            payload.symbol,
            payload.side,
            payload.size,
            payload.leverage,
            payload.entry_price,
            Decimal::new(25, 3), // Default 2.5%
        )
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to open position: {}", e)))?;

    Ok(Json(OpenPositionResponse {
        position: PositionDto::from(position),
        signature: signature.to_string(),
    }))
}

/// GET /users/:id/positions - Get user's positions
pub async fn get_user_positions(
    State(state): State<AppState>,
    Path(owner): Path<String>,
) -> Result<Json<Vec<PositionDto>>, ApiError> {
    let owner = Pubkey::from_str(&owner)
        .map_err(|e| ApiError::BadRequest(format!("Invalid owner pubkey: {}", e)))?;

    let positions = state
        .position_manager
        .get_user_positions(&owner)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to fetch positions: {}", e)))?;

    let dtos: Vec<PositionDto> = positions.into_iter().map(PositionDto::from).collect();

    Ok(Json(dtos))
}

/// POST /users/initialize - Initialize user account
pub async fn initialize_user(
    State(state): State<AppState>,
    Json(payload): Json<InitializeUserRequest>,
) -> Result<Json<InitializeUserResponse>, ApiError> {
    let owner = Pubkey::from_str(&payload.owner)
        .map_err(|e| ApiError::BadRequest(format!("Invalid owner pubkey: {}", e)))?;

    let signature = state
        .position_manager
        .initialize_user(&owner)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to initialize user: {}", e)))?;

    Ok(Json(InitializeUserResponse {
        signature: signature.to_string(),
        message: "User account initialized".to_string(),
    }))
}


/// POST /users/:id/collateral - Add collateral
pub async fn add_collateral(
    State(state): State<AppState>,
    Path(owner): Path<String>,
    Json(payload): Json<AddCollateralRequest>,
) -> Result<Json<AddCollateralResponse>, ApiError> {
    let owner = Pubkey::from_str(&owner)
        .map_err(|e| ApiError::BadRequest(format!("Invalid owner pubkey: {}", e)))?;

    let signature = state
        .position_manager
        .add_collateral(&owner, payload.amount)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to add collateral: {}", e)))?;

    Ok(Json(AddCollateralResponse {
        signature: signature.to_string(),
        message: format!("Added {} collateral", payload.amount),
    }))
}


/// GET /users/:id/account - Get user account details
pub async fn get_user_account(
    State(state): State<AppState>,
    Path(owner): Path<String>,
) -> Result<Json<UserAccountDto>, ApiError> {
    let owner = Pubkey::from_str(&owner)
        .map_err(|e| ApiError::BadRequest(format!("Invalid owner pubkey: {}", e)))?;

    let user_account = state
        .position_manager
        .get_user_account(&owner)
        .await
        .map_err(|e| ApiError::NotFound(format!("User account not found: {}", e)))?;

    Ok(Json(UserAccountDto {
        owner: user_account.owner.to_string(),
        total_collateral: user_account.total_collateral,
        locked_collateral: user_account.locked_collateral,
        available_collateral: user_account.total_collateral - user_account.locked_collateral,
        total_pnl: user_account.total_pnl,
        position_count: user_account.position_count,
        position_count_total: user_account.position_count_total,
    }))
}


/// GET /positions/:id - Get position details
pub async fn get_position_details(
    State(state): State<AppState>,
    Path(position_account): Path<String>,  // Now expects pubkey string
) -> Result<Json<PositionDto>, ApiError> {
    let position_account = Pubkey::from_str(&position_account)
        .map_err(|e| ApiError::BadRequest(format!("Invalid position account: {}", e)))?;

    let position = state
        .position_manager
        .get_position(position_account)  // Uses Pubkey
        .await
        .map_err(|e| ApiError::NotFound(format!("Position not found: {}", e)))?;

    Ok(Json(PositionDto::from(position)))
}

/// PUT /positions/:id/modify - Modify position
pub async fn modify_position(
    State(state): State<AppState>,
    Path(position_account): Path<String>,  // Now expects pubkey string
    Json(payload): Json<ModifyPositionRequest>,
) -> Result<Json<ModifyPositionResponse>, ApiError> {
    let position_account = Pubkey::from_str(&position_account)
        .map_err(|e| ApiError::BadRequest(format!("Invalid position account: {}", e)))?;

    let signature = state
        .position_manager
        .modify_position(position_account, payload.new_size, payload.margin_delta)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to modify position: {}", e)))?;

    Ok(Json(ModifyPositionResponse {
        signature: signature.to_string(),
        message: "Position modified successfully".to_string(),
    }))
}

/// DELETE /positions/:id/close - Close position
pub async fn close_position(
    State(state): State<AppState>,
    Path(position_account): Path<String>,  // Now expects pubkey string
    Json(payload): Json<ClosePositionRequest>,
) -> Result<Json<ClosePositionResponse>, ApiError> {
    let position_account = Pubkey::from_str(&position_account)
        .map_err(|e| ApiError::BadRequest(format!("Invalid position account: {}", e)))?;

    let (pnl, signature) = state
        .position_manager
        .close_position(position_account, payload.final_price)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to close position: {}", e)))?;

    Ok(Json(ClosePositionResponse {
        pnl,
        signature: signature.to_string(),
        message: "Position closed successfully".to_string(),
    }))
}