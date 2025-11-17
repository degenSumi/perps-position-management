use axum::{
    routing::{get, post, put, delete},
    Router,
};

use super::handlers::*;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health_check))
        
        // User routes
        .route("/users/initialize", post(initialize_user))
        .route("/users/:id/account", get(get_user_account))
        .route("/users/:id/collateral", post(add_collateral))
        .route("/users/:id/positions", get(get_user_positions))
        
        // Position routes
        .route("/positions/open", post(open_position))
        .route("/positions/:id", get(get_position_details))
        .route("/positions/:id/modify", put(modify_position))
        .route("/positions/:id/close", delete(close_position))
        
        // Monitoring routes
        .route("/positions", get(list_positions))
        .route("/positions/by-asset/:symbol", get(get_positions_by_asset))
        .route("/statistics", get(get_statistics))
        .route("/prices", get(get_prices))
        .route("/prices/:symbol", get(get_price))
        
        // WebSocket route
        .route("/ws", get(super::websocket::ws_handler))
        
        .with_state(state)
}
