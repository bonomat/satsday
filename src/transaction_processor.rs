use crate::db;
use crate::games::get_game;
use crate::games::GameType;
use crate::key_derivation::Multiplier;
use crate::nonce_service::NonceService;
use crate::server::DonationItem;
use crate::server::GameHistoryItem;
use crate::websocket::SharedBroadcaster;
use crate::ArkClient;
use anyhow::Result;
use ark_core::server::VirtualTxOutPoint;
use ark_core::ArkAddress;
use bitcoin::Amount;
use sqlx::Pool;
use sqlx::Sqlite;
use std::sync::Arc;
use time;
use tokio::time::sleep;
use tokio::time::Duration;

#[derive(Debug, Clone)]
struct GameResult {
    multiplier: Multiplier,
    outpoint: VirtualTxOutPoint,
    sender_address: ArkAddress,
    sender: String,
    input_amount: u64,
    current_nonce: u64,
    rolled_number: i64,
    is_win: bool,
    payout_amount: Option<u64>,
}

#[derive(Debug)]
struct BatchProcessingResult {
    winners: Vec<GameResult>,
    losers: Vec<GameResult>,
    donations: Vec<GameResult>,
}

pub struct TransactionProcessor {
    ark_client: Arc<ArkClient>,
    my_addresses: Vec<ArkAddress>,
    check_interval: Duration,
    nonce_service: NonceService,
    db_pool: Pool<Sqlite>,
    broadcaster: SharedBroadcaster,
    max_payout_sats: u64,
}

impl TransactionProcessor {
    pub fn new(
        ark_client: Arc<ArkClient>,
        my_addresses: Vec<ArkAddress>,
        check_interval_seconds: u64,
        nonce_service: NonceService,
        db_pool: Pool<Sqlite>,
        broadcaster: SharedBroadcaster,
        max_payout_sats: u64,
    ) -> Self {
        Self {
            ark_client,
            my_addresses,
            check_interval: Duration::from_secs(check_interval_seconds),
            nonce_service,
            db_pool,
            broadcaster,
            max_payout_sats,
        }
    }

    pub async fn start_monitoring(&self) {
        tracing::info!("🔍 Starting transaction monitoring loop...");

        loop {
            if let Err(e) = self.check_for_new_transactions().await {
                tracing::error!("Error checking for new transactions: {}", e);
            }

            sleep(self.check_interval).await;
        }
    }

    async fn check_for_new_transactions(&self) -> Result<()> {
        tracing::info!("Checking for new spendable VTXOs...");

        let spendable_vtxos = self.ark_client.spendable_game_vtxos(true).await?;
        let total_vtxo: usize = spendable_vtxos.values().map(|v| v.len()).sum();
        tracing::info!(total_vtxo, "Found spendable vtxos");

        let mut batch_result = BatchProcessingResult {
            winners: Vec::new(),
            losers: Vec::new(),
            donations: Vec::new(),
        };

        // First pass: collect all game results
        for ((game_type, multiplier), outpoints) in &spendable_vtxos {
            for outpoint in outpoints {
                let tx_id = outpoint.outpoint.txid.to_string();

                // Check if this is our own transaction
                let is_own_tx = db::is_own_transaction(&self.db_pool, &tx_id).await;
                let is_tx_processed = db::is_transaction_processed(&self.db_pool, &tx_id).await;

                match (is_tx_processed, is_own_tx) {
                    (Ok(false), Ok(false)) => {
                        tracing::trace!(target : "tx_processor", tx_id, "Processing new transaction");

                        if let Some(game_result) =
                            self.evaluate_game(*game_type, multiplier, outpoint).await?
                        {
                            match game_result {
                                result
                                    if result.payout_amount.is_none()
                                        && result.input_amount
                                            > self.get_donation_threshold(&result.multiplier) =>
                                {
                                    batch_result.donations.push(result);
                                }
                                result if result.is_win => {
                                    batch_result.winners.push(result);
                                }
                                result => {
                                    batch_result.losers.push(result);
                                }
                            }
                        }
                    }
                    (Ok(true), _) => {
                        tracing::trace!(target : "tx_processor", tx_id, "Transaction already processed, skipping");
                        continue;
                    }
                    (_, Ok(true)) => {
                        tracing::trace!(target : "tx_processor", tx_id, "Own transaction, skipping");
                        continue;
                    }
                    (Err(e), Ok(_)) => {
                        tracing::error!(tx_id, "Error checking if transaction is processed: {}", e);
                    }
                    (_, Err(e)) => {
                        tracing::error!(tx_id, "Error checking if transaction is own: {}", e);
                    }
                }
            }
        }

        // Second pass: process results in batches
        self.process_batch_results(batch_result).await?;

        Ok(())
    }

    async fn broadcast_game_result(&self, game: GameHistoryItem) {
        let broadcaster = self.broadcaster.read().await;
        if let Err(e) = broadcaster.broadcast_game_result(game) {
            tracing::error!("Failed to broadcast game result: {}", e);
        }
    }

    async fn broadcast_donation(&self, donation: DonationItem) {
        let broadcaster = self.broadcaster.read().await;
        if let Err(e) = broadcaster.broadcast_donation(donation) {
            tracing::error!("Failed to broadcast donation: {}", e);
        }
    }

    fn get_donation_threshold(&self, multiplier: &Multiplier) -> u64 {
        // Calculate max input amount: max_payout * 100 / multiplier
        (self.max_payout_sats * 100) / multiplier.multiplier()
    }

    async fn evaluate_game(
        &self,
        game_type: GameType,
        multiplier: &Multiplier,
        outpoint: &VirtualTxOutPoint,
    ) -> Result<Option<GameResult>> {
        let ark_addresses = self.ark_client.get_parent_vtxo(outpoint.outpoint).await?;
        let own_address = self
            .my_addresses
            .first()
            .cloned()
            .expect("to have own address");

        for sender_address in ark_addresses {
            if sender_address.encode() == own_address.encode() {
                tracing::debug!(
                    outpoint = ?outpoint.outpoint.txid,
                    amount = ?outpoint.amount,
                    own_address = sender_address.encode(),
                    "Ignoring own address"
                );
                continue;
            }

            let sender = sender_address.encode();
            let current_nonce = self.nonce_service.get_current_nonce().await;
            let input_amount = outpoint.amount.to_sat();

            tracing::info!(outpoint = ?outpoint.outpoint.txid, amount = ?outpoint.amount, sender, "Found sender");

            // Check donation threshold
            let donation_threshold = self.get_donation_threshold(multiplier);
            if input_amount > donation_threshold {
                return Ok(Some(GameResult {
                    multiplier: *multiplier,
                    outpoint: outpoint.clone(),
                    sender_address,
                    sender,
                    input_amount,
                    current_nonce,
                    rolled_number: -1, // Special value for donations
                    is_win: false,
                    payout_amount: None,
                }));
            }

            // Game logic - using the abstracted game system
            let game = get_game(game_type);
            let evaluation = game.evaluate(
                current_nonce,
                &outpoint.outpoint.txid.to_string(),
                multiplier,
            );

            let payout_amount = if evaluation.is_win {
                Some((input_amount as f64 * evaluation.payout_multiplier.unwrap()) as u64)
            } else {
                None
            };

            return Ok(Some(GameResult {
                multiplier: *multiplier,
                outpoint: outpoint.clone(),
                sender_address,
                sender,
                input_amount,
                current_nonce,
                rolled_number: evaluation.rolled_value,
                is_win: evaluation.is_win,
                payout_amount,
            }));
        }

        Ok(None)
    }

    async fn process_batch_results(&self, batch_result: BatchProcessingResult) -> Result<()> {
        let BatchProcessingResult {
            winners,
            losers,
            donations,
        } = batch_result;

        tracing::info!(
            winners = winners.len(),
            losers = losers.len(),
            donations = donations.len(),
            "Processing batch results"
        );

        // Process donations first (no payout needed)
        for donation in donations {
            self.process_donation(donation).await?;
        }

        // Process batch payouts for winners
        if !winners.is_empty() {
            self.process_batch_winners(winners).await?;
        }

        // Process losers (store results, no payout)
        for loser in losers {
            self.process_loser(loser).await?;
        }

        Ok(())
    }

    async fn process_donation(&self, donation: GameResult) -> Result<()> {
        tracing::info!(
            input_amount = donation.input_amount,
            sender = donation.sender,
            "💝 Processing donation"
        );

        // Store as donation in database
        if let Err(e) = db::insert_game_result(
            &self.db_pool,
            &donation.current_nonce.to_string(),
            donation.rolled_number,
            &donation.outpoint.outpoint.txid.to_string(),
            None,
            donation.input_amount as i64,
            None,
            &donation.sender,
            false, // Not a win
            false, // Not processed as game
            donation.multiplier.multiplier() as i64,
        )
        .await
        {
            tracing::error!("Failed to store donation: {}", e);
        } else {
            // Broadcast donation notification
            let donation_item = DonationItem {
                id: format!("donation-{}", donation.outpoint.outpoint.txid),
                amount: Amount::from_sat(donation.input_amount),
                sender: donation.sender,
                input_tx_id: donation.outpoint.outpoint.txid.to_string(),
                timestamp: time::OffsetDateTime::now_utc(),
            };

            self.broadcast_donation(donation_item).await;
        }

        Ok(())
    }

    async fn process_batch_winners(&self, winners: Vec<GameResult>) -> Result<()> {
        if winners.is_empty() {
            return Ok(());
        }

        let dust_value = self.ark_client.dust_value();
        let total_payout: u64 = winners.iter().map(|w| w.payout_amount.unwrap_or(0)).sum();

        tracing::info!(
            winner_count = winners.len(),
            total_payout = total_payout,
            dust_threshold = dust_value.to_sat(),
            "🎉 Processing batch payouts"
        );

        // Separate winners by payout amount (dust vs regular)
        let (dust_winners, regular_winners): (Vec<_>, Vec<_>) =
            winners.into_iter().partition(|winner| {
                let payout_amount = winner.payout_amount.unwrap_or(0);
                payout_amount < dust_value.to_sat()
            });

        // Process dust payouts individually
        for winner in dust_winners {
            self.process_individual_winner(winner).await?;
        }

        // Process regular payouts as batch if any exist
        if !regular_winners.is_empty() {
            self.process_regular_batch_winners(regular_winners).await?;
        }

        Ok(())
    }

    async fn process_individual_winner(&self, winner: GameResult) -> Result<()> {
        let payout_sats = winner.payout_amount.unwrap_or(0);
        let payout_amount = Amount::from_sat(payout_sats);

        tracing::info!(
            payout = payout_sats,
            sender = winner.sender,
            "💸 Processing individual dust payout"
        );

        const MAX_RETRIES: u8 = 3;
        let mut retry_count = 0;

        loop {
            match self
                .ark_client
                .send(vec![(&winner.sender_address, payout_amount)])
                .await
            {
                Ok(txid) => {
                    tracing::info!(
                        txid = txid.to_string(),
                        payout = payout_sats,
                        "💸 Individual payout sent successfully"
                    );

                    // Store as our own transaction
                    if let Err(e) = db::insert_own_transaction(
                        &self.db_pool,
                        &txid.to_string(),
                        "individual_payout",
                    )
                    .await
                    {
                        tracing::error!("Failed to store individual payout transaction: {}", e);
                    }

                    // Process winner result
                    self.process_winner_result(winner, Some(txid.to_string()))
                        .await?;
                    break;
                }
                Err(e) => {
                    retry_count += 1;
                    tracing::error!(
                        retry = retry_count,
                        max_retries = MAX_RETRIES,
                        error = %e,
                        payout = payout_sats,
                        "🚨 Failed to send individual payout"
                    );

                    if retry_count >= MAX_RETRIES {
                        tracing::error!(
                            "🚨 Max retries exceeded for individual payout, processing as failed winner"
                        );
                        self.process_winner_result(winner, None).await?;
                        break;
                    } else {
                        // Wait before retrying (exponential backoff)
                        let delay_ms = 1000 * (2_u64.pow(retry_count as u32 - 1));
                        tracing::info!("Retrying individual payout in {}ms...", delay_ms);
                        sleep(Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_regular_batch_winners(&self, winners: Vec<GameResult>) -> Result<()> {
        // Prepare batch payment data for regular (non-dust) payouts
        let payout_data: Vec<_> = winners
            .iter()
            .map(|winner| {
                let payout_sats = winner.payout_amount.unwrap_or(0);
                (&winner.sender_address, Amount::from_sat(payout_sats))
            })
            .collect();

        let total_payout: u64 = winners.iter().map(|w| w.payout_amount.unwrap_or(0)).sum();

        tracing::info!(
            winner_count = winners.len(),
            total_payout = total_payout,
            "🎉 Processing regular batch payouts"
        );

        const MAX_RETRIES: u8 = 3;
        let mut retry_count = 0;

        loop {
            match self.ark_client.send(payout_data.clone()).await {
                Ok(txid) => {
                    tracing::info!(txid = txid.to_string(), "🎉 Batch payout sent successfully");

                    // Store as our own transaction
                    if let Err(e) =
                        db::insert_own_transaction(&self.db_pool, &txid.to_string(), "batch_payout")
                            .await
                    {
                        tracing::error!("Failed to store batch payout transaction: {}", e);
                    }

                    // Process each winner individually for database and notifications
                    for winner in winners {
                        self.process_winner_result(winner, Some(txid.to_string()))
                            .await?;
                    }
                    break;
                }
                Err(e) => {
                    retry_count += 1;
                    tracing::error!(
                        retry = retry_count,
                        max_retries = MAX_RETRIES,
                        error = %e,
                        "🚨 Failed to send batch payout"
                    );

                    if retry_count >= MAX_RETRIES {
                        tracing::error!(
                            "🚨 Max retries exceeded for batch payout, processing as failed winners"
                        );
                        for winner in winners {
                            self.process_winner_result(winner, None).await?;
                        }
                        break;
                    } else {
                        // Wait before retrying (exponential backoff)
                        let delay_ms = 1000 * (2_u64.pow(retry_count as u32 - 1));
                        tracing::info!("Retrying batch payout in {}ms...", delay_ms);
                        sleep(Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_winner_result(
        &self,
        winner: GameResult,
        payout_txid: Option<String>,
    ) -> Result<()> {
        // Store game result in database
        let game_result = db::insert_game_result(
            &self.db_pool,
            &winner.current_nonce.to_string(),
            winner.rolled_number,
            &winner.outpoint.outpoint.txid.to_string(),
            payout_txid.as_deref(),
            winner.input_amount as i64,
            winner.payout_amount.map(|p| p as i64),
            &winner.sender,
            true,                  // Is win
            payout_txid.is_some(), // Processed successfully if payout_txid exists
            winner.multiplier.multiplier() as i64,
        )
        .await;

        if let Err(e) = game_result {
            tracing::error!("Failed to store winner game result: {}", e);
        } else {
            // Broadcast game result
            let nonce_str = winner.current_nonce.to_string();
            let revealable_nonce = self.nonce_service.get_revealable_nonce(&nonce_str).await;
            let nonce_hash = self.nonce_service.get_current_nonce_hash().await;

            let game_item = GameHistoryItem {
                id: "latest".to_string(),
                amount_sent: Amount::from_sat(winner.input_amount),
                multiplier: winner.multiplier.multiplier() as f64 / 100.0,
                result_number: winner.rolled_number,
                target_number: (65536.0 * 1000.0 / winner.multiplier.multiplier() as f64) as i64,
                is_win: true,
                payout: winner.payout_amount.map(Amount::from_sat),
                input_tx_id: winner.outpoint.outpoint.txid.to_string(),
                output_tx_id: payout_txid,
                nonce: revealable_nonce,
                nonce_hash,
                timestamp: time::OffsetDateTime::now_utc(),
            };

            self.broadcast_game_result(game_item).await;
        }

        Ok(())
    }

    async fn process_loser(&self, loser: GameResult) -> Result<()> {
        tracing::info!(
            rolled_number = loser.rolled_number,
            bet = loser.input_amount,
            nonce = loser.current_nonce,
            "🏠 House won! Player lost their bet"
        );

        // Store losing game result
        let game_result = db::insert_game_result(
            &self.db_pool,
            &loser.current_nonce.to_string(),
            loser.rolled_number,
            &loser.outpoint.outpoint.txid.to_string(),
            None,
            loser.input_amount as i64,
            None,
            &loser.sender,
            false, // Not a win
            true,  // Processed (no payment needed for losses)
            loser.multiplier.multiplier() as i64,
        )
        .await;

        if let Err(e) = game_result {
            tracing::error!("Failed to store loser game result: {}", e);
        } else {
            // Broadcast game result
            let nonce_str = loser.current_nonce.to_string();
            let revealable_nonce = self.nonce_service.get_revealable_nonce(&nonce_str).await;
            let nonce_hash = self.nonce_service.get_current_nonce_hash().await;

            let game_item = GameHistoryItem {
                id: "latest".to_string(),
                amount_sent: Amount::from_sat(loser.input_amount),
                multiplier: loser.multiplier.multiplier() as f64 / 100.0,
                result_number: loser.rolled_number,
                target_number: (65536.0 * 1000.0 / loser.multiplier.multiplier() as f64) as i64,
                is_win: false,
                payout: None,
                input_tx_id: loser.outpoint.outpoint.txid.to_string(),
                output_tx_id: None,
                nonce: revealable_nonce,
                nonce_hash,
                timestamp: time::OffsetDateTime::now_utc(),
            };

            self.broadcast_game_result(game_item).await;
        }

        Ok(())
    }
}

pub async fn spawn_transaction_monitor(
    ark_client: Arc<ArkClient>,
    my_addresses: Vec<ArkAddress>,
    check_interval_seconds: u64,
    nonce_service: NonceService,
    db_pool: Pool<Sqlite>,
    broadcaster: SharedBroadcaster,
    max_payout_sats: u64,
) {
    let processor = TransactionProcessor::new(
        ark_client,
        my_addresses,
        check_interval_seconds,
        nonce_service,
        db_pool,
        broadcaster,
        max_payout_sats,
    );

    tokio::spawn(async move {
        processor.start_monitoring().await;
    });
}

/// Legacy function for backward compatibility
/// Game evaluation logic has been moved to the games module
#[deprecated(note = "Use games::get_game(GameType::SatoshisNumber).evaluate() instead")]
pub fn evaluate_game_outcome(nonce: u64, txid: &str, multiplier: &Multiplier) -> (i64, bool) {
    let game = get_game(GameType::SatoshisNumber);
    let evaluation = game.evaluate(nonce, txid, multiplier);
    (evaluation.rolled_value, evaluation.is_win)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_derivation::Multiplier;
    use rayon::prelude::*;
    use std::collections::HashMap;

    const TEST_ITERATIONS: usize = 1000;

    fn run_multiplier_test(multiplier: Multiplier) -> (f64, f64, HashMap<&'static str, usize>) {
        let results: Vec<bool> = (0..TEST_ITERATIONS)
            .into_par_iter()
            .map(|i| {
                let game = get_game(GameType::SatoshisNumber);
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

    fn print_test_results(
        multiplier_name: &str,
        multiplier_value: f64,
        actual_win_rate: f64,
        expected_win_rate: f64,
        stats: HashMap<&'static str, usize>,
    ) {
        println!("\n=== {} ({}x) ===", multiplier_name, multiplier_value);
        println!("Iterations: {}", stats["total"]);
        println!("Wins: {} | Losses: {}", stats["wins"], stats["losses"]);
        println!("Expected win rate: {:.2}%", expected_win_rate);
        println!("Actual win rate: {:.2}%", actual_win_rate);
        println!(
            "Deviation: {:.2}%",
            (actual_win_rate - expected_win_rate).abs()
        );

        let house_edge = 100.0 - (expected_win_rate * multiplier_value);
        println!("House edge: {:.2}%", house_edge);

        // Calculate profit/loss for 1000 sats per bet
        let bet_amount = 1000i64;
        let total_wagered = bet_amount * stats["total"] as i64;
        let win_payout = (bet_amount as f64 * multiplier_value) as i64;
        let player_return = stats["wins"] as i64 * win_payout;
        let player_profit = player_return - total_wagered;
        let house_profit = -player_profit;

        println!("📊 If player bet 1000 sats per game:");
        println!("   Total wagered: {} sats", total_wagered);
        println!(
            "   Player would have: {} sats ({})",
            if player_profit >= 0 {
                format!("+{}", player_profit)
            } else {
                player_profit.to_string()
            },
            if player_profit >= 0 { "profit" } else { "loss" }
        );
        println!(
            "   House would have: {} sats ({})",
            if house_profit >= 0 {
                format!("+{}", house_profit)
            } else {
                house_profit.to_string()
            },
            if house_profit >= 0 { "profit" } else { "loss" }
        );
    }

    #[test]
    fn test_x105_multiplier() {
        let multiplier = Multiplier::X105;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X105", 1.05, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x110_multiplier() {
        let multiplier = Multiplier::X110;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X110", 1.10, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x133_multiplier() {
        let multiplier = Multiplier::X133;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X133", 1.33, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x150_multiplier() {
        let multiplier = Multiplier::X150;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X150", 1.50, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x200_multiplier() {
        let multiplier = Multiplier::X200;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X200", 2.00, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x300_multiplier() {
        let multiplier = Multiplier::X300;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X300", 3.00, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x1000_multiplier() {
        let multiplier = Multiplier::X1000;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X1000", 10.00, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x2500_multiplier() {
        let multiplier = Multiplier::X2500;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X2500", 25.00, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x5000_multiplier() {
        let multiplier = Multiplier::X5000;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X5000", 50.00, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x10000_multiplier() {
        let multiplier = Multiplier::X10000;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X10000", 100.00, actual, expected, stats);
        assert!(
            (actual - expected).abs() < 3.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_x100000_multiplier() {
        let multiplier = Multiplier::X100000;
        let (actual, expected, stats) = run_multiplier_test(multiplier);
        print_test_results("X100000", 1000.00, actual, expected, stats);
        // Allow higher deviation for very low probability events
        assert!(
            (actual - expected).abs() < 5.0,
            "Win rate deviation too high"
        );
    }

    #[test]
    fn test_all_multipliers_summary() {
        println!("\n========================================");
        println!("COMPREHENSIVE MULTIPLIER TEST SUMMARY");
        println!("========================================");

        let multipliers = vec![
            (Multiplier::X105, "X105", 1.05),
            (Multiplier::X110, "X110", 1.10),
            (Multiplier::X133, "X133", 1.33),
            (Multiplier::X150, "X150", 1.50),
            (Multiplier::X200, "X200", 2.00),
            (Multiplier::X300, "X300", 3.00),
            (Multiplier::X1000, "X1000", 10.00),
            (Multiplier::X2500, "X2500", 25.00),
            (Multiplier::X5000, "X5000", 50.00),
            (Multiplier::X10000, "X10000", 100.00),
            (Multiplier::X100000, "X100000", 1000.00),
        ];

        for (mult, name, value) in multipliers {
            let (actual, expected, stats) = run_multiplier_test(mult);
            print_test_results(name, value, actual, expected, stats);
        }
    }
}
