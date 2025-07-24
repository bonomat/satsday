use anyhow::Result;
use ark_core::server::VtxoOutPoint;
use ark_core::{ArkAddress, Vtxo};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

use crate::ArkClient;

pub struct TransactionProcessor {
    ark_client: Arc<ArkClient>,
    my_addresses: Vec<ArkAddress>,
    check_interval: Duration,
}

impl TransactionProcessor {
    pub fn new(
        ark_client: Arc<ArkClient>,
        my_addresses: Vec<ArkAddress>,
        check_interval_seconds: u64,
    ) -> Self {
        Self {
            ark_client,
            my_addresses,
            check_interval: Duration::from_secs(check_interval_seconds),
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

        for (vtxo, outpoints) in spendable_vtxos {
            for outpoint in outpoints {
                self.process_spendable_outpoint(&vtxo, &outpoint).await?;
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
            outpoint = ?outpoint.outpoint,
            spent = ?outpoint.is_spent,
            spent = ?outpoint.spent_by,
            "Processing spendable outpoint"
        );
        // dbg!(&outpoint);

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

            // TODO: Add dice game logic here
            // For now, send back 0.5*amount sats as a test
            // let amount = outpoint.amount / 2;
            // let txid = self.ark_client.send(&sender_address, amount).await?;
            // tracing::info!(?amount, txid = txid.to_string(), "Sent back to sender");
        }

        Ok(())
    }
}

pub async fn spawn_transaction_monitor(
    ark_client: Arc<ArkClient>,
    my_addresses: Vec<ArkAddress>,
    check_interval_seconds: u64,
) {
    let processor = TransactionProcessor::new(ark_client, my_addresses, check_interval_seconds);

    tokio::spawn(async move {
        processor.start_monitoring().await;
    });
}
