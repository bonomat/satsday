use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub ark_server_url: String,
    pub esplora_url: String,
    pub seed_file: String,
}

impl Config {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}