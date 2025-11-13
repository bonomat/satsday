use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub ark_server_url: String,
    pub esplora_url: String,
    pub master_seed_file: String,
    pub database: String,
    #[serde(default = "default_transaction_check_interval")]
    pub transaction_check_interval_seconds: u64,
    #[serde(default = "default_max_payout_sats")]
    pub max_payout_sats: u64,
    #[serde(default = "default_vtxo_sync_interval")]
    pub vtxo_sync_interval_seconds: u64,
}

fn default_transaction_check_interval() -> u64 {
    10
}

fn default_max_payout_sats() -> u64 {
    100_000
}

fn default_vtxo_sync_interval() -> u64 {
    300 // 5 minutes
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get telegram bot token from environment variable
    pub fn telegram_bot_token() -> Option<String> {
        std::env::var("TELEGRAM_BOT_KEY").ok()
    }
}
