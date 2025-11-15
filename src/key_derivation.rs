use anyhow::anyhow;
use anyhow::Result;
use bitcoin::bip32::DerivationPath;
use bitcoin::bip32::Xpriv;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use std::str::FromStr;

/// Multiplier options for the dice game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Multiplier {
    X105,    // 1.05x
    X110,    // 1.10x
    X133,    // 1.33x
    X150,    // 1.5x
    X200,    // 2.00x
    X300,    // 3.00x
    X1000,   // 10.00x
    X2500,   // 25.00x
    X5000,   // 50.00x
    X10000,  // 100.00x
    X100000, // 1000.00x
}

impl Multiplier {
    /// Get the actual multiplier value (e.g., 1.05 becomes 105, 2.0 becomes 200)
    pub fn multiplier(&self) -> u64 {
        match self {
            Multiplier::X105 => 105,
            Multiplier::X110 => 110,
            Multiplier::X133 => 133,
            Multiplier::X150 => 150,
            Multiplier::X200 => 200,
            Multiplier::X300 => 300,
            Multiplier::X1000 => 1000,
            Multiplier::X2500 => 2500,
            Multiplier::X5000 => 5000,
            Multiplier::X10000 => 10000,
            Multiplier::X100000 => 100000,
        }
    }

    /// Get the index for derivation path
    pub fn index(&self) -> u32 {
        match self {
            Multiplier::X105 => 0,
            Multiplier::X110 => 1,
            Multiplier::X133 => 2,
            Multiplier::X150 => 3,
            Multiplier::X200 => 4,
            Multiplier::X300 => 5,
            Multiplier::X1000 => 6,
            Multiplier::X2500 => 7,
            Multiplier::X5000 => 8,
            Multiplier::X10000 => 9,
            Multiplier::X100000 => 10,
        }
    }

    pub const fn get_lower_than(&self) -> u16 {
        match self {
            Multiplier::X105 => 60_541,
            Multiplier::X110 => 57_789,
            Multiplier::X133 => 47_796,
            Multiplier::X150 => 42_379,
            Multiplier::X200 => 31_784,
            Multiplier::X300 => 21_189,
            Multiplier::X1000 => 6_356,
            Multiplier::X2500 => 2_542,
            Multiplier::X5000 => 1_271,
            Multiplier::X10000 => 635,
            Multiplier::X100000 => 64,
        }
    }

    pub(crate) fn is_win(&self, rolled_number: u16) -> bool {
        rolled_number < self.get_lower_than()
    }

    /// Create from index
    pub fn from_index(index: u32) -> Option<Self> {
        match index {
            0 => Some(Multiplier::X105),
            1 => Some(Multiplier::X110),
            2 => Some(Multiplier::X133),
            3 => Some(Multiplier::X150),
            4 => Some(Multiplier::X200),
            5 => Some(Multiplier::X300),
            6 => Some(Multiplier::X1000),
            7 => Some(Multiplier::X2500),
            8 => Some(Multiplier::X5000),
            9 => Some(Multiplier::X10000),
            10 => Some(Multiplier::X100000),
            _ => None,
        }
    }

    /// Get all multipliers
    pub fn all() -> Vec<Self> {
        vec![
            Multiplier::X105,
            Multiplier::X110,
            Multiplier::X133,
            Multiplier::X150,
            Multiplier::X200,
            Multiplier::X300,
            Multiplier::X1000,
            Multiplier::X2500,
            Multiplier::X5000,
            Multiplier::X10000,
            Multiplier::X100000,
        ]
    }

    /// Create from stored multiplier value (e.g., 105 for 1.05x, 200 for 2.0x)
    pub fn from_value(value: u64) -> Option<Self> {
        match value {
            105 => Some(Multiplier::X105),
            110 => Some(Multiplier::X110),
            133 => Some(Multiplier::X133),
            150 => Some(Multiplier::X150),
            200 => Some(Multiplier::X200),
            300 => Some(Multiplier::X300),
            1000 => Some(Multiplier::X1000),
            2500 => Some(Multiplier::X2500),
            5000 => Some(Multiplier::X5000),
            10000 => Some(Multiplier::X10000),
            100000 => Some(Multiplier::X100000),
            _ => None,
        }
    }
}

impl std::fmt::Display for Multiplier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.multiplier() as f64 / 100.0;
        if value < 2.0 {
            write!(f, "{value:.2}x")
        } else {
            write!(f, "{value:.0}x")
        }
    }
}

/// Key derivation manager for HD wallet
pub struct KeyDerivation {
    master_key: Xpriv,
}

impl KeyDerivation {
    /// Create a new key derivation manager from a master extended private key
    pub fn new(master_xpriv: &str) -> Result<Self> {
        let master_key =
            Xpriv::from_str(master_xpriv).map_err(|e| anyhow!("Invalid master key: {}", e))?;

        Ok(KeyDerivation { master_key })
    }

    /// Create from a hex seed (32 bytes)
    pub fn from_seed(seed_hex: &str, network: Network) -> Result<Self> {
        let seed_bytes = hex::decode(seed_hex).map_err(|e| anyhow!("Invalid hex seed: {}", e))?;

        if seed_bytes.len() != 32 {
            return Err(anyhow!("Seed must be 32 bytes"));
        }

        let _secp = Secp256k1::new();
        let master_key = Xpriv::new_master(network, &seed_bytes)
            .map_err(|e| anyhow!("Failed to create master key: {}", e))?;

        Ok(KeyDerivation { master_key })
    }

    /// Get the main operational key (m/84'/0'/0'/0/0)
    pub fn get_main_key(&self) -> Result<Xpriv> {
        let path = DerivationPath::from_str("m/84'/0'/0'/0/0")?;
        let secp = Secp256k1::new();
        Ok(self.master_key.derive_priv(&secp, &path)?)
    }

    /// Get a game key for a specific multiplier (m/84'/0'/0'/1/{index})
    pub fn get_game_key(&self, multiplier: Multiplier) -> Result<Xpriv> {
        let path_str = format!("m/84'/0'/0'/1/{}", multiplier.index());
        let path = DerivationPath::from_str(&path_str)?;
        let secp = Secp256k1::new();
        Ok(self.master_key.derive_priv(&secp, &path)?)
    }

    /// Get all game keys as a vector
    pub fn get_all_game_keys(&self) -> Result<Vec<(Multiplier, Xpriv)>> {
        let mut keys = Vec::new();
        for multiplier in Multiplier::all() {
            let key = self.get_game_key(multiplier)?;
            keys.push((multiplier, key));
        }
        Ok(keys)
    }

    /// Get the secret key bytes for a specific multiplier
    pub fn get_game_secret_key(&self, multiplier: Multiplier) -> Result<[u8; 32]> {
        let key = self.get_game_key(multiplier)?;
        Ok(key.private_key.secret_bytes())
    }

    /// Get the main secret key bytes
    pub fn get_main_secret_key(&self) -> Result<[u8; 32]> {
        let key = self.get_main_key()?;
        Ok(key.private_key.secret_bytes())
    }
}
