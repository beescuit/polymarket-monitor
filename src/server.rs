use axum::{
    Json, Router,
    extract::{
        Query, State,
        ws::{Utf8Bytes, WebSocket},
    },
    routing::get,
};
use serde::Deserialize;
use tokio::sync::broadcast;
use tracing::{debug, error, info};

use crate::market::{ActiveMarket, MarketCategory};
use crate::monitor::{MarketChangeEvent, MarketState};

#[derive(Debug, Clone)]
pub struct ServerState {
    markets: MarketState,
    tx: broadcast::Sender<MarketChangeEvent>,
}

pub struct HttpServer {
    port: u16,
    state: ServerState,
}

impl HttpServer {
    pub fn new(port: u16, markets: MarketState, tx: broadcast::Sender<MarketChangeEvent>) -> Self {
        Self {
            port,
            state: ServerState { markets, tx },
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/markets", get(Self::list_markets))
            .route("/ws", get(Self::ws_handler))
            .with_state(self.state);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;

        info!("HTTP server listening on port {}", self.port);

        axum::serve(listener, app).await?;

        Ok(())
    }

    pub async fn list_markets(
        Query(params): Query<MarketQuery>,
        State(state): State<ServerState>,
    ) -> Json<Vec<ActiveMarket>> {
        let mut markets = state
            .markets
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();

        if let Some(category) = params.category {
            let target_category = match category.to_lowercase().as_str() {
                "sports" => MarketCategory::Sports,
                "updown" => MarketCategory::UpNDown,
                "weather" => MarketCategory::Weather,
                other => MarketCategory::Other(other.to_string()),
            };

            markets.retain(|m| m.category == target_category);
        }

        if let Some(min_volume) = params.min_volume {
            markets.retain(|m| m.volume_24h >= min_volume);
        }

        if params.volume_sort != Some(false) {
            markets.sort_by(|a, b| {
                b.volume_24h
                    .partial_cmp(&a.volume_24h)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Json(markets)
    }

    async fn ws_handler(
        ws: axum::extract::WebSocketUpgrade,
        State(state): State<ServerState>,
    ) -> impl axum::response::IntoResponse {
        ws.on_upgrade(move |socket| self::handle_socket(socket, state.tx.clone()))
    }
}

async fn handle_socket(mut socket: WebSocket, tx: broadcast::Sender<MarketChangeEvent>) {
    let mut rx = tx.subscribe();

    info!("New WebSocket connection established");

    loop {
        match rx.recv().await {
            Ok(event) => {
                socket
                    .send(axum::extract::ws::Message::Text(Utf8Bytes::from(
                        serde_json::to_string(&event).unwrap_or_else(|_| {
                            error!("Failed to serialize MarketChangeEvent: {:?}", event);
                            "{}".to_string()
                        }),
                    )))
                    .await
                    .unwrap_or_else(|e| {
                        debug!("Failed to send WebSocket message: {}", e);
                    });
            }
            Err(e) => {
                error!("WebSocket broadcast error: {:?}", e);
                break;
            }
        }
    }
}

#[derive(Deserialize)]
pub struct MarketQuery {
    pub category: Option<String>,
    pub volume_sort: Option<bool>,
    pub min_volume: Option<f64>,
}
