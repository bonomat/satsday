use anyhow::Result;
use ark_core::server::VtxoOutPoint;
use ark_core::{ArkAddress, Vtxo};
use bitcoin::hashes::Hash;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

use crate::{ArkClient, db, nonce_service::NonceService};

pub struct TransactionProcessor {
    ark_client: Arc<ArkClient>,
    my_addresses: Vec<ArkAddress>,
    check_interval: Duration,
    nonce_service: NonceService,
    db_pool: Pool<Sqlite>,
}

impl TransactionProcessor {
    pub fn new(
        ark_client: Arc<ArkClient>,
        my_addresses: Vec<ArkAddress>,
        check_interval_seconds: u64,
        nonce_service: NonceService,
        db_pool: Pool<Sqlite>,
    ) -> Self {
        Self {
            ark_client,
            my_addresses,
            check_interval: Duration::from_secs(check_interval_seconds),
            nonce_service,
            db_pool,
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
        tracing::debug!("Checking for new spendable VTXOs...");

        let spendable_vtxos = self.ark_client.spendable_vtxos(false).await?;

        for (vtxo, outpoints) in &spendable_vtxos {
            for outpoint in outpoints {
                let tx_id = outpoint.outpoint.txid.to_string();

                // Check if this is our own transaction
                match db::is_own_transaction(&self.db_pool, &tx_id).await {
                    Ok(true) => {
                        tracing::debug!(tx_id, "Own transaction, skipping");
                        continue;
                    }
                    Ok(false) => {
                        // Not our own transaction, check if already processed
                        match db::is_transaction_processed(&self.db_pool, &tx_id).await {
                            Ok(true) => {
                                tracing::debug!(tx_id, "Transaction already processed, skipping");
                                continue;
                            }
                            Ok(false) => {
                                // Process the transaction
                                self.process_spendable_outpoint(vtxo, &outpoint).await?;
                            }
                            Err(e) => {
                                tracing::error!(
                                    tx_id,
                                    "Error checking if transaction is processed: {}",
                                    e
                                );
                                // Continue processing in case of DB error to avoid missing transactions
                                self.process_spendable_outpoint(vtxo, &outpoint).await?;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(tx_id, "Error checking if transaction is own: {}", e);
                        // Continue with normal processing in case of DB error
                        match db::is_transaction_processed(&self.db_pool, &tx_id).await {
                            Ok(true) => {
                                tracing::debug!(tx_id, "Transaction already processed, skipping");
                                continue;
                            }
                            Ok(false) => {
                                self.process_spendable_outpoint(vtxo, &outpoint).await?;
                            }
                            Err(e) => {
                                tracing::error!(
                                    tx_id,
                                    "Error checking if transaction is processed: {}",
                                    e
                                );
                                self.process_spendable_outpoint(vtxo, &outpoint).await?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_spendable_outpoint(
        &self,
        _vtxo: &Vtxo,
        outpoint: &VtxoOutPoint,
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

            // Simple dice game: hash nonce + outpoint txid
            let hash_input = format!("{}{}", current_nonce, outpoint.outpoint.txid);
            let hash = bitcoin::hashes::sha256::Hash::hash(hash_input.as_bytes());
            let hash_bytes = hash.as_byte_array();

            // Use first 8 bytes as u64 for randomness
            let random_value = u64::from_be_bytes([
                hash_bytes[0],
                hash_bytes[1],
                hash_bytes[2],
                hash_bytes[3],
                hash_bytes[4],
                hash_bytes[5],
                hash_bytes[6],
                hash_bytes[7],
            ]);

            // Simple game: if random value is even, player wins 1.8x their bet
            // If odd, house wins and player loses their bet
            let player_wins = random_value % 2 == 0;
            let rolled_number = (random_value % 100) as i64; // Convert to 0-99 range for display

            if player_wins {
                let payout = (input_amount * 18) / 10; // 1.8x multiplier
                let payout_amount = bitcoin::Amount::from_sat(payout);

                tracing::info!(
                    nonce = current_nonce,
                    random = random_value,
                    bet_amount = input_amount,
                    payout = payout,
                    "🎉 Player won! Sending payout...."
                );

                match self.ark_client.send(&sender_address, payout_amount).await {
                    Ok(txid) => {
                        tracing::info!(txid = txid.to_string(), "🎉 Player won! Sent payout");

                        // Store this as our own transaction
                        if let Err(e) =
                            db::insert_own_transaction(&self.db_pool, &txid.to_string(), "payout")
                                .await
                        {
                            tracing::error!("Failed to store own transaction: {}", e);
                        }

                        // Store successful winning game result
                        if let Err(e) = db::insert_game_result(
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
                        )
                        .await
                        {
                            tracing::error!("Failed to store game result: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("🚨 Failed to send payout: {}", e);

                        // Store failed winning game result
                        if let Err(e) = db::insert_game_result(
                            &self.db_pool,
                            &current_nonce.to_string(),
                            rolled_number,
                            &outpoint.outpoint.txid.to_string(),
                            None,
                            input_amount as i64,
                            Some(payout as i64),
                            &sender,
                            true,
                            false,
                        )
                        .await
                        {
                            tracing::error!("Failed to store game result: {}", e);
                        }
                    }
                }
            } else {
                tracing::info!(
                    nonce = current_nonce,
                    random = random_value,
                    bet = input_amount,
                    "🏠 House won! Player lost their bet"
                );

                // Store losing game result
                if let Err(e) = db::insert_game_result(
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
                )
                .await
                {
                    tracing::error!("Failed to store game result: {}", e);
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
) {
    let processor = TransactionProcessor::new(
        ark_client,
        my_addresses,
        check_interval_seconds,
        nonce_service,
        db_pool,
    );

    tokio::spawn(async move {
        processor.start_monitoring().await;
    });
}
