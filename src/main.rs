mod market;
mod monitor;
mod polymarket;
mod server;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::subscriber;
use tracing_subscriber::FmtSubscriber;

use crate::{
    monitor::Monitor,
    polymarket::{RestMarketClient, WsMarketClient},
};

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .expect("PORT environment variable not set.")
        .parse()
        .expect("Failed to parse PORT.");
    let clob_url = std::env::var("CLOB_URL")
        .unwrap_or_else(|_| "wss://ws-subscriptions-clob.polymarket.com/ws/market".into());
    let gamma_url =
        std::env::var("GAMMA_URL").unwrap_or_else(|_| "https://gamma-api.polymarket.com".into());
    let update_interval: u64 = std::env::var("UPDATE_INTERVAL")
        .unwrap_or_else(|_| "5".into())
        .parse()
        .expect("Failed to parse UPDATE_INTERVAL.");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .finish();

    subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let markets: crate::monitor::MarketState = Arc::new(RwLock::new(HashMap::new()));

    let (tx, _) = tokio::sync::broadcast::channel(100);

    let monitor = Monitor::new(
        WsMarketClient::new(clob_url),
        RestMarketClient::new(gamma_url),
        std::time::Duration::from_mins(update_interval),
        markets.clone(),
        tx.clone(),
    );

    let server = server::HttpServer::new(port, markets.clone(), tx.clone());

    tokio::select! {
        res = monitor.run() => res.expect("Monitor failed"),
        res = server.run() => res.expect("Server failed"),
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl+C, shutting down");
        }
    }
}
