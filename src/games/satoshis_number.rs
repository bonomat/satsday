use super::Game;
use super::GameEvaluation;
use crate::key_derivation::Multiplier;
use bitcoin::hashes::Hash;

/// The original Satoshi's Number game
/// Players bet on whether a hash-derived number will be below a threshold
pub struct SatoshisNumberGame;

impl Game for SatoshisNumberGame {
    fn evaluate(&self, nonce: u64, txid: &str, multiplier: &Multiplier) -> GameEvaluation {
        // Hash nonce + txid to get randomness
        let hash_input = format!("{nonce}{txid}");
        let hash = bitcoin::hashes::sha256::Hash::hash(hash_input.as_bytes());
        let hash_bytes = hash.as_byte_array();

        // Use first 2 bytes as u16 for randomness (0-65535 range)
        let random_value = u16::from_be_bytes([hash_bytes[0], hash_bytes[1]]);
        let rolled_number = random_value as i64;
        let player_wins = multiplier.is_win(random_value);

        GameEvaluation {
            rolled_value: rolled_number,
            is_win: player_wins,
            payout_multiplier: if player_wins {
                Some(multiplier.multiplier() as f64 / 100.0)
            } else {
                None
            },
        }
    }

    fn name(&self) -> &'static str {
        "Satoshi's Number"
    }

    fn description(&self) -> &'static str {
        "Guess if the hash-derived number will be below the target threshold. \
         The lower the threshold, the higher the payout multiplier."
    }
}

#[allow(clippy::print_stdout)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_derivation::Multiplier;
    use rayon::prelude::*;
    use std::collections::HashMap;

    const TEST_ITERATIONS: usize = 1000;

    fn run_multiplier_test(multiplier: Multiplier) -> (f64, f64, HashMap<&'static str, usize>) {
        let game = SatoshisNumberGame;

        let results: Vec<bool> = (0..TEST_ITERATIONS)
            .into_par_iter()
            .map(|i| {
                let nonce = i as u64;
                let txid = format!("test_txid_{i}");
                let evaluation = game.evaluate(nonce, &txid, &multiplier);
                evaluation.is_win
            })
            .collect();

        let wins = results.iter().filter(|&&x| x).count();
        let losses = results.iter().filter(|&&x| !x).count();

        let actual_win_rate = (wins as f64 / TEST_ITERATIONS as f64) * 100.0;
        let expected_win_rate = (multiplier.get_lower_than() as f64 / 65536.0) * 100.0;

        let mut stats = HashMap::new();
        stats.insert("wins", wins);
        stats.insert("losses", losses);
        stats.insert("total", TEST_ITERATIONS);

        (actual_win_rate, expected_win_rate, stats)
    }

    #[test]
    fn test_x200_multiplier() {
        let multiplier = Multiplier::X200;
        let (actual, expected, stats) = run_multiplier_test(multiplier);

        println!("X200 Test Results:");
        println!("Wins: {} | Losses: {}", stats["wins"], stats["losses"]);
        println!("Expected win rate: {expected:.2}%",);
        println!("Actual win rate: {actual:.2}%",);

        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_game_evaluation() {
        let game = SatoshisNumberGame;
        let evaluation = game.evaluate(12345, "test_tx", &Multiplier::X200);

        // Check that evaluation produces expected fields
        assert!(evaluation.rolled_value >= 0 && evaluation.rolled_value <= 65535);
        if evaluation.is_win {
            assert!(evaluation.payout_multiplier.is_some());
            assert_eq!(evaluation.payout_multiplier.unwrap(), 2.0);
        } else {
            assert!(evaluation.payout_multiplier.is_none());
        }
    }
}
