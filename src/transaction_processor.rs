use crate::db;
use crate::key_derivation::Multiplier;
use crate::nonce_service::NonceService;
use crate::server::DonationItem;
use crate::server::GameHistoryItem;
use crate::websocket::SharedBroadcaster;
use crate::ArkClient;
use anyhow::Result;
use ark_core::server::VirtualTxOutPoint;
use ark_core::ArkAddress;
use bitcoin::hashes::Hash;
use bitcoin::Amount;
use sqlx::Pool;
use sqlx::Sqlite;
use std::sync::Arc;
use time;
use tokio::time::sleep;
use tokio::time::Duration;

pub struct TransactionProcessor {
    ark_client: Arc<ArkClient>,
    my_addresses: Vec<ArkAddress>,
    check_interval: Duration,
    nonce_service: NonceService,
    db_pool: Pool<Sqlite>,
    broadcaster: SharedBroadcaster,
}

impl TransactionProcessor {
    pub fn new(
        ark_client: Arc<ArkClient>,
        my_addresses: Vec<ArkAddress>,
        check_interval_seconds: u64,
        nonce_service: NonceService,
        db_pool: Pool<Sqlite>,
        broadcaster: SharedBroadcaster,
    ) -> Self {
        Self {
            ark_client,
            my_addresses,
            check_interval: Duration::from_secs(check_interval_seconds),
            nonce_service,
            db_pool,
            broadcaster,
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

        for (multiplier, outpoints) in &spendable_vtxos {
            for outpoint in outpoints {
                let tx_id = outpoint.outpoint.txid.to_string();

                // Check if this is our own transaction
                let is_own_tx = db::is_own_transaction(&self.db_pool, &tx_id).await;
                let is_tx_processed = db::is_transaction_processed(&self.db_pool, &tx_id).await;

                match (is_tx_processed, is_own_tx) {
                    (Ok(false), Ok(false)) => {
                        tracing::debug!(tx_id, "Processing new transaction");

                        self.process_spendable_outpoint(multiplier, outpoint)
                            .await?;
                    }
                    (Ok(true), _) => {
                        tracing::debug!(tx_id, "Transaction already processed, skipping");
                        continue;
                    }
                    (_, Ok(true)) => {
                        tracing::debug!(tx_id, "Own transaction, skipping");
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

    async fn process_spendable_outpoint(
        &self,
        multiplier: &Multiplier,
        outpoint: &VirtualTxOutPoint,
    ) -> Result<()> {
        tracing::debug!(
            amount = ?outpoint.amount,
            outpoint = ?outpoint.outpoint.txid,
            "Processing spendable outpoint"
        );

        let ark_addresses = self.ark_client.get_parent_vtxo(outpoint.outpoint).await?;

        // Check if this transaction was sent to one of our addresses
        // TODO: there might be multiple in the future
        let own_address = self
            .my_addresses
            .first()
            .cloned()
            .expect("to have own address");

        for sender_address in ark_addresses {
            if sender_address.encode() == own_address.encode() {
                tracing::debug!(
                    outpoint = ?outpoint.outpoint.txid, amount = ?outpoint.amount,
                    own_address = sender_address.encode(),
                    "Ignoring own address");
                continue;
            }
            let sender = sender_address.encode();
            tracing::info!(outpoint = ?outpoint.outpoint.txid, amount = ?outpoint.amount, sender, "Found sender");

            // Dice game logic using current nonce
            let current_nonce = self.nonce_service.get_current_nonce().await;
            let input_amount = outpoint.amount.to_sat();

            // Check if potential payout exceeds maximum allowed
            let max_allowed_payout = std::env::var("MAX_PAYOUT_SATS")
                .unwrap_or_else(|_| "100000".to_string())
                .parse::<u64>()
                .unwrap_or(100_000u64);
            let potential_payout = (input_amount * multiplier.multiplier()) / 100;

            if potential_payout > max_allowed_payout {
                tracing::info!(
                    input_amount = input_amount,
                    potential_payout = potential_payout,
                    max_allowed = max_allowed_payout,
                    sender = sender,
                    "💝 Received donation - amount exceeds max payout limit"
                );

                // Store as donation in database
                if let Err(e) = db::insert_game_result(
                    &self.db_pool,
                    &current_nonce.to_string(),
                    -1, // Special value to indicate donation
                    &outpoint.outpoint.txid.to_string(),
                    None,
                    input_amount as i64,
                    None,
                    &sender,
                    false, // Not a win
                    false, // Not processed as game
                    multiplier.multiplier() as i64,
                )
                .await
                {
                    tracing::error!("Failed to store donation: {}", e);
                } else {
                    // Broadcast donation notification via websocket
                    let donation_item = DonationItem {
                        id: format!("donation-{}", outpoint.outpoint.txid),
                        amount: Amount::from_sat(input_amount),
                        sender: sender.clone(),
                        input_tx_id: outpoint.outpoint.txid.to_string(),
                        timestamp: time::OffsetDateTime::now_utc(),
                    };

                    self.broadcast_donation(donation_item).await;
                }

                continue; // Skip game processing for this transaction
            }

            // Simple dice game: hash nonce + outpoint txid
            let hash_input = format!("{}{}", current_nonce, outpoint.outpoint.txid);
            let hash = bitcoin::hashes::sha256::Hash::hash(hash_input.as_bytes());
            let hash_bytes = hash.as_byte_array();

            // Use first 2 bytes as u16 for randomness (0-65535 range)
            let random_value = u16::from_be_bytes([hash_bytes[0], hash_bytes[1]]);

            let rolled_number = random_value as i64;

            let player_wins = multiplier.is_win(random_value);
            let max_value = multiplier.get_lower_than();

            if player_wins {
                let payout = (input_amount * multiplier.multiplier()) / 100; //
                let payout_amount = Amount::from_sat(payout);

                tracing::info!(
                    max_value = max_value,
                    rolled_number = random_value,
                    bet_amount = input_amount,
                    payout = payout,
                    nonce = current_nonce,
                    "🎉 Player won! Sending payout...."
                );

                // TODO: we should send to all addresses at the same time
                match self.ark_client.send(vec![(&sender_address, payout_amount)]).await {
                    Ok(txid) => {
                        tracing::debug!(txid = txid.to_string(), "🎉 Player won! Sent payout");

                        // Store this as our own transaction
                        if let Err(e) =
                            db::insert_own_transaction(&self.db_pool, &txid.to_string(), "payout")
                                .await
                        {
                            tracing::error!("Failed to store own transaction: {}", e);
                        }

                        // Store successful winning game result
                        let game_result = db::insert_game_result(
                            &self.db_pool,
                            &current_nonce.to_string(),
                            rolled_number,
                            &outpoint.outpoint.txid.to_string(),
                            Some(&txid.to_string()),
                            input_amount as i64,
                            Some(payout as i64),
                            &sender,
                            true,
                            true,
                            multiplier.multiplier() as i64,
                        )
                        .await;

                        if let Err(e) = game_result {
                            tracing::error!("Failed to store game result: {}", e);
                        } else {
                            // Broadcast the game result
                            let nonce_str = current_nonce.to_string();
                            let revealable_nonce =
                                self.nonce_service.get_revealable_nonce(&nonce_str).await;
                            let nonce_hash = self.nonce_service.get_current_nonce_hash().await;

                            let game_item = GameHistoryItem {
                                id: "latest".to_string(), /* This will be replaced by actual ID
                                                           * from DB */
                                amount_sent: Amount::from_sat(input_amount),
                                multiplier: multiplier.multiplier() as f64 / 100.0,
                                result_number: rolled_number,
                                target_number: (65536.0 * 1000.0 / multiplier.multiplier() as f64)
                                    as i64,
                                is_win: true,
                                payout: Some(Amount::from_sat(payout)),
                                input_tx_id: outpoint.outpoint.txid.to_string(),
                                output_tx_id: Some(txid.to_string()),
                                nonce: revealable_nonce,
                                nonce_hash,
                                timestamp: time::OffsetDateTime::now_utc(),
                            };

                            self.broadcast_game_result(game_item).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!("🚨 Failed to send payout: {}", e);
                        // Don't store the game result so it can be retried later
                    }
                }
            } else {
                tracing::info!(
                    max_value = max_value,
                    rolled_number = random_value,
                    bet = input_amount,
                    nonce = current_nonce,
                    "🏠 House won! Player lost their bet"
                );

                // Store losing game result
                let game_result = db::insert_game_result(
                    &self.db_pool,
                    &current_nonce.to_string(),
                    rolled_number,
                    &outpoint.outpoint.txid.to_string(),
                    None,
                    input_amount as i64,
                    None,
                    &sender,
                    false,
                    true, // No payment needed for losses
                    multiplier.multiplier() as i64,
                )
                .await;

                if let Err(e) = game_result {
                    tracing::error!("Failed to store game result: {}", e);
                } else {
                    // Broadcast the game result
                    let nonce_str = current_nonce.to_string();
                    let revealable_nonce =
                        self.nonce_service.get_revealable_nonce(&nonce_str).await;
                    let nonce_hash = self.nonce_service.get_current_nonce_hash().await;

                    let game_item = GameHistoryItem {
                        id: "latest".to_string(), // This will be replaced by actual ID from DB
                        amount_sent: Amount::from_sat(input_amount),
                        multiplier: multiplier.multiplier() as f64 / 100.0,
                        result_number: rolled_number,
                        target_number: (65536.0 * 1000.0 / multiplier.multiplier() as f64) as i64,
                        is_win: false,
                        payout: None,
                        input_tx_id: outpoint.outpoint.txid.to_string(),
                        output_tx_id: None,
                        nonce: revealable_nonce,
                        nonce_hash,
                        timestamp: time::OffsetDateTime::now_utc(),
                    };

                    self.broadcast_game_result(game_item).await;
                }
            }
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
) {
    let processor = TransactionProcessor::new(
        ark_client,
        my_addresses,
        check_interval_seconds,
        nonce_service,
        db_pool,
        broadcaster,
    );

    tokio::spawn(async move {
        processor.start_monitoring().await;
    });
}
