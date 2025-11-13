use crate::client::SubscriptionEvent;
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
use ark_core::ArkAddress;
use bitcoin::Amount;
use bitcoin::OutPoint;
use sqlx::Pool;
use sqlx::Sqlite;
use std::sync::Arc;
use time;
use tokio::time::sleep;
use tokio::time::Duration;

#[derive(Debug, Clone)]
struct GameResult {
    multiplier: Multiplier,
    outpoint: OutPoint,
    sender_address: ArkAddress,
    sender: String,
    input_amount: u64,
    current_nonce: u64,
    rolled_number: i64,
    is_win: bool,
    payout_amount: Option<u64>,
}

pub struct TransactionProcessor {
    ark_client: Arc<ArkClient>,
    my_addresses: Vec<ArkAddress>,
    nonce_service: NonceService,
    db_pool: Pool<Sqlite>,
    broadcaster: SharedBroadcaster,
    max_payout_sats: u64,
    dust_amount: Amount,
}

impl TransactionProcessor {
    pub fn new(
        ark_client: Arc<ArkClient>,
        my_addresses: Vec<ArkAddress>,
        nonce_service: NonceService,
        db_pool: Pool<Sqlite>,
        broadcaster: SharedBroadcaster,
        max_payout_sats: u64,
        dust_amount: Amount,
    ) -> Self {
        Self {
            ark_client,
            my_addresses,
            nonce_service,
            db_pool,
            broadcaster,
            max_payout_sats,
            dust_amount
        }
    }

    pub async fn start_monitoring(&self) {
        tracing::info!("🔍 Starting transaction monitoring with subscriptions...");

        // Get all game addresses to subscribe to
        let game_addresses = self.ark_client.get_game_addresses();

        // Collect addresses for subscription
        let scripts: Vec<_> = game_addresses
            .iter()
            .map(|(_, _, address)| *address)
            .collect();

        tracing::info!("📡 Subscribing to {} game addresses", scripts.len());

        // Subscribe to all game address scripts
        let subscription_id = match self.ark_client.subscribe_to_scripts(scripts).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("🚨 CRITICAL: Failed to subscribe to game addresses: {}", e);
                tracing::error!("🚨 CRITICAL: Exiting service to force restart");
                std::process::exit(1);
            }
        };

        tracing::info!(
            "✅ Successfully subscribed to game addresses with ID: {}",
            subscription_id
        );

        // Get subscription stream and process events
        let stream = match self.ark_client.get_subscription(subscription_id).await {
            Ok(stream) => stream,
            Err(e) => {
                tracing::error!("🚨 CRITICAL: Failed to get subscription stream: {}", e);
                tracing::error!("🚨 CRITICAL: Exiting service to force restart");
                std::process::exit(1);
            }
        };

        // Process the stream - if this returns, the stream has ended
        self.process_subscription_stream(stream).await;

        // If we reach this point, the subscription stream has ended unexpectedly
        tracing::error!("🚨 CRITICAL: Subscription stream ended unexpectedly");
        tracing::error!("🚨 CRITICAL: Exiting service to force restart");
        std::process::exit(1);
    }

    async fn process_subscription_stream(
        &self,
        mut stream: std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<SubscriptionEvent>> + Send + '_>,
        >,
    ) {
        use futures::StreamExt;

        tracing::info!("🔄 Processing subscription stream...");

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    if let Err(e) = self.process_single_event(event).await {
                        tracing::error!("Error processing subscription event: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Error in subscription stream: {}", e);
                    // Add a delay before continuing to avoid tight error loops
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        tracing::info!("📡 Subscription stream processing completed");
    }

    async fn process_single_event(&self, event: SubscriptionEvent) -> Result<()> {
        let tx_id = event.txid.to_string();
        tracing::info!(tx_id, ?event, "📨 Received subscription event for tx",);

        // Check if this is our own transaction
        let is_own_tx = db::is_own_transaction(&self.db_pool, &tx_id).await;
        let is_tx_processed = db::is_transaction_processed(&self.db_pool, &tx_id).await;

        match (is_tx_processed, is_own_tx) {
            (Ok(false), Ok(false)) => {
                tracing::trace!(target: "tx_processor", tx_id, "Processing new subscription event");

                // Find which game address this transaction is for
                if let Some((game_type, multiplier)) =
                    self.find_game_for_script(&event.script_pubkey, event.amount)
                {
                    if let Some(game_result) =
                        self.evaluate_game(game_type, &multiplier, &event).await?
                    {
                        // Process individual events immediately (no batching for now)
                        match game_result {
                            result
                                if result.payout_amount.is_none()
                                    && result.input_amount
                                        > self.get_donation_threshold(&result.multiplier) =>
                            {
                                self.process_donation(result).await?;
                            }
                            result if result.is_win => {
                                // For individual winners, use individual payout method
                                self.process_individual_winner(result).await?;
                            }
                            result => {
                                self.process_loser(result).await?;
                            }
                        }
                    }
                } else {
                    tracing::warn!("⚠️ Received event for unknown script pubkey");
                }
            }
            (Ok(true), _) => {
                tracing::trace!(target: "tx_processor", tx_id, "Transaction already processed, skipping");
            }
            (_, Ok(true)) => {
                tracing::trace!(target: "tx_processor", tx_id, "Own transaction, skipping");
            }
            (Err(e), Ok(_)) => {
                tracing::error!(tx_id, "Error checking if transaction is processed: {}", e);
            }
            (_, Err(e)) => {
                tracing::error!(tx_id, "Error checking if transaction is own: {}", e);
            }
        }

        Ok(())
    }

    /// Find which game corresponds to a script pubkey
    fn find_game_for_script(
        &self,
        script_pubkey: &bitcoin::ScriptBuf,
        amount: Amount,
    ) -> Option<(GameType, Multiplier)> {
        let game_addresses = self.ark_client.get_game_addresses();

        for (game_type, multiplier, address) in game_addresses {
            if amount <= self.dust_amount {
                if address.to_sub_dust_script_pubkey() == *script_pubkey {
                    return Some((game_type, multiplier));
                }
            }

            if address.to_p2tr_script_pubkey() == *script_pubkey {
                return Some((game_type, multiplier));
            }
        }

        None
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
        event: &SubscriptionEvent,
    ) -> Result<Option<GameResult>> {
        let out_point = OutPoint {
            txid: event.txid,
            vout: event.vout,
        };
        let ark_addresses = self.ark_client.get_parent_vtxo(out_point).await?;
        let own_address = self
            .my_addresses
            .first()
            .cloned()
            .expect("to have own address");

        for sender_address in ark_addresses {
            if sender_address.encode() == own_address.encode() {
                tracing::debug!(
                    outpoint = ?event.txid,
                    amount = ?event.amount,
                    own_address = sender_address.encode(),
                    "Ignoring own address"
                );
                continue;
            }

            let sender = sender_address.encode();
            let current_nonce = self.nonce_service.get_current_nonce().await;
            let input_amount = event.amount.to_sat();

            tracing::info!(outpoint = ?event.txid, amount = ?event.amount, sender, "Found sender");

            // Check donation threshold
            let donation_threshold = self.get_donation_threshold(multiplier);
            if input_amount > donation_threshold {
                return Ok(Some(GameResult {
                    multiplier: *multiplier,
                    outpoint: out_point,
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
            let evaluation = game.evaluate(current_nonce, &out_point.txid.to_string(), multiplier);

            let payout_amount = if evaluation.is_win {
                Some(
                    (input_amount as f64
                        * evaluation
                            .payout_multiplier
                            .expect("to have a payout multiplier")) as u64,
                )
            } else {
                None
            };

            return Ok(Some(GameResult {
                multiplier: *multiplier,
                outpoint: out_point,
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
            &donation.outpoint.txid.to_string(),
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
                id: format!("donation-{}", donation.outpoint.txid),
                amount: Amount::from_sat(donation.input_amount),
                sender: donation.sender,
                input_tx_id: donation.outpoint.txid.to_string(),
                timestamp: time::OffsetDateTime::now_utc(),
            };

            self.broadcast_donation(donation_item).await;
        }

        Ok(())
    }

    async fn process_individual_winner(&self, winner: GameResult) -> Result<()> {
        let payout_sats = winner.payout_amount.unwrap_or(0);
        let payout_amount = Amount::from_sat(payout_sats);

        tracing::info!(
            payout = payout_sats,
            sender = winner.sender,
            "💸 Processing individual payout"
        );

        const MAX_RETRIES: u8 = 3;
        let mut retry_count = 0;

        loop {
            match self
                .ark_client
                .send_vtxo(winner.sender_address, payout_amount)
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
                        error = ?e,
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
            &winner.outpoint.txid.to_string(),
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
                input_tx_id: winner.outpoint.txid.to_string(),
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
            &loser.outpoint.txid.to_string(),
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
                input_tx_id: loser.outpoint.txid.to_string(),
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
    nonce_service: NonceService,
    db_pool: Pool<Sqlite>,
    broadcaster: SharedBroadcaster,
    max_payout_sats: u64,
    dust_amount: Amount,
) {
    let processor = TransactionProcessor::new(
        ark_client,
        my_addresses,
        nonce_service,
        db_pool,
        broadcaster,
        max_payout_sats,
        dust_amount
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

#[allow(clippy::print_stdout)]
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
        println!("\n=== {multiplier_name} ({multiplier_value}x) ===");
        println!("Iterations: {}", stats["total"]);
        println!("Wins: {} | Losses: {}", stats["wins"], stats["losses"]);
        println!("Expected win rate: {expected_win_rate:.2}%",);
        println!("Actual win rate: {actual_win_rate:.2}%",);
        println!(
            "Deviation: {:.2}%",
            (actual_win_rate - expected_win_rate).abs()
        );

        let house_edge = 100.0 - (expected_win_rate * multiplier_value);
        println!("House edge: {house_edge:.2}%",);

        // Calculate profit/loss for 1000 sats per bet
        let bet_amount = 1000i64;
        let total_wagered = bet_amount * stats["total"] as i64;
        let win_payout = (bet_amount as f64 * multiplier_value) as i64;
        let player_return = stats["wins"] as i64 * win_payout;
        let player_profit = player_return - total_wagered;
        let house_profit = -player_profit;

        println!("📊 If player bet 1000 sats per game:");
        println!("   Total wagered: {total_wagered} sats",);
        println!(
            "   Player would have: {} sats ({})",
            if player_profit >= 0 {
                format!("+{player_profit}",)
            } else {
                player_profit.to_string()
            },
            if player_profit >= 0 { "profit" } else { "loss" }
        );
        println!(
            "   House would have: {} sats ({})",
            if house_profit >= 0 {
                format!("+{house_profit}",)
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
