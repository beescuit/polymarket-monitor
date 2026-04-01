use std::collections::HashMap;

use async_stream::stream;
use chrono::{DateTime, SecondsFormat, Utc};
use futures::StreamExt;
use futures::stream::{FuturesUnordered, Stream};
use tracing::debug;

use crate::polymarket::rest_types::{Event, Market};

pub struct RestMarketClient {
    client: reqwest::Client,
    base_url: String,
}

// Some of these methods/options are not used, but I'm keeping them in case I decide to use them later
#[allow(dead_code)]
impl RestMarketClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    pub async fn get_events(&self, options: EventsRequest) -> Result<Vec<Event>, reqwest::Error> {
        let mut params = HashMap::new();

        if let Some(limit) = options.limit {
            params.insert("limit", limit.to_string());
        }
        if let Some(offset) = options.offset {
            params.insert("offset", offset.to_string());
        }
        if let Some(order) = options.order {
            params.insert("order", order);
        }
        if let Some(ascending) = options.ascending {
            params.insert("ascending", ascending.to_string());
        }
        if let Some(active) = options.active {
            params.insert("active", active.to_string());
        }
        if let Some(closed) = options.closed {
            params.insert("closed", closed.to_string());
        }
        if let Some(start_date_min) = options.start_date_min {
            params.insert("start_date_min", start_date_min);
        }

        params.insert("cb", Utc::now().timestamp_millis().to_string());

        let events = self
            .get("events", params)
            .await?
            .json::<Vec<Event>>()
            .await?;

        Ok(events)
    }

    pub async fn get_markets(
        &self,
        options: MarketsRequest,
    ) -> Result<Vec<Market>, reqwest::Error> {
        let mut params = HashMap::new();

        if let Some(limit) = options.limit {
            params.insert("limit", limit.to_string());
        }
        if let Some(offset) = options.offset {
            params.insert("offset", offset.to_string());
        }
        if let Some(order) = options.order {
            params.insert("order", order);
        }
        if let Some(ascending) = options.ascending {
            params.insert("ascending", ascending.to_string());
        }
        if let Some(active) = options.active {
            params.insert("active", active.to_string());
        }
        if let Some(closed) = options.closed {
            params.insert("closed", closed.to_string());
        }
        if let Some(start_date_min) = options.start_date_min {
            params.insert("start_date_min", start_date_min);
        }

        params.insert("cb", Utc::now().timestamp_millis().to_string());

        let markets = self
            .get("markets", params)
            .await?
            .json::<Vec<Market>>()
            .await?;

        Ok(markets)
    }

    pub async fn get_all_events(
        &self,
        only_active: Option<bool>,
    ) -> Result<Vec<Event>, reqwest::Error> {
        let mut all_events = Vec::new();
        let mut offset = 0;
        let limit = 500;

        loop {
            let mut request = EventsRequest::default().with_limit(500).with_offset(offset);

            if Some(true) == only_active {
                request = request.only_active();
            }

            let events = self.get_events(request).await?;

            if events.is_empty() {
                break;
            }

            debug!("Received {} events (offset: {})", events.len(), offset);

            all_events.extend(events);
            offset += limit;
        }

        Ok(all_events)
    }

    pub fn stream_all_events(
        &self,
        only_active: Option<bool>,
    ) -> impl Stream<Item = Result<Vec<Event>, reqwest::Error>> {
        let concurrency = 10;
        let page_size = 500;
        debug!(
            "Starting event stream with concurrency: {}, page_size: {}, only_active: {:?}",
            concurrency, page_size, only_active
        );
        stream! {
            let mut pending = FuturesUnordered::new();

            for i in 0..concurrency {
                let mut req = EventsRequest::default().with_limit(page_size).with_offset(i * page_size);
                if Some(true) == only_active {
                    req = req.only_active();
                }
                pending.push(self.get_events(req));
            }

            let mut next_offset = concurrency * page_size;

            while let Some(result) = pending.next().await {
                match result {
                    Ok(events) if !events.is_empty() => {
                        let mut req = EventsRequest::default().with_limit(page_size).with_offset(next_offset);
                        if Some(true) == only_active {
                            req = req.only_active();
                        }
                        pending.push(self.get_events(req));
                        next_offset += page_size;
                        yield Ok(events);
                    }
                    Ok(_) => {}
                    Err(e) => yield Err(e)
                }
            }
        }
    }

    async fn get(
        &self,
        endpoint: &str,
        params: HashMap<&str, String>,
    ) -> reqwest::Result<reqwest::Response> {
        let base_url = format!("{}/{}", self.base_url, endpoint);
        let url = reqwest::Url::parse_with_params(&base_url, &params)
            .expect("Failed to parse URL with parameters");

        self.client.get(url).send().await
    }
}

#[derive(Default, Debug)]
pub struct EventsRequest {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub order: Option<String>,
    pub ascending: Option<bool>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub start_date_min: Option<String>,
}

#[allow(dead_code)]
impl EventsRequest {
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn with_order(mut self, order: &str) -> Self {
        self.order = Some(order.to_string());
        self
    }

    pub fn with_start_date_min(mut self, start_date_min: DateTime<Utc>) -> Self {
        self.start_date_min = Some(start_date_min.to_rfc3339_opts(SecondsFormat::Micros, true));
        self
    }

    pub fn with_closed(mut self, closed: bool) -> Self {
        self.closed = Some(closed);
        self
    }

    pub fn with_active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    pub fn only_active(mut self) -> Self {
        self.active = Some(true);
        self.closed = Some(false);
        self
    }
}

#[derive(Default, Debug)]
pub struct MarketsRequest {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub order: Option<String>,
    pub ascending: Option<bool>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub start_date_min: Option<String>,
}

#[allow(dead_code)]
impl MarketsRequest {
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn with_order(mut self, order: &str) -> Self {
        self.order = Some(order.to_string());
        self
    }

    pub fn with_start_date_min(mut self, start_date_min: DateTime<Utc>) -> Self {
        self.start_date_min = Some(start_date_min.to_rfc3339_opts(SecondsFormat::Micros, true));
        self
    }

    pub fn with_closed(mut self, closed: bool) -> Self {
        self.closed = Some(closed);
        self
    }

    pub fn with_active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    pub fn only_active(mut self) -> Self {
        self.active = Some(true);
        self.closed = Some(false);
        self
    }
}
