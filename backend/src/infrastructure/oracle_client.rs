use anyhow::{Result, Context};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Pyth price feed IDs for different assets
#[derive(Debug, Clone)]
pub struct AssetConfig {
    pub symbol: String,
    pub pyth_price_id: String, // Hex string without 0x
}

pub struct OracleClient {
    http_client: reqwest::Client,
    base_url: String,
    asset_configs: HashMap<String, AssetConfig>,
    latest_prices: Arc<RwLock<HashMap<String, Decimal>>>,
}

impl OracleClient {
    pub fn new(base_url: String) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            base_url,
            asset_configs: HashMap::new(),
            latest_prices: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Create with default Pyth Hermes API
    pub fn new_hermes() -> Self {
        Self::new("https://hermes.pyth.network".to_string())
    }
    
    /// Add an asset to monitor
    pub fn add_asset(&mut self, config: AssetConfig) {
        self.asset_configs.insert(config.symbol.clone(), config);
    }
    
    /// Configure with default Pyth price feeds
    pub fn with_mainnet_defaults(mut self) -> Self {
        // BTC/USD
        self.add_asset(AssetConfig {
            symbol: "BTC-USD".to_string(),
            pyth_price_id: "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43".to_string(),
        });
        
        // ETH/USD
        self.add_asset(AssetConfig {
            symbol: "ETH-USD".to_string(),
            pyth_price_id: "ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace".to_string(),
        });
        
        // SOL/USD
        self.add_asset(AssetConfig {
            symbol: "SOL-USD".to_string(),
            pyth_price_id: "ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d".to_string(),
        });
        
        self
    }
    
    /// Fetch current price for an asset via HTTP API
    pub async fn fetch_price(&self, symbol: &str) -> Result<Decimal> {
        let config = self.asset_configs
            .get(symbol)
            .ok_or_else(|| anyhow::anyhow!("Asset not configured: {}", symbol))?;
        
        // Construct Pyth Hermes API URL
        let url = format!(
            "{}/v2/updates/price/latest?ids[]=0x{}",
            self.base_url,
            config.pyth_price_id
        );
        
        // Fetch from API
        let response = self.http_client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch price from Pyth API")?;
        
        // Parse response
        let price_response: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Pyth API response")?;
        
        // Extract price data
        let parsed = price_response
            .get("parsed")
            .and_then(|p| p.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("price"))
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;
        
        let price_str = parsed
            .get("price")
            .and_then(|p| p.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing price field"))?;
        
        let expo = parsed
            .get("expo")
            .and_then(|e| e.as_i64())
            .ok_or_else(|| anyhow::anyhow!("Missing expo field"))? as i32;
        
        let conf_str = parsed
            .get("conf")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing conf field"))?;
        
        // Parse price and convert to decimal
        let price_i64: i64 = price_str.parse()
            .context("Failed to parse price")?;
        
        let conf_u64: u64 = conf_str.parse()
            .context("Failed to parse confidence")?;
        
        // Convert to Decimal
        let price_value = if expo >= 0 {
            let multiplier = 10_i64.pow(expo as u32);
            Decimal::new(price_i64 * multiplier, 0)
        } else {
            Decimal::new(price_i64, expo.unsigned_abs())
        };
        
        let conf_value = Decimal::new(conf_u64 as i64, expo.unsigned_abs());
        
        tracing::debug!(
            "Price for {}: {} ± {} (conf)",
            symbol,
            price_value,
            conf_value
        );
        
        // Update cache
        let mut latest_prices = self.latest_prices.write().await;
        latest_prices.insert(symbol.to_string(), price_value);
        
        Ok(price_value)
    }
    
    /// Get cached price (non-blocking)
    pub async fn get_cached_price(&self, symbol: &str) -> Option<Decimal> {
        let latest_prices = self.latest_prices.read().await;
        latest_prices.get(symbol).copied()
    }
    
    /// Get all configured symbols
    pub fn get_symbols(&self) -> Vec<String> {
        self.asset_configs.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use solana_sdk::msg;

    use super::*;
    
    #[test]
    fn test_oracle_configuration() {
        let oracle: OracleClient = OracleClient::new_hermes()
            .with_mainnet_defaults();
        
        let symbols = oracle.get_symbols();
        assert!(symbols.contains(&"BTC-USD".to_string()));
        assert!(symbols.contains(&"ETH-USD".to_string()));
    }
    
    #[tokio::test]
    async fn test_fetch_price() {
        let oracle = OracleClient::new_hermes()
            .with_mainnet_defaults();
        
        msg!("got oracle {}", oracle.base_url);
        // This will actually hit the API
        let price = oracle.fetch_price("BTC-USD").await;

        msg!("got price {:?}", price);
        
        match price {
            Ok(p) => {
                println!("BTC-USD price: {}", p);
                assert!(p > Decimal::ZERO);
            }
            Err(e) => {
                println!("Failed to fetch price (might be rate limited): {}", e);
            }
        }
    }
}
