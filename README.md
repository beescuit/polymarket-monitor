# polymarket-monitor

A simple service that scrapes and indexes active Polymarket markets.

This application periodically scans the /events gamma API, listens for new markets via the CLOB websocket and indexes all active markets in memory. It exposes the data via HTTP REST and streams changes via Websocket.

Usage:
```sh
docker build . -t polymarket-monitor
docker run -e PORT=8080 -p 8080:8080 polymarket-monitor
```

Other optional enviroment variables:
- `UPDATE_INTERVAL` - REST (gamma) api scraping interval in minutes (defaults to 5);
- `GAMMA_URL` - URL of the gamma api;
- `CLOB_URL` - WSS url for the `new_market` CLOB feed;

## API

### GET /markets

**Query Parameters:**
- category (string) - Can be `sports`, `updown`, `weather`, `none` or the first tag returned by the events api;
- min_volume (f64);
- volume_sort (bool);

**Response:**

```json
[
  {
    "id": "123456",
    "condition_id": "0xabc...",
    "slug": "will-x-happen",
    "question": "Will X happen by end of 2026?",
    "token_ids": ["12345", "67890"],
    "outcomes": ["Yes", "No"],
    "volume_24h": 50000.0,
    "category": "sports",
    "first_seen": "2026-04-01T12:00:00Z",
    "source": "Rest"
  }
]
```

### WS /ws

No subscription message is needed.

**Event types:**

`NewMarket` — emitted when a new market is detected:

```json
{
  "NewMarket": {
    "id": "123456",
    "condition_id": "0xabc...",
    "slug": "will-x-happen",
    "question": "Will X happen by end of 2026?",
    "token_ids": ["12345", "67890"],
    "outcomes": ["Yes", "No"],
    "volume_24h": 0.0,
    "category": "none",
    "first_seen": "2026-04-01T12:00:00Z",
    "source": "WebSocket"
  }
}
```

`MarketClosed` — emitted when a market is removed from the active list:

```json
{
  "MarketClosed": {
    "id": "123456",
    "condition_id": "0xabc...",
    "slug": "will-x-happen",
    "question": "Will X happen by end of 2026?",
    "token_ids": ["12345", "67890"],
    "outcomes": ["Yes", "No"],
    "volume_24h": 50000.0,
    "category": "sports",
    "first_seen": "2026-04-01T12:00:00Z",
    "source": "Rest"
  }
}
```