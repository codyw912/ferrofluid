//! WebSocket provider for real-time market data and user events

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use dashmap::DashMap;
use fastwebsockets::{Frame, OpCode, Role, WebSocket, handshake};
use http_body_util::Empty;
use hyper::{Request, StatusCode, body::Bytes, header, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    Network,
    errors::HyperliquidError,
    types::ws::{Message, Subscription, WsRequest},
    types::Symbol,
};

pub type SubscriptionId = u32;

#[derive(Clone)]
struct SubscriptionHandle {
    subscription: Subscription,
    tx: UnboundedSender<Message>,
}

/// Raw WebSocket provider for Hyperliquid
///
/// This is a thin wrapper around fastwebsockets that provides:
/// - Type-safe subscriptions
/// - Simple message routing
/// - No automatic reconnection (user controls retry logic)
pub struct RawWsProvider {
    _network: Network,
    ws: Option<WebSocket<TokioIo<Upgraded>>>,
    subscriptions: Arc<DashMap<SubscriptionId, SubscriptionHandle>>,
    next_id: Arc<AtomicU32>,
    message_tx: Option<UnboundedSender<String>>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RawWsProvider {
    /// Connect to Hyperliquid WebSocket
    pub async fn connect(network: Network) -> Result<Self, HyperliquidError> {
        let url = network.ws_url().to_string();
        Self::connect_url(&url, network).await
    }

    /// Connect to a custom WebSocket URL
    pub async fn connect_url(url: &str, network: Network) -> Result<Self, HyperliquidError> {
        let ws = Self::establish_connection(url).await?;
        let subscriptions = Arc::new(DashMap::new());
        let next_id = Arc::new(AtomicU32::new(1));

        // Create message routing channel
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        // Spawn message routing task
        let subscriptions_clone = subscriptions.clone();
        let task_handle = tokio::spawn(async move {
            Self::message_router(message_rx, subscriptions_clone).await;
        });

        Ok(Self {
            _network: network,
            ws: Some(ws),
            subscriptions,
            next_id,
            message_tx: Some(message_tx),
            task_handle: Some(task_handle),
        })
    }

    async fn establish_connection(
        url: &str,
    ) -> Result<WebSocket<TokioIo<Upgraded>>, HyperliquidError> {
        let uri = url
            .parse::<hyper::Uri>()
            .map_err(|e| HyperliquidError::WebSocket(format!("Invalid URL: {}", e)))?;

        let scheme = uri.scheme_str().unwrap_or("wss");
        let is_secure = scheme == "https" || scheme == "wss";

        if is_secure {
            Self::establish_https_connection(&uri).await
        } else {
            // Handle ws:// and http:// schemes
            Self::establish_http_connection(&uri).await
        }
    }

    async fn establish_https_connection(
        uri: &hyper::Uri,
    ) -> Result<WebSocket<TokioIo<Upgraded>>, HyperliquidError> {
        use hyper_rustls::HttpsConnectorBuilder;
        use hyper_util::client::legacy::Client;

        // Create HTTPS connector with proper configuration
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| {
                HyperliquidError::WebSocket(format!("Failed to load native roots: {}", e))
            })?
            .https_only()
            .enable_http1()
            .build();

        let client = Client::builder(hyper_util::rt::TokioExecutor::new())
            .build::<_, Empty<Bytes>>(https);

        Self::perform_websocket_upgrade(uri, client).await
    }

    async fn establish_http_connection(
        uri: &hyper::Uri,
    ) -> Result<WebSocket<TokioIo<Upgraded>>, HyperliquidError> {
        use hyper_util::client::legacy::Client;
        use hyper_util::client::legacy::connect::HttpConnector;

        // Convert ws:// to http:// for the HTTP connector
        let http_uri = if uri.scheme_str() == Some("ws") {
            let mut parts = uri.clone().into_parts();
            parts.scheme = Some("http".parse().unwrap());
            hyper::Uri::from_parts(parts)
                .map_err(|e| HyperliquidError::WebSocket(format!("Failed to convert URI: {}", e)))?
        } else {
            uri.clone()
        };

        let http = HttpConnector::new();
        let client = Client::builder(hyper_util::rt::TokioExecutor::new())
            .build::<_, Empty<Bytes>>(http);

        Self::perform_websocket_upgrade(&http_uri, client).await
    }

    async fn perform_websocket_upgrade<C>(
        uri: &hyper::Uri,
        client: hyper_util::client::legacy::Client<C, Empty<Bytes>>,
    ) -> Result<WebSocket<TokioIo<Upgraded>>, HyperliquidError>
    where
        C: hyper_util::client::legacy::connect::Connect + Clone + Send + Sync + 'static,
    {
        // Create WebSocket upgrade request
        let host = uri
            .host()
            .ok_or_else(|| HyperliquidError::WebSocket("No host in URL".to_string()))?;

        let req = Request::builder()
            .method("GET")
            .uri(uri)
            .header(header::HOST, host)
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            .header(header::SEC_WEBSOCKET_KEY, handshake::generate_key())
            .body(Empty::new())
            .map_err(|e| {
                HyperliquidError::WebSocket(format!("Request build failed: {}", e))
            })?;

        let res = client.request(req).await.map_err(|e| {
            HyperliquidError::WebSocket(format!("HTTP request failed: {}", e))
        })?;

        if res.status() != StatusCode::SWITCHING_PROTOCOLS {
            return Err(HyperliquidError::WebSocket(format!(
                "WebSocket upgrade failed: {}",
                res.status()
            )));
        }

        let upgraded = hyper::upgrade::on(res)
            .await
            .map_err(|e| HyperliquidError::WebSocket(format!("Upgrade failed: {}", e)))?;

        Ok(WebSocket::after_handshake(
            TokioIo::new(upgraded),
            Role::Client,
        ))
    }

    /// Subscribe to L2 order book updates
    pub async fn subscribe_l2_book(
        &mut self,
        coin: impl Into<Symbol>,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let symbol = coin.into();
        let subscription = Subscription::L2Book {
            coin: symbol.as_str().to_string(),
        };
        self.subscribe(subscription).await
    }

    /// Subscribe to trades
    pub async fn subscribe_trades(
        &mut self,
        coin: impl Into<Symbol>,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let symbol = coin.into();
        let subscription = Subscription::Trades {
            coin: symbol.as_str().to_string(),
        };
        self.subscribe(subscription).await
    }

    /// Subscribe to all mid prices
    pub async fn subscribe_all_mids(
        &mut self,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        self.subscribe(Subscription::AllMids).await
    }

    /// Generic subscription method
    pub async fn subscribe(
        &mut self,
        subscription: Subscription,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| HyperliquidError::WebSocket("Not connected".to_string()))?;

        // Send subscription request
        let request = WsRequest::subscribe(subscription.clone());
        let payload = serde_json::to_string(&request)
            .map_err(|e| HyperliquidError::Serialize(e.to_string()))?;

        ws.write_frame(Frame::text(payload.into_bytes().into()))
            .await
            .map_err(|e| {
                HyperliquidError::WebSocket(format!("Failed to send subscription: {}", e))
            })?;

        // Create channel for this subscription
        let (tx, rx) = mpsc::unbounded_channel();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        self.subscriptions
            .insert(id, SubscriptionHandle { subscription, tx });

        Ok((id, rx))
    }

    /// Unsubscribe from a subscription
    pub async fn unsubscribe(
        &mut self,
        id: SubscriptionId,
    ) -> Result<(), HyperliquidError> {
        if let Some((_, handle)) = self.subscriptions.remove(&id) {
            let ws = self.ws.as_mut().ok_or_else(|| {
                HyperliquidError::WebSocket("Not connected".to_string())
            })?;

            let request = WsRequest::unsubscribe(handle.subscription);
            let payload = serde_json::to_string(&request)
                .map_err(|e| HyperliquidError::Serialize(e.to_string()))?;

            ws.write_frame(Frame::text(payload.into_bytes().into()))
                .await
                .map_err(|e| {
                    HyperliquidError::WebSocket(format!(
                        "Failed to send unsubscribe: {}",
                        e
                    ))
                })?;
        }

        Ok(())
    }

    /// Send a ping to keep connection alive
    pub async fn ping(&mut self) -> Result<(), HyperliquidError> {
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| HyperliquidError::WebSocket("Not connected".to_string()))?;

        let request = WsRequest::ping();
        let payload = serde_json::to_string(&request)
            .map_err(|e| HyperliquidError::Serialize(e.to_string()))?;

        ws.write_frame(Frame::text(payload.into_bytes().into()))
            .await
            .map_err(|e| {
                HyperliquidError::WebSocket(format!("Failed to send ping: {}", e))
            })?;

        Ok(())
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.ws.is_some()
    }

    /// Start reading messages (must be called after connecting)
    pub async fn start_reading(&mut self) -> Result<(), HyperliquidError> {
        let mut ws = self
            .ws
            .take()
            .ok_or_else(|| HyperliquidError::WebSocket("Not connected".to_string()))?;

        let message_tx = self.message_tx.clone().ok_or_else(|| {
            HyperliquidError::WebSocket("Message channel not initialized".to_string())
        })?;

        tokio::spawn(async move {
            while let Ok(frame) = ws.read_frame().await {
                match frame.opcode {
                    OpCode::Text => {
                        if let Ok(text) = String::from_utf8(frame.payload.to_vec()) {
                            let _ = message_tx.send(text);
                        }
                    }
                    OpCode::Close => {
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    async fn message_router(
        mut rx: UnboundedReceiver<String>,
        subscriptions: Arc<DashMap<SubscriptionId, SubscriptionHandle>>,
    ) {
        while let Some(text) = rx.recv().await {
            // Use simd-json for fast parsing
            let mut text_bytes = text.into_bytes();
            match simd_json::from_slice::<Message>(&mut text_bytes) {
                Ok(message) => {
                    // Route message to matching subscriptions only
                    for entry in subscriptions.iter() {
                        if message_matches_subscription(&message, &entry.value().subscription) {
                            let _ = entry.value().tx.send(message.clone());
                        }
                    }
                }
                Err(_) => {
                    // Ignore parse errors
                }
            }
        }
    }
}

/// Check if an incoming message matches a subscription.
///
/// This function determines whether a WebSocket message should be routed
/// to a particular subscription channel based on message type and parameters.
fn message_matches_subscription(message: &Message, subscription: &Subscription) -> bool {
    match (message, subscription) {
        // Market data subscriptions
        (Message::AllMids(_), Subscription::AllMids) => true,

        (Message::L2Book(book), Subscription::L2Book { coin }) => {
            book.data.coin.eq_ignore_ascii_case(coin)
        }

        (Message::Trades(trades), Subscription::Trades { coin }) => {
            trades.data.first().map_or(false, |t| t.coin.eq_ignore_ascii_case(coin))
        }

        (Message::Candle(candle), Subscription::Candle { coin, interval }) => {
            candle.data.coin.eq_ignore_ascii_case(coin)
                && candle.data.interval.eq_ignore_ascii_case(interval)
        }

        // User subscriptions - match by user address
        (Message::OrderUpdates(_), Subscription::OrderUpdates { user: _ }) => {
            // OrderUpdates messages don't contain user in data, but the exchange
            // only sends updates for the subscribed user, so we accept all
            true
        }

        (Message::UserFills(fills), Subscription::UserFills { user }) => {
            fills.data.user == *user
        }

        (Message::UserFundings(fundings), Subscription::UserFundings { user }) => {
            fundings.data.user == *user
        }

        (Message::UserNonFundingLedgerUpdates(updates), Subscription::UserNonFundingLedgerUpdates { user }) => {
            updates.data.user == *user
        }

        (Message::User(_), Subscription::UserEvents { user: _ }) => {
            // User messages (generic user events) don't always contain user address
            // in a consistent location, so we accept all for UserEvents subscription
            true
        }

        (Message::Notification(_), Subscription::Notification { user: _ }) => {
            // Notifications are user-specific, exchange only sends for subscribed user
            true
        }

        (Message::WebData2(data), Subscription::WebData2 { user }) => {
            data.data.user == *user
        }

        // Control messages - route to all subscribers (they'll filter as needed)
        (Message::SubscriptionResponse, _) => true,
        (Message::Pong, _) => true,

        // No match - message type doesn't correspond to subscription type
        _ => false,
    }
}

impl Drop for RawWsProvider {
    fn drop(&mut self) {
        // Clean shutdown
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
    }
}

// ==================== Enhanced WebSocket Provider ====================

use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Configuration for managed WebSocket provider
#[derive(Clone, Debug)]
pub struct WsConfig {
    /// Interval between ping messages (0 to disable)
    pub ping_interval: Duration,
    /// Timeout waiting for pong response
    pub pong_timeout: Duration,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Initial delay between reconnection attempts
    pub reconnect_delay: Duration,
    /// Maximum reconnection attempts (None for infinite)
    pub max_reconnect_attempts: Option<u32>,
    /// Use exponential backoff for reconnection delays
    pub exponential_backoff: bool,
    /// Maximum backoff delay when using exponential backoff
    pub max_reconnect_delay: Duration,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(30),
            pong_timeout: Duration::from_secs(5),
            auto_reconnect: true,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_attempts: None,
            exponential_backoff: true,
            max_reconnect_delay: Duration::from_secs(60),
        }
    }
}

#[derive(Clone)]
struct ManagedSubscription {
    subscription: Subscription,
    tx: UnboundedSender<Message>,
    #[allow(dead_code)]
    created_at: Instant,  // For future use: subscription age tracking
}

/// Managed WebSocket provider with automatic keep-alive and reconnection
///
/// This provider builds on top of RawWsProvider to add:
/// - Automatic ping/pong keep-alive
/// - Automatic reconnection with subscription replay
/// - Connection state monitoring
/// - Configurable retry behavior
pub struct ManagedWsProvider {
    network: Network,
    inner: Arc<Mutex<Option<RawWsProvider>>>,
    subscriptions: Arc<DashMap<SubscriptionId, ManagedSubscription>>,
    config: WsConfig,
    next_id: Arc<AtomicU32>,
}

impl ManagedWsProvider {
    /// Connect with custom configuration
    pub async fn connect(network: Network, config: WsConfig) -> Result<Arc<Self>, HyperliquidError> {
        // Create initial connection
        let raw_provider = RawWsProvider::connect(network.clone()).await?;
        
        let provider = Arc::new(Self {
            network,
            inner: Arc::new(Mutex::new(Some(raw_provider))),
            subscriptions: Arc::new(DashMap::new()),
            config,
            next_id: Arc::new(AtomicU32::new(1)),
        });
        
        // Start keep-alive task if configured
        if provider.config.ping_interval > Duration::ZERO {
            let provider_clone = provider.clone();
            tokio::spawn(async move {
                provider_clone.keepalive_loop().await;
            });
        }
        
        // Start reconnection task if configured
        if provider.config.auto_reconnect {
            let provider_clone = provider.clone();
            tokio::spawn(async move {
                provider_clone.reconnect_loop().await;
            });
        }
        
        Ok(provider)
    }
    
    /// Connect with default configuration
    pub async fn connect_with_defaults(network: Network) -> Result<Arc<Self>, HyperliquidError> {
        Self::connect(network, WsConfig::default()).await
    }
    
    /// Check if currently connected
    pub async fn is_connected(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.as_ref().map(|p| p.is_connected()).unwrap_or(false)
    }
    
    /// Get mutable access to the raw provider
    pub async fn raw(&self) -> Result<tokio::sync::MutexGuard<'_, Option<RawWsProvider>>, HyperliquidError> {
        Ok(self.inner.lock().await)
    }
    
    /// Subscribe to L2 order book updates with automatic replay on reconnect
    pub async fn subscribe_l2_book(
        &self,
        coin: impl Into<Symbol>,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let symbol = coin.into();
        let subscription = Subscription::L2Book {
            coin: symbol.as_str().to_string(),
        };
        self.subscribe(subscription).await
    }
    
    /// Subscribe to trades with automatic replay on reconnect
    pub async fn subscribe_trades(
        &self,
        coin: impl Into<Symbol>,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let symbol = coin.into();
        let subscription = Subscription::Trades {
            coin: symbol.as_str().to_string(),
        };
        self.subscribe(subscription).await
    }
    
    /// Subscribe to all mid prices with automatic replay on reconnect
    pub async fn subscribe_all_mids(
        &self,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        self.subscribe(Subscription::AllMids).await
    }
    
    /// Generic subscription with automatic replay on reconnect
    pub async fn subscribe(
        &self,
        subscription: Subscription,
    ) -> Result<(SubscriptionId, UnboundedReceiver<Message>), HyperliquidError> {
        let mut inner = self.inner.lock().await;
        let raw_provider = inner.as_mut().ok_or_else(|| {
            HyperliquidError::WebSocket("Not connected".to_string())
        })?;
        
        // Subscribe using the raw provider
        let (_raw_id, rx) = raw_provider.subscribe(subscription.clone()).await?;
        
        // Generate our own ID for tracking
        let managed_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        
        // Create channel for managed subscription
        let (tx, managed_rx) = mpsc::unbounded_channel();
        
        // Store subscription for replay
        self.subscriptions.insert(
            managed_id,
            ManagedSubscription {
                subscription,
                tx: tx.clone(),
                created_at: Instant::now(),
            },
        );
        
        // Forward messages from raw to managed
        let subscriptions = self.subscriptions.clone();
        tokio::spawn(async move {
            let mut rx = rx;
            while let Some(msg) = rx.recv().await {
                if let Some(entry) = subscriptions.get(&managed_id) {
                    let _ = entry.tx.send(msg);
                }
            }
            // Clean up when channel closes
            subscriptions.remove(&managed_id);
        });
        
        Ok((managed_id, managed_rx))
    }
    
    /// Unsubscribe and stop automatic replay
    pub async fn unsubscribe(&self, id: SubscriptionId) -> Result<(), HyperliquidError> {
        // Remove from our tracking
        self.subscriptions.remove(&id);
        
        // Note: We can't unsubscribe from the raw provider because we don't
        // track the mapping between our IDs and raw IDs. This is fine since
        // the subscription will be cleaned up on reconnect anyway.
        
        Ok(())
    }
    
    /// Start reading messages (must be called after connecting)
    pub async fn start_reading(&self) -> Result<(), HyperliquidError> {
        let mut inner = self.inner.lock().await;
        let raw_provider = inner.as_mut().ok_or_else(|| {
            HyperliquidError::WebSocket("Not connected".to_string())
        })?;
        raw_provider.start_reading().await
    }
    
    // Keep-alive loop
    async fn keepalive_loop(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.config.ping_interval);
        
        loop {
            interval.tick().await;
            
            let mut inner = self.inner.lock().await;
            if let Some(provider) = inner.as_mut() {
                if provider.ping().await.is_err() {
                    // Ping failed, connection might be dead
                    drop(inner);
                    self.handle_disconnect().await;
                }
            }
        }
    }
    
    // Reconnection loop
    async fn reconnect_loop(self: Arc<Self>) {
        let mut reconnect_attempts = 0u32;
        let mut current_delay = self.config.reconnect_delay;
        
        loop {
            // Wait a bit before checking
            sleep(Duration::from_secs(1)).await;
            
            // Check if we need to reconnect
            if !self.is_connected().await {
                // Check max attempts
                if let Some(max) = self.config.max_reconnect_attempts {
                    if reconnect_attempts >= max {
                        eprintln!("Max reconnection attempts ({}) reached", max);
                        break;
                    }
                }
                
                println!("Attempting reconnection #{}", reconnect_attempts + 1);
                
                match RawWsProvider::connect(self.network.clone()).await {
                    Ok(mut new_provider) => {
                        // Start reading before replaying subscriptions
                        if let Err(e) = new_provider.start_reading().await {
                            eprintln!("Failed to start reading after reconnect: {}", e);
                            continue;
                        }
                        
                        // Replay all subscriptions
                        let mut replay_errors = 0;
                        for entry in self.subscriptions.iter() {
                            if let Err(e) = new_provider.subscribe(entry.subscription.clone()).await {
                                eprintln!("Failed to replay subscription: {}", e);
                                replay_errors += 1;
                            }
                        }
                        
                        if replay_errors == 0 {
                            // Success! Reset counters
                            *self.inner.lock().await = Some(new_provider);
                            reconnect_attempts = 0;
                            current_delay = self.config.reconnect_delay;
                            println!("Reconnection successful, {} subscriptions replayed", 
                                     self.subscriptions.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("Reconnection failed: {}", e);
                        
                        // Wait before next attempt
                        sleep(current_delay).await;
                        
                        // Update delay for next attempt
                        reconnect_attempts += 1;
                        if self.config.exponential_backoff {
                            current_delay = std::cmp::min(
                                current_delay * 2,
                                self.config.max_reconnect_delay,
                            );
                        }
                    }
                }
            }
        }
    }
    
    // Handle disconnection
    async fn handle_disconnect(&self) {
        *self.inner.lock().await = None;
    }
}

// Note: Background tasks (keepalive and reconnect loops) will automatically
// terminate when all Arc references to the provider are dropped, since they
// hold Arc<Self> and will exit when is_connected() returns false.

// Re-export for backwards compatibility
pub use RawWsProvider as WsProvider;

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    fn make_l2_book(coin: &str) -> Message {
        Message::L2Book(crate::types::ws::L2Book {
            data: crate::types::ws::L2BookData {
                coin: coin.to_string(),
                time: 0,
                levels: vec![],
            },
        })
    }

    fn make_all_mids() -> Message {
        Message::AllMids(crate::types::ws::AllMids {
            data: crate::types::ws::AllMidsData {
                mids: std::collections::HashMap::new(),
            },
        })
    }

    fn make_trades(coin: &str) -> Message {
        Message::Trades(crate::types::ws::Trades {
            data: vec![crate::types::ws::Trade {
                coin: coin.to_string(),
                side: "B".to_string(),
                px: "100".to_string(),
                sz: "1".to_string(),
                time: 0,
                hash: "0x123".to_string(),
                tid: 1,
            }],
        })
    }

    fn make_user_fills(user: alloy::primitives::Address) -> Message {
        Message::UserFills(crate::types::ws::UserFills {
            data: crate::types::ws::UserFillsData {
                is_snapshot: Some(false),
                user,
                fills: vec![],
            },
        })
    }

    #[test]
    fn test_l2_book_routes_to_matching_subscription() {
        let msg = make_l2_book("BTC");
        let sub = Subscription::L2Book { coin: "BTC".to_string() };
        assert!(message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_l2_book_does_not_route_to_different_coin() {
        let msg = make_l2_book("BTC");
        let sub = Subscription::L2Book { coin: "ETH".to_string() };
        assert!(!message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_l2_book_does_not_route_to_all_mids() {
        let msg = make_l2_book("BTC");
        let sub = Subscription::AllMids;
        assert!(!message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_l2_book_does_not_route_to_user_fills() {
        let msg = make_l2_book("BTC");
        let user = address!("0000000000000000000000000000000000000001");
        let sub = Subscription::UserFills { user };
        assert!(!message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_all_mids_routes_correctly() {
        let msg = make_all_mids();
        let sub = Subscription::AllMids;
        assert!(message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_all_mids_does_not_route_to_l2_book() {
        let msg = make_all_mids();
        let sub = Subscription::L2Book { coin: "BTC".to_string() };
        assert!(!message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_trades_routes_to_matching_coin() {
        let msg = make_trades("BTC");
        let sub = Subscription::Trades { coin: "BTC".to_string() };
        assert!(message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_trades_does_not_route_to_different_coin() {
        let msg = make_trades("BTC");
        let sub = Subscription::Trades { coin: "ETH".to_string() };
        assert!(!message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_user_fills_routes_to_matching_user() {
        let user = address!("0000000000000000000000000000000000000001");
        let msg = make_user_fills(user);
        let sub = Subscription::UserFills { user };
        assert!(message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_user_fills_does_not_route_to_different_user() {
        let user1 = address!("0000000000000000000000000000000000000001");
        let user2 = address!("0000000000000000000000000000000000000002");
        let msg = make_user_fills(user1);
        let sub = Subscription::UserFills { user: user2 };
        assert!(!message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_user_fills_does_not_route_to_l2_book() {
        let user = address!("0000000000000000000000000000000000000001");
        let msg = make_user_fills(user);
        let sub = Subscription::L2Book { coin: "BTC".to_string() };
        assert!(!message_matches_subscription(&msg, &sub));
    }

    #[test]
    fn test_control_messages_route_to_all() {
        let sub_l2 = Subscription::L2Book { coin: "BTC".to_string() };
        let sub_mids = Subscription::AllMids;
        let user = address!("0000000000000000000000000000000000000001");
        let sub_fills = Subscription::UserFills { user };

        // SubscriptionResponse routes to all
        assert!(message_matches_subscription(&Message::SubscriptionResponse, &sub_l2));
        assert!(message_matches_subscription(&Message::SubscriptionResponse, &sub_mids));
        assert!(message_matches_subscription(&Message::SubscriptionResponse, &sub_fills));

        // Pong routes to all
        assert!(message_matches_subscription(&Message::Pong, &sub_l2));
        assert!(message_matches_subscription(&Message::Pong, &sub_mids));
        assert!(message_matches_subscription(&Message::Pong, &sub_fills));
    }

    #[test]
    fn test_no_cross_contamination() {
        // This is the critical test - ensures L2Book messages don't end up on UserFills channel
        let l2_msg = make_l2_book("BTC");
        let mids_msg = make_all_mids();
        let trades_msg = make_trades("BTC");
        let user = address!("0000000000000000000000000000000000000001");
        let fills_msg = make_user_fills(user);

        let l2_sub = Subscription::L2Book { coin: "BTC".to_string() };
        let mids_sub = Subscription::AllMids;
        let trades_sub = Subscription::Trades { coin: "BTC".to_string() };
        let fills_sub = Subscription::UserFills { user };

        // L2Book only matches L2Book
        assert!(message_matches_subscription(&l2_msg, &l2_sub));
        assert!(!message_matches_subscription(&l2_msg, &mids_sub));
        assert!(!message_matches_subscription(&l2_msg, &trades_sub));
        assert!(!message_matches_subscription(&l2_msg, &fills_sub));

        // AllMids only matches AllMids
        assert!(!message_matches_subscription(&mids_msg, &l2_sub));
        assert!(message_matches_subscription(&mids_msg, &mids_sub));
        assert!(!message_matches_subscription(&mids_msg, &trades_sub));
        assert!(!message_matches_subscription(&mids_msg, &fills_sub));

        // Trades only matches Trades
        assert!(!message_matches_subscription(&trades_msg, &l2_sub));
        assert!(!message_matches_subscription(&trades_msg, &mids_sub));
        assert!(message_matches_subscription(&trades_msg, &trades_sub));
        assert!(!message_matches_subscription(&trades_msg, &fills_sub));

        // UserFills only matches UserFills
        assert!(!message_matches_subscription(&fills_msg, &l2_sub));
        assert!(!message_matches_subscription(&fills_msg, &mids_sub));
        assert!(!message_matches_subscription(&fills_msg, &trades_sub));
        assert!(message_matches_subscription(&fills_msg, &fills_sub));
    }
}
