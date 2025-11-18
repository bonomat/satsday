use crate::db;
use crate::games::get_game;
use crate::nonce_service::NonceService;
use crate::ArkClient;
use anyhow::Context;
use anyhow::Result;
use bitcoin::Amount;
use bitcoin::OutPoint;
use sqlx::Pool;
use sqlx::Sqlite;
use std::sync::Arc;

/// Check for and process any missed games on startup by:
pub async fn process_missed_payouts(
    ark_client: Arc<ArkClient>,
    pool: &Pool<Sqlite>,
    dry_run: bool,
    hours: Option<u64>,
) -> Result<()> {
    let mut successful_payouts = 0;
    let mut failed_payouts = 0;
    let mut total_payout_amount = 0u64;
    let mut retry_payouts = 0;

    let unpaid_winners = match hours {
        Some(h) => db::get_unpaid_winners_within_hours(pool, h).await?,
        None => db::get_unpaid_winners(pool).await?,
    };
    if !unpaid_winners.is_empty() {
        tracing::info!("Found {} unpaid winners to process", unpaid_winners.len());

        for winner in unpaid_winners {
            retry_payouts += 1;
            let payout_sats = winner.winning_amount.unwrap_or(0) as u64;
            total_payout_amount += payout_sats;

            if dry_run {
                tracing::info!(
                    "🎰 [DRY RUN] Would retry payout for winner: game_id={}, player={}, payout={} sats",
                    winner.id,
                    winner.player_address,
                    payout_sats
                );
                successful_payouts += 1;
            } else {
                tracing::info!(
                    "🎰 Retrying payout for unpaid winner: game_id={}, player={}, payout={} sats",
                    winner.id,
                    winner.player_address,
                    payout_sats
                );

                // Decode player address
                let player_address = match ark_core::ArkAddress::decode(&winner.player_address) {
                    Ok(addr) => addr,
                    Err(e) => {
                        tracing::error!(
                            "Failed to decode player address {}: {}",
                            winner.player_address,
                            e
                        );
                        failed_payouts += 1;
                        continue;
                    }
                };

                // Attempt to send payout with retries
                const MAX_RETRIES: u8 = 3;
                let mut retry_count = 0;
                let mut payout_sent = false;

                while retry_count < MAX_RETRIES {
                    ark_client.sync_spendable_vtxos().await?;

                    match ark_client
                        .send_vtxo(player_address, Amount::from_sat(payout_sats))
                        .await
                    {
                        Ok(txid) => {
                            let output_txid = txid.to_string();
                            tracing::info!(
                                "✅ Retry payout sent: game_id={}, payout_txid={}, amount={} sats",
                                winner.id,
                                txid,
                                payout_sats
                            );
                            payout_sent = true;

                            // Store as our own transaction
                            if let Err(e) =
                                db::insert_own_transaction(pool, &output_txid, "retry_payout").await
                            {
                                tracing::error!("Failed to store own transaction: {}", e);
                            }

                            // Mark as paid in database
                            if let Err(e) =
                                db::mark_payment_successful(pool, winner.id, &output_txid).await
                            {
                                tracing::error!("Failed to mark payment as successful: {}", e);
                            }

                            successful_payouts += 1;
                            break;
                        }
                        Err(e) => {
                            retry_count += 1;
                            tracing::error!(
                                "Failed to send retry payout (attempt {}/{}): {:#}",
                                retry_count,
                                MAX_RETRIES,
                                e
                            );

                            if retry_count < MAX_RETRIES {
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                            }
                        }
                    }
                }

                if !payout_sent {
                    failed_payouts += 1;
                    tracing::error!(
                        "❌ Failed to send retry payout after {} attempts for game_id={}",
                        MAX_RETRIES,
                        winner.id
                    );
                }
            }
        }
    } else {
        tracing::info!("✅ No unpaid winners found in database");
    }

    if failed_payouts > 0 {
        tracing::error!(
            "⚠️  Recovery completed: {} retry payouts sent, {} FAILED",
            successful_payouts - failed_payouts,
            failed_payouts
        );
        Err(anyhow::anyhow!("{} retry payouts failed", failed_payouts))
    } else if retry_payouts > 0 {
        let new_winners = successful_payouts - retry_payouts;
        tracing::info!(
                "✅ Recovery completed: {} retry payouts sent + {} new winners recorded in DB (total {} sats pending payout)",
                retry_payouts,
                new_winners,
                total_payout_amount
            );
        Ok(())
    } else {
        tracing::info!(
            "✅ No unpaid winners or missed games found - all transactions are up to date"
        );
        Ok(())
    }
}
/// 1. Fetching all VTXOs for game addresses from the Ark server
/// 2. Checking which ones are not in our database
/// 3. Evaluating missed games and paying out winners
pub async fn process_missed_games(
    ark_client: Arc<ArkClient>,
    pool: &Pool<Sqlite>,
    nonce_service: &NonceService,
    max_payout_sats: u64,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        tracing::info!(
            "🔍 Checking for missed games by scanning all game address VTXOs (DRY RUN)..."
        );
    } else {
        tracing::info!("🔍 Checking for missed games by scanning all game address VTXOs...");
    }
    ark_client.sync_spendable_vtxos().await?;

    // Get all game addresses
    let game_addresses = ark_client.get_game_addresses();
    let addresses: Vec<_> = game_addresses
        .iter()
        .map(|(_, _, address)| *address)
        .collect();

    tracing::info!("📡 Fetching VTXOs for {} game addresses", addresses.len());

    // Fetch all VTXOs for game addresses from Ark server
    let vtxos = ark_client
        .list_vtxos(addresses.as_slice())
        .await
        .context("Failed to fetch VTXOs from Ark server")?;

    tracing::info!(
        "Found {} total VTXOs across all game addresses",
        vtxos.len()
    );

    let mut new_games = 0;
    let mut already_processed = 0;
    let mut own_transactions = 0;
    let mut successful_payouts = 0;
    let failed_payouts = 0;
    let mut total_payout_amount = 0u64;
    let mut donation_count = 0;
    let retry_payouts = 0;

    // First, handle unpaid winners from database
    tracing::info!("🔍 Checking for unpaid winners in database...");

    for vtxo in vtxos {
        let tx_id = vtxo.outpoint.txid.to_string();

        // Skip if already processed
        if db::is_transaction_processed(pool, &tx_id).await? {
            already_processed += 1;
            continue;
        }

        // Skip if it's our own transaction
        if db::is_own_transaction(pool, &tx_id).await? {
            own_transactions += 1;
            continue;
        }

        // This is a new game we haven't seen!
        new_games += 1;
        tracing::info!(
            "🎲 Found unprocessed game: txid={}, amount={} sats",
            tx_id,
            vtxo.amount.to_sat()
        );

        // Find which game this VTXO belongs to
        let (game_type, multiplier) = match game_addresses
            .iter()
            .find(|(_, _, addr)| {
                vtxo.script == addr.to_p2tr_script_pubkey()
                    || vtxo.script == addr.to_sub_dust_script_pubkey()
            })
            .map(|(gt, m, _)| (*gt, *m))
        {
            Some(game_info) => game_info,
            None => {
                tracing::warn!("Could not find game address for VTXO script, skipping");
                continue;
            }
        };

        // Get sender address
        let out_point = OutPoint {
            txid: vtxo.outpoint.txid,
            vout: vtxo.outpoint.vout,
        };

        let ark_addresses = ark_client.get_parent_vtxo(out_point).await?;
        let own_address = ark_client.get_address();

        let sender_address = match ark_addresses
            .into_iter()
            .find(|addr| addr.encode() != own_address.encode())
        {
            Some(addr) => addr,
            None => {
                tracing::debug!("No external sender found (likely our own transaction), skipping");
                continue;
            }
        };

        let input_amount = vtxo.amount.to_sat();
        let current_nonce = nonce_service.get_current_nonce().await;

        // Check donation threshold
        let donation_threshold = (max_payout_sats * 100) / multiplier.multiplier();
        if input_amount > donation_threshold {
            donation_count += 1;
            if dry_run {
                tracing::info!(
                    "💝 [DRY RUN] Would record donation: amount={} sats (threshold: {}), sender={}",
                    input_amount,
                    donation_threshold,
                    sender_address.encode()
                );
            } else {
                tracing::info!(
                    "💝 Missed donation detected: amount={} sats (threshold: {}), sender={}",
                    input_amount,
                    donation_threshold,
                    sender_address.encode()
                );

                // Store as donation
                if let Err(e) = db::insert_game_result(
                    pool,
                    &current_nonce.to_string(),
                    -1, // Special value for donations
                    &tx_id,
                    None,
                    input_amount as i64,
                    None,
                    &sender_address.encode(),
                    false,
                    false,
                    multiplier.multiplier() as i64,
                )
                .await
                {
                    tracing::error!("Failed to store missed donation: {}", e);
                }
            }
            continue;
        }

        // Evaluate the game
        let game = get_game(game_type);
        let evaluation = game.evaluate(current_nonce, &tx_id, &multiplier);

        let (is_win, payout_amount) = if evaluation.is_win {
            let payout = (input_amount as f64
                * evaluation
                    .payout_multiplier
                    .expect("to have payout multiplier")) as u64;
            (true, Some(payout))
        } else {
            (false, None)
        };

        if is_win {
            let payout_sats = payout_amount.unwrap();
            total_payout_amount += payout_sats;

            if dry_run {
                tracing::info!(
                    "🎰 [DRY RUN] Would record WINNER! txid={}, amount={} sats, payout={} sats, rolled={}, target={}",
                    tx_id,
                    input_amount,
                    payout_sats,
                    evaluation.rolled_value,
                    multiplier.get_lower_than()
                );
                successful_payouts += 1;
            } else {
                tracing::info!(
                    "🎰 Missed WINNER found! Recording in DB (not paying out yet): txid={}, amount={} sats, payout={} sats, rolled={}, target={}",
                    tx_id,
                    input_amount,
                    payout_sats,
                    evaluation.rolled_value,
                    multiplier.get_lower_than()
                );

                // Store game result in database as unpaid winner
                if let Err(e) = db::insert_game_result(
                    pool,
                    &current_nonce.to_string(),
                    evaluation.rolled_value,
                    &tx_id,
                    None, // No output tx yet
                    input_amount as i64,
                    payout_amount.map(|p| p as i64),
                    &sender_address.encode(),
                    true,  // is_winner
                    false, // payment_successful = false (will be paid later)
                    multiplier.multiplier() as i64,
                )
                .await
                {
                    tracing::error!("Failed to store missed winning game: {:#}", e);
                } else {
                    successful_payouts += 1;
                }
            }
        } else {
            if dry_run {
                tracing::debug!(
                    "[DRY RUN] Would record loser: txid={}, rolled={}, target={}",
                    tx_id,
                    evaluation.rolled_value,
                    multiplier.get_lower_than()
                );
            } else {
                tracing::debug!(
                    "Missed loser: txid={}, rolled={}, target={}",
                    tx_id,
                    evaluation.rolled_value,
                    multiplier.get_lower_than()
                );

                // Store losing game result
                if let Err(e) = db::insert_game_result(
                    pool,
                    &current_nonce.to_string(),
                    evaluation.rolled_value,
                    &tx_id,
                    None,
                    input_amount as i64,
                    None,
                    &sender_address.encode(),
                    false,
                    true, // Not a payout needed
                    multiplier.multiplier() as i64,
                )
                .await
                {
                    tracing::error!("Failed to store missed losing game: {:#}", e);
                }
            }
        }
    }

    if dry_run {
        tracing::info!(
            "📊 [DRY RUN] Summary: {} unpaid winners to retry, {} new games found ({} winners, {} donations), {} already processed, {} own transactions",
            retry_payouts,
            new_games,
            successful_payouts - retry_payouts,
            donation_count,
            already_processed,
            own_transactions
        );
        tracing::info!(
            "💰 [DRY RUN] Total payout amount that would be recorded: {} sats",
            total_payout_amount
        );
        if retry_payouts > 0 || new_games > 0 {
            tracing::info!(
                "✅ [DRY RUN] Would record {} retry payouts + {} new games ({} total winners for {} sats)",
                retry_payouts,
                new_games,
                successful_payouts,
                total_payout_amount
            );
        } else {
            tracing::info!("✅ [DRY RUN] No unpaid winners or missed games found - all up to date");
        }
        Ok(())
    } else {
        tracing::info!(
            "📊 Recovery summary: {} retry payouts, {} new games, {} already processed, {} own transactions",
            retry_payouts,
            new_games,
            already_processed,
            own_transactions
        );

        if failed_payouts > 0 {
            tracing::error!(
                "⚠️  Recovery completed: {} retry payouts sent, {} FAILED",
                successful_payouts - (new_games - failed_payouts),
                failed_payouts
            );
            Err(anyhow::anyhow!("{} retry payouts failed", failed_payouts))
        } else if retry_payouts > 0 || new_games > 0 {
            let new_winners = successful_payouts - retry_payouts;
            tracing::info!(
                "✅ Recovery completed: {} retry payouts sent + {} new winners recorded in DB (total {} sats pending payout)",
                retry_payouts,
                new_winners,
                total_payout_amount
            );
            Ok(())
        } else {
            tracing::info!(
                "✅ No unpaid winners or missed games found - all transactions are up to date"
            );
            Ok(())
        }
    }
}
