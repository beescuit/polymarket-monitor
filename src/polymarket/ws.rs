use futures_util::{SinkExt, Stream, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info};

pub struct WsMarketClient {
    url: String,
}

impl WsMarketClient {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub fn subscribe(&self) -> impl Stream<Item = Result<NewMarketMessage, anyhow::Error>> {
        async_stream::stream! {
            loop {
                match self.connect().await {
                    Ok(stream) => {
                        tokio::pin!(stream);
                        while let Some(event) = stream.next().await {
                            yield event;
                        }
                    }
                    Err(e) => {
                        yield Err(e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }

                error!("WebSocket connection lost, attempting to reconnect...");
            }
        }
    }

    async fn connect(
        &self,
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<NewMarketMessage>>> {
        let (ws, _) = connect_async(&self.url).await?;
        let (mut write, read) = ws.split();

        write
            .send(Message::Text(
                r#"{"type":"new_market","custom_feature_enabled":true}"#.into(),
            ))
            .await?;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                match write.send(Message::Ping("PING".into())).await {
                    Ok(_) => debug!("pinged"),
                    Err(e) => {
                        error!("Failed to send ping: {}", e);
                        break;
                    }
                }
            }
        });

        let stream = read.filter_map(|msg| async {
            debug!("Received message: {:?}", msg);
            match msg {
                Ok(Message::Text(text)) => match serde_json::from_str(text.as_str()) {
                    Ok(new_market_msg) => Some(Ok(new_market_msg)),
                    Err(e) => {
                        error!("Failed to parse message: {}", e);
                        None
                    }
                },
                Ok(Message::Pong(_)) => {
                    debug!("Received pong");
                    None
                }
                Ok(Message::Close(frame)) => {
                    info!("WebSocket closed: {:?}", frame);
                    None
                }
                Ok(_) => None,
                Err(e) => Some(Err(e.into())),
            }
        });

        Ok(stream)
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct NewMarketMessage {
    pub id: String,
    pub question: String,
    pub market: String,
    pub slug: String,
    pub description: String,
    pub assets_ids: Vec<String>,
    pub outcomes: Vec<String>,
}
