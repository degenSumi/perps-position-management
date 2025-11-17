use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};
use std::{collections::HashSet, sync::Arc};

use crate::api::handlers::AppState;
use crate::api::dto::{PriceDto, PositionUpdateDto, LiquidationAlertDto};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    SubscribeSymbol { symbol: String },
    UnsubscribeSymbol { symbol: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsMessage {
    Connected { message: String },
    PriceUpdate(PriceDto),
    PositionUpdate(PositionUpdateDto),
    LiquidationAlert(LiquidationAlertDto),
    Error { message: String },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| websocket_handler(socket, state))
}

async fn websocket_handler(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();

    // Wrap sender in Arc<Mutex> so it can be shared safely
    let sender = Arc::new(Mutex::new(sender));

    // Subscribe to the broadcast channels for price, position, and liquidation
    let mut price_rx = state.monitor.subscribe_prices();
    let mut position_rx = state.monitor.subscribe_positions();
    let mut liquidation_rx = state.monitor.subscribe_liquidation_alerts();

    info!("WebSocket client connected");

    // Track subscribed symbols; empty means subscribe to all
    let subscribed_symbols: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(HashSet::new()));

    // Send welcome message
    {
        let mut sender_lock = sender.lock().await;
        if let Err(e) = sender_lock.send(Message::Text(
            serde_json::to_string(&WsMessage::Connected {
                message: "Connected to Perpetual Futures Backend".to_string(),
            })
            .unwrap(),
        ))
        .await
        {
            error!("Failed to send welcome message: {}", e);
            return;
        }
    }

    // Task to handle incoming client messages
    let recv_sender = Arc::clone(&sender);
    let recv_subscriptions = Arc::clone(&subscribed_symbols);
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                match serde_json::from_str::<ClientCommand>(&text) {
                    Ok(cmd) => {
                        match cmd {
                            ClientCommand::SubscribeSymbol { symbol } => {
                                info!("Client subscribed to symbol: {}", symbol);
                                recv_subscriptions.write().await.insert(symbol);
                            }
                            ClientCommand::UnsubscribeSymbol { symbol } => {
                                info!("Client unsubscribed from symbol: {}", symbol);
                                recv_subscriptions.write().await.remove(&symbol);
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = WsMessage::Error {
                            message: format!("Invalid command: {}", e),
                        };
                        let mut sender_lock = recv_sender.lock().await;
                        if let Err(e) =
                            sender_lock.send(Message::Text(serde_json::to_string(&error_msg).unwrap())).await
                        {
                            error!("Failed to send error message: {}", e);
                            break;
                        }
                    }
                }
            } else if let Message::Close(_) = msg {
                info!("Client disconnected");
                break;
            }
        }
    });

    // Task to send updates to client filtered by subscribed symbols
    let send_sender = Arc::clone(&sender);
    let send_subscriptions = Arc::clone(&subscribed_symbols);
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                Ok(price_update) = price_rx.recv() => {
                    let subs = send_subscriptions.read().await;
                    if subs.is_empty() || subs.contains(&price_update.symbol) {
                        let dto = PriceDto {
                            symbol: price_update.symbol.clone(),
                            price: price_update.price,
                            timestamp: price_update.timestamp,
                        };
                        let msg = WsMessage::PriceUpdate(dto);
                        let mut sender_lock = send_sender.lock().await;
                        if let Err(e) = sender_lock.send(Message::Text(serde_json::to_string(&msg).unwrap())).await {
                            warn!("Failed to send price update: {}", e);
                            break;
                        }
                    }
                },
                Ok(position_update) = position_rx.recv() => {
                    let subs = send_subscriptions.read().await;
                    if subs.is_empty() || subs.contains(&position_update.symbol) {
                        let dto = PositionUpdateDto {
                            position_account: position_update.position_account.to_string(),
                            symbol: position_update.symbol.clone(),
                            side: position_update.side,
                            size: position_update.size,
                            entry_price: position_update.entry_price,
                            mark_price: position_update.mark_price,
                            unrealized_pnl: position_update.unrealized_pnl,
                            margin_ratio: position_update.margin_ratio,
                            timestamp: position_update.timestamp,
                        };
                        let msg = WsMessage::PositionUpdate(dto);
                        let mut sender_lock = send_sender.lock().await;
                        if let Err(e) = sender_lock.send(Message::Text(serde_json::to_string(&msg).unwrap())).await {
                            warn!("Failed to send position update: {}", e);
                            break;
                        }
                    }
                },
                Ok(alert) = liquidation_rx.recv() => {
                    let subs = send_subscriptions.read().await;
                    if subs.is_empty() || subs.contains(&alert.symbol) {
                        let dto = LiquidationAlertDto {
                            risk_type: alert.risk_type,
                            position_account: alert.position_account,
                            symbol: alert.symbol.clone(),
                            side: alert.side,
                            liquidation_price: alert.liquidation_price,
                            current_price: alert.current_price,
                        };
                        let msg = WsMessage::LiquidationAlert(dto);
                        let mut sender_lock = send_sender.lock().await;
                        if let Err(e) = sender_lock.send(Message::Text(serde_json::to_string(&msg).unwrap())).await {
                            warn!("Failed to send liquidation alert: {}", e);
                            break;
                        }
                    }
                },
                else => {
                    break;
                }
            }
        }
    });

    // Pin the tasks
    tokio::pin!(recv_task);
    tokio::pin!(send_task);

    tokio::select! {
        _ = &mut recv_task => send_task.abort(),
        _ = &mut send_task => recv_task.abort(),
    }

    info!("WebSocket connection closed");
}
