// ==================== Network Configuration ====================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
    /// Custom network with user-specified URLs
    Custom {
        api_url: String,
        ws_url: String,
    },
}

impl Network {
    pub fn api_url(&self) -> &str {
        match self {
            Network::Mainnet => "https://api.hyperliquid.xyz",
            Network::Testnet => "https://api.hyperliquid-testnet.xyz",
            Network::Custom { api_url, .. } => api_url,
        }
    }

    pub fn ws_url(&self) -> &str {
        match self {
            Network::Mainnet => "wss://api.hyperliquid.xyz/ws",
            Network::Testnet => "wss://api.hyperliquid-testnet.xyz/ws",
            Network::Custom { ws_url, .. } => ws_url,
        }
    }

    /// Create a localhost network for testing
    pub fn localhost(port: u16) -> Self {
        Network::Custom {
            api_url: format!("http://localhost:{}", port),
            ws_url: format!("ws://localhost:{}/ws", port),
        }
    }
}

// ==================== Chain Configuration ====================

// Chain IDs
pub const CHAIN_ID_MAINNET: u64 = 42161; // Arbitrum One
pub const CHAIN_ID_TESTNET: u64 = 421614; // Arbitrum Sepolia

// Agent Sources
pub const AGENT_SOURCE_MAINNET: &str = "a";
pub const AGENT_SOURCE_TESTNET: &str = "b";

// Exchange Endpoints
pub const EXCHANGE_ENDPOINT_MAINNET: &str = "https://api.hyperliquid.xyz/exchange";
pub const EXCHANGE_ENDPOINT_TESTNET: &str =
    "https://api.hyperliquid-testnet.xyz/exchange";

// ==================== Rate Limit Weights ====================

// Info endpoints
pub const WEIGHT_ALL_MIDS: u32 = 2;
pub const WEIGHT_L2_BOOK: u32 = 1;
pub const WEIGHT_USER_STATE: u32 = 2;
pub const WEIGHT_USER_FILLS: u32 = 2;
pub const WEIGHT_USER_FUNDING: u32 = 2;
pub const WEIGHT_USER_FEES: u32 = 1;
pub const WEIGHT_OPEN_ORDERS: u32 = 1;
pub const WEIGHT_ORDER_STATUS: u32 = 1;
pub const WEIGHT_RECENT_TRADES: u32 = 1;
pub const WEIGHT_CANDLES: u32 = 2;
pub const WEIGHT_FUNDING_HISTORY: u32 = 2;
pub const WEIGHT_TOKEN_BALANCES: u32 = 1;
pub const WEIGHT_REFERRAL: u32 = 1;

// Exchange endpoints (these have higher weights)
pub const WEIGHT_PLACE_ORDER: u32 = 3;
pub const WEIGHT_CANCEL_ORDER: u32 = 2;
pub const WEIGHT_MODIFY_ORDER: u32 = 3;
pub const WEIGHT_BULK_ORDER: u32 = 10;
pub const WEIGHT_BULK_CANCEL: u32 = 8;

// ==================== Rate Limit Configuration ====================

pub const RATE_LIMIT_MAX_TOKENS: u32 = 1200;
pub const RATE_LIMIT_REFILL_RATE: u32 = 600; // per minute

// ==================== Time Constants ====================

pub const NONCE_WINDOW_MS: u64 = 60_000; // 60 seconds

// ==================== Order Constants ====================

pub const TIF_GTC: &str = "Gtc";
pub const TIF_IOC: &str = "Ioc";
pub const TIF_ALO: &str = "Alo";

pub const TPSL_TP: &str = "tp";
pub const TPSL_SL: &str = "sl";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_mainnet_urls() {
        let network = Network::Mainnet;
        assert_eq!(network.api_url(), "https://api.hyperliquid.xyz");
        assert_eq!(network.ws_url(), "wss://api.hyperliquid.xyz/ws");
    }

    #[test]
    fn test_network_testnet_urls() {
        let network = Network::Testnet;
        assert_eq!(network.api_url(), "https://api.hyperliquid-testnet.xyz");
        assert_eq!(network.ws_url(), "wss://api.hyperliquid-testnet.xyz/ws");
    }

    #[test]
    fn test_network_custom_urls() {
        let network = Network::Custom {
            api_url: "https://custom.example.com".to_string(),
            ws_url: "wss://custom.example.com/ws".to_string(),
        };
        assert_eq!(network.api_url(), "https://custom.example.com");
        assert_eq!(network.ws_url(), "wss://custom.example.com/ws");
    }

    #[test]
    fn test_network_localhost() {
        let network = Network::localhost(8080);
        assert_eq!(network.api_url(), "http://localhost:8080");
        assert_eq!(network.ws_url(), "ws://localhost:8080/ws");
    }

    #[test]
    fn test_network_localhost_different_ports() {
        let network1 = Network::localhost(3000);
        let network2 = Network::localhost(9000);

        assert_eq!(network1.api_url(), "http://localhost:3000");
        assert_eq!(network1.ws_url(), "ws://localhost:3000/ws");

        assert_eq!(network2.api_url(), "http://localhost:9000");
        assert_eq!(network2.ws_url(), "ws://localhost:9000/ws");
    }

    #[test]
    fn test_network_equality() {
        assert_eq!(Network::Mainnet, Network::Mainnet);
        assert_eq!(Network::Testnet, Network::Testnet);
        assert_ne!(Network::Mainnet, Network::Testnet);

        let custom1 = Network::Custom {
            api_url: "http://a.com".to_string(),
            ws_url: "ws://a.com/ws".to_string(),
        };
        let custom2 = Network::Custom {
            api_url: "http://a.com".to_string(),
            ws_url: "ws://a.com/ws".to_string(),
        };
        let custom3 = Network::Custom {
            api_url: "http://b.com".to_string(),
            ws_url: "ws://b.com/ws".to_string(),
        };

        assert_eq!(custom1, custom2);
        assert_ne!(custom1, custom3);
        assert_ne!(custom1, Network::Mainnet);
    }

    #[test]
    fn test_network_clone() {
        let network = Network::localhost(8080);
        let cloned = network.clone();
        assert_eq!(network, cloned);
    }
}
