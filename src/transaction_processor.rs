use anyhow::Result;
use ark_core::server::VtxoOutPoint;
use ark_core::Vtxo;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};

use crate::ArkClient;

pub struct TransactionProcessor {
    ark_client: Arc<ArkClient>,
    check_interval: Duration,
}

impl TransactionProcessor {
    pub fn new(ark_client: Arc<ArkClient>, check_interval_seconds: u64) -> Self {
        Self {
            ark_client,
            check_interval: Duration::from_secs(check_interval_seconds),
        }
    }

    pub async fn start_monitoring(&self) {
        info!("🔍 Starting transaction monitoring loop...");
        
        loop {
            if let Err(e) = self.check_for_new_transactions().await {
                error!("Error checking for new transactions: {}", e);
            }
            
            sleep(self.check_interval).await;
        }
    }

    async fn check_for_new_transactions(&self) -> Result<()> {
        debug!("Checking for new spendable VTXOs...");
        
        let spendable_vtxos = self.ark_client.spendable_vtxos(false).await?;
        
        for (vtxo, outpoints) in spendable_vtxos {
            for outpoint in outpoints {
                self.process_spendable_outpoint(&vtxo, &outpoint).await?;
            }
        }
        
        Ok(())
    }

    async fn process_spendable_outpoint(&self, vtxo: &Vtxo, outpoint: &VtxoOutPoint) -> Result<()> {
        debug!(
            "Processing spendable outpoint: {} with amount: {}",
            outpoint.outpoint, outpoint.amount
        );
        
        // TODO: Add dice game logic here
        // This is where we'll determine if this is a dice game transaction
        // and process the win/loss logic
        
        Ok(())
    }
}

pub async fn spawn_transaction_monitor(ark_client: Arc<ArkClient>, check_interval_seconds: u64) {
    let processor = TransactionProcessor::new(ark_client, check_interval_seconds);
    
    tokio::spawn(async move {
        processor.start_monitoring().await;
    });
}