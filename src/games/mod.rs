pub mod satoshis_number;

use crate::key_derivation::Multiplier;
use std::fmt;
use std::fmt::Formatter;

/// Result of evaluating a game
#[derive(Debug, Clone)]
pub struct GameEvaluation {
    /// The number/value that was rolled/generated
    pub rolled_value: i64,
    /// Whether the player won
    pub is_win: bool,
    /// The payout multiplier if they won
    pub payout_multiplier: Option<f64>,
}

/// Trait that all games must implement
pub trait Game: Send + Sync {
    /// Evaluate the game outcome based on inputs
    fn evaluate(&self, nonce: u64, txid: &str, multiplier: &Multiplier) -> GameEvaluation;

    /// Get the game name
    fn name(&self) -> &'static str;

    /// Get a description of the game rules
    fn description(&self) -> &'static str;
}

/// Factory function to get a game by type
pub fn get_game(game_type: GameType) -> Box<dyn Game> {
    match game_type {
        GameType::SatoshisNumber => Box::new(satoshis_number::SatoshisNumberGame),
    }
}

/// Enum of available game types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameType {
    SatoshisNumber,
    // Future games can be added here
    // HighLow,
    // DiceRoll,
    // CoinFlip,
}

impl Default for GameType {
    fn default() -> Self {
        GameType::SatoshisNumber
    }
}

impl fmt::Display for GameType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            GameType::SatoshisNumber => write!(f, "satoshis-number"),
        }
    }
}
