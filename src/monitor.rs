use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::Serialize;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::market::{ActiveMarket, MarketCategory};
use crate::polymarket::*;

pub type MarketState = Arc<RwLock<HashMap<String, ActiveMarket>>>;

#[derive(Debug, Clone, Serialize)]
pub enum MarketChangeEvent {
    NewMarket(ActiveMarket),
    MarketClosed(ActiveMarket),
}

pub struct Monitor {
    ws: WsMarketClient,
    rest: RestMarketClient,
    poll_interval: std::time::Duration,
    markets: MarketState,
    tx: tokio::sync::broadcast::Sender<MarketChangeEvent>,
}

pub enum MonitorEvent {
    NewMarket(ActiveMarket),
    Snapshot(Vec<ActiveMarket>, DateTime<Utc>),
}

impl Monitor {
    pub fn new(
        ws: WsMarketClient,
        rest: RestMarketClient,
        poll_interval: std::time::Duration,
        markets: MarketState,
        tx: tokio::sync::broadcast::Sender<MarketChangeEvent>,
    ) -> Self {
        Self {
            ws,
            rest,
            poll_interval,
            markets,
            tx,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let ws_tx = tx.clone();
        let ws = self.ws;
        tokio::spawn(async move {
            let stream = ws.subscribe();
            tokio::pin!(stream);
            while let Some(result) = stream.next().await {
                match result {
                    Ok(event) => {
                        if let Err(e) = ws_tx.send(MonitorEvent::NewMarket(event.into())) {
                            error!("Failed to send new market event from WebSocket: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("WebSocket error: {:?}", e);
                    }
                }
            }
        });

        let rest_tx = tx.clone();
        let rest = self.rest;
        let poll_interval = self.poll_interval;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let start = Utc::now();
                let stream = rest.stream_all_events(Some(true));
                tokio::pin!(stream);
                let mut active_markets = Vec::new();
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(events) => {
                            for event in events {
                                if let Some(markets) = event.markets {
                                    let active = markets
                                        .into_iter()
                                        .filter(|m| {
                                            m.active.unwrap_or(false) && !m.closed.unwrap_or(true)
                                        })
                                        .collect::<Vec<_>>();
                                    active_markets.extend(active.into_iter().filter_map(|m| {
                                        match ActiveMarket::try_from(&m) {
                                            Ok(market) => {
                                                if let Some(tags) = event.tags.clone() {
                                                    Some(market.with_tags(tags))
                                                } else {
                                                    Some(market)
                                                }
                                            },
                                            Err(e) => {
                                                error!("Failed to convert Market to ActiveMarket: {:?}", e);
                                                None
                                            }
                                        }
                                    }));
                                }
                            }
                        }
                        Err(e) => {
                            error!("REST API error: {:?}", e);
                        }
                    }
                }
                if !active_markets.is_empty()
                    && let Err(e) = rest_tx.send(MonitorEvent::Snapshot(active_markets, start))
                {
                    error!("Failed to send snapshot event from REST API: {}", e);
                }
            }
        });

        drop(tx);

        while let Some(event) = rx.recv().await {
            match event {
                MonitorEvent::NewMarket(market) => {
                    info!("New market: {:?}", market);
                    if let Err(e) = self.tx.send(MarketChangeEvent::NewMarket(market.clone())) {
                        debug!("No active subscribers for new market event: {}", e);
                    }
                    self.markets
                        .write()
                        .await
                        .insert(market.condition_id.clone(), market);
                }
                MonitorEvent::Snapshot(snapshot_markets, scrape_time) => {
                    info!(
                        "Snapshot of active markets at {}: {} markets",
                        scrape_time,
                        snapshot_markets.len()
                    );
                    let snapshot_ids: std::collections::HashSet<String> = snapshot_markets
                        .iter()
                        .map(|m| m.condition_id.clone())
                        .collect();
                    let missing_markets: Vec<_> = self
                        .markets
                        .read()
                        .await
                        .values()
                        .filter(|m| !snapshot_ids.contains(&m.condition_id))
                        .cloned()
                        .collect();
                    *self.markets.write().await = snapshot_markets
                        .into_iter()
                        .map(|m| (m.condition_id.clone(), m))
                        .collect();

                    for market in missing_markets {
                        if market.first_seen < scrape_time {
                            info!("Market closed: {:?}", market);
                            if let Err(e) = self.tx.send(MarketChangeEvent::MarketClosed(market)) {
                                debug!("No active subscribers for market closed event: {}", e);
                            }
                        } else {
                            // to cover race condition between ws and rest
                            self.markets
                                .write()
                                .await
                                .insert(market.condition_id.clone(), market);
                        }
                    }
                }
            }

            info!("Total active markets: {}", self.markets.read().await.len());

            let mut category_counts: HashMap<String, usize> = HashMap::new();
            for market in self.markets.read().await.values() {
                let category = match &market.category {
                    MarketCategory::Sports => "Sports".to_string(),
                    MarketCategory::UpNDown => "UpNDown".to_string(),
                    MarketCategory::Other(_) => "Other".to_string(),
                    MarketCategory::None => "None".to_string(),
                    MarketCategory::Weather => "Weather".to_string(),
                };
                *category_counts.entry(category).or_insert(0) += 1;
            }

            info!("Market counts by category:");
            for (category, count) in category_counts {
                info!("  {}: {}", category, count);
            }
        }

        Ok(())
    }
}
