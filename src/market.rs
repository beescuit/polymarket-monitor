use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::polymarket::rest_types::{Market, Tag};
use crate::polymarket::ws::NewMarketMessage;

#[derive(Debug, Clone, Serialize)]
pub struct ActiveMarket {
    pub id: String,
    pub condition_id: String,
    pub slug: String,
    pub question: String,
    pub token_ids: Vec<String>,
    pub outcomes: Vec<String>,
    pub volume_24h: f64,
    pub category: MarketCategory,
    pub first_seen: DateTime<Utc>,
    pub source: MarketSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarketCategory {
    Sports,
    UpNDown,
    Weather,
    Other(String),
    None,
}

impl Serialize for MarketCategory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            MarketCategory::Sports => serializer.serialize_str("sports"),
            MarketCategory::UpNDown => serializer.serialize_str("updown"),
            MarketCategory::Weather => serializer.serialize_str("weather"),
            MarketCategory::Other(s) => serializer.serialize_str(s),
            MarketCategory::None => serializer.serialize_str("none"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum MarketSource {
    WebSocket,
    Rest,
}

impl From<NewMarketMessage> for ActiveMarket {
    fn from(msg: NewMarketMessage) -> Self {
        ActiveMarket {
            id: msg.id,
            condition_id: msg.market,
            slug: msg.slug,
            question: msg.question,
            token_ids: msg.assets_ids,
            outcomes: msg.outcomes,
            volume_24h: 0.0,
            category: MarketCategory::None,
            first_seen: Utc::now(),
            source: MarketSource::WebSocket,
        }
    }
}

impl TryFrom<&Market> for ActiveMarket {
    type Error = anyhow::Error;

    fn try_from(m: &Market) -> anyhow::Result<Self> {
        Some(Self {
            id: m
                .id
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Failed to convert Market id to ActiveMarket"))?,
            condition_id: m.condition_id.clone().ok_or_else(|| {
                anyhow::anyhow!("Failed to convert Market condition_id to ActiveMarket")
            })?,
            slug: m
                .slug
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Failed to convert Market slug to ActiveMarket"))?,
            question: m.question.clone().ok_or_else(|| {
                anyhow::anyhow!("Failed to convert Market question to ActiveMarket")
            })?,
            token_ids: serde_json::from_str(m.clob_token_ids.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Failed to convert Market assets_ids to ActiveMarket")
            })?)?,
            outcomes: serde_json::from_str(m.outcomes.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Failed to convert Market outcomes to ActiveMarket")
            })?)?,
            volume_24h: m.volume24hr.unwrap_or(0.0),
            category: m.categorize(),
            first_seen: Utc::now(),
            source: MarketSource::Rest,
        })
        .ok_or_else(|| anyhow::anyhow!("Failed to convert Market to ActiveMarket"))
    }
}

trait Categorizable {
    fn categorize(&self) -> MarketCategory;
}

impl Categorizable for Market {
    fn categorize(&self) -> MarketCategory {
        if let Some(slug) = &self.slug
            && slug.contains("updown")
        {
            return MarketCategory::UpNDown;
        }
        if self.sports_market_type.is_some() {
            return MarketCategory::Sports;
        }
        if let Some(category) = &self.category {
            if category.to_lowercase().contains("sports") {
                return MarketCategory::Sports;
            } else {
                return MarketCategory::Other(category.clone());
            }
        }
        MarketCategory::None
    }
}

impl ActiveMarket {
    pub fn with_tags(mut self, tags: Vec<Tag>) -> Self {
        if let MarketCategory::None = self.category {
            // debug!("Categorizing market {} with tags: {:?}", self.id, tags);
            let slugs = tags
                .iter()
                .map(|t| t.slug.clone().unwrap_or("".into()))
                .collect::<Vec<_>>();
            if slugs.iter().any(|s| s.to_lowercase().contains("weather")) {
                self.category = MarketCategory::Weather;
            } else {
                self.category = MarketCategory::Other(slugs.first().cloned().unwrap_or("".into()));
            }
        }

        self
    }
}
