use anyhow::Result;
use clap::Parser;
use rand::thread_rng;
use satoshi_dice::db;
use satoshi_dice::logger;
use satoshi_dice::ArkClient;
use satoshi_dice::Config;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use tracing_subscriber::filter::LevelFilter;

static MIGRATOR: Migrator = sqlx::migrate!(); // defaults to "./migrations"

#[derive(Parser)]
#[command(name = "ark-cli")]
#[command(about = "Simple ARK client CLI")]
struct Cli {
    #[arg(short, long, default_value = "local.config.toml")]
    config: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Start {
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    Balance,
    Address,
    Stats,
    GameAddresses,
    BoardingAddress,
    Send {
        address: String,
        amount: u64,
    },
    Settle,
    CatchupMissedPayouts {
        #[arg(short, long, help = "Dry run - show what would be paid without sending payments")]
        dry_run: bool,
    },
    CatchupMissedGames {
        #[arg(short, long, help = "Dry run - show what would be paid without modifying DB")]
        dry_run: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    logger::init_tracing(LevelFilter::DEBUG, false)?;

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("to be able to install crypto providers");

    let cli = Cli::parse();

    let config = Config::from_file(&cli.config)?;

    let db_url = config.database.clone();
    let pool = SqlitePoolOptions::new().connect(db_url.as_str()).await?;
    MIGRATOR.run(&pool).await?;

    let client = ArkClient::new(config.clone()).await?;

    match cli.command {
        Commands::Start { port } => {
            let game_addresses = client.get_game_addresses();
            tracing::info!("🎲 Starting Satoshi Dice server...");
            tracing::info!("📍 Offchain address: {}", client.get_address());
            tracing::info!("🚢 Boarding address: {}", client.get_boarding_address());
            tracing::info!("🚢 Max bet amount: {}", config.max_payout_sats);
            for (game_type, multiplier, address) in game_addresses {
                tracing::info!(
                    "👾Game Address {} {}: {}",
                    game_type.to_string(),
                    multiplier,
                    address.encode()
                );
            }

            let balance = client.get_balance().await?;
            tracing::info!("💰 Balance: {:?}", balance);

            satoshi_dice::server::start_server(client, port, pool, config).await?;
        }
        Commands::Balance => {
            let balance = client.get_balance().await?;
            tracing::info!(
                "Offchain balance: spendable = {}, expired = {}",
                balance.offchain_spendable,
                balance.offchain_expired
            );
            tracing::info!(
                "Boarding balance: spendable = {}, expired = {}, pending = {}",
                balance.boarding_spendable,
                balance.boarding_expired,
                balance.boarding_pending
            );
        }
        Commands::Address => {
            tracing::info!("Offchain address: {}", client.get_address());
        }
        Commands::GameAddresses => {
            let game_addresses = client.get_game_addresses();
            for (game_type, multiplier, address) in game_addresses {
                tracing::info!(
                    "👾Game Address {} {}: {}",
                    game_type as u8,
                    multiplier,
                    address.encode()
                );
            }
        }
        Commands::BoardingAddress => {
            tracing::info!("Boarding address: {}", client.get_boarding_address());
        }
        Commands::Send { address, amount } => {
            let ark_address = ark_core::ArkAddress::decode(&address)?;
            let amount = bitcoin::Amount::from_sat(amount);
            let txid = client.send_vtxo(ark_address, amount).await?;

            tracing::info!("Sent {} to {} in transaction {}", amount, address, txid);
            db::insert_own_transaction(&pool, txid.to_string().as_str(), "manual_send").await?;
        }
        Commands::Settle => {
            let mut rng = thread_rng();
            match client.settle(&mut rng, true).await? {
                Some(txid) => {
                    tracing::info!("Settlement completed. Round TXID: {}", txid);
                    db::insert_own_transaction(&pool, txid.to_string().as_str(), "consolidation")
                        .await?;
                }
                None => tracing::info!("No boarding outputs or VTXOs to settle"),
            }
        }
        Commands::Stats => {
            tracing::info!("📊 Fetching statistics...");

            // VTXO Stats (from Ark server)
            tracing::info!("🔍 Scanning VTXOs on Ark server...");
            let game_addresses = client.get_game_addresses();
            let game_addresses_list = game_addresses
                .into_iter()
                .map(|(_, _, address)| address)
                .collect::<Vec<_>>();

            let vtxos = client.list_vtxos(game_addresses_list.as_slice()).await?;
            tracing::info!(number = vtxos.len(), "📡 Total VTXOs on Ark server");
            let mut all_received = bitcoin::Amount::ZERO;
            for (game, multiplier, ark_address) in client.get_game_addresses() {
                let per_address = vtxos
                    .iter()
                    .filter(|vtxo| vtxo.script == ark_address.to_p2tr_script_pubkey())
                    .collect::<Vec<_>>();
                let total_received: bitcoin::Amount = per_address.iter().map(|v| v.amount).sum();
                all_received  += total_received;
                tracing::info!(
                    number_of_games = per_address.len(),
                    total_received = %total_received,
                    address = ark_address.encode(),
                    "👾 Game Address {game}-{multiplier}",
                );
            }
            tracing::info!(
                total_received = %all_received,
                "💰 Total received on Ark server"
            );

            // Database Stats
            tracing::info!("💾 Fetching database statistics...");
            let db_stats = db::get_database_stats(&pool).await?;

            tracing::info!("📊 Database Statistics:");
            tracing::info!(
                total_games = db_stats.total_games,
                winners = db_stats.total_winners,
                losers = db_stats.total_losers,
                unpaid_winners = db_stats.unpaid_winners,
                "🎲 Games processed"
            );

            let win_rate = if db_stats.total_games > 0 {
                (db_stats.total_winners as f64 / db_stats.total_games as f64) * 100.0
            } else {
                0.0
            };

            tracing::info!(
                win_rate = format!("{:.2}%", win_rate),
                "📈 Win rate"
            );

            tracing::info!(
                total_bet = %bitcoin::Amount::from_sat(db_stats.total_bet_amount as u64),
                total_payout = %bitcoin::Amount::from_sat(db_stats.total_payout_amount as u64),
                house_profit = %bitcoin::Amount::from_sat(db_stats.total_house_profit as u64),
                "💵 Financial summary"
            );

            if db_stats.unpaid_winners > 0 {
                tracing::warn!(
                    unpaid_winners = db_stats.unpaid_winners,
                    "⚠️  Unpaid winners detected! Run 'catchup-missed-games' to process"
                );
            }

            // Per-multiplier stats
            tracing::info!("📊 Win Rate by Multiplier:");
            let multiplier_stats = db::get_stats_by_multiplier(&pool).await?;
            for stat in multiplier_stats {
                let multiplier_display = stat.multiplier as f64 / 100.0;
                let win_rate = if stat.total_games > 0 {
                    (stat.total_winners as f64 / stat.total_games as f64) * 100.0
                } else {
                    0.0
                };
                let house_profit = stat.total_bet_amount - stat.total_payout_amount;
                let house_profit_display = if house_profit >= 0 {
                    format!("+{}", bitcoin::Amount::from_sat(house_profit as u64))
                } else {
                    format!("-{}", bitcoin::Amount::from_sat((-house_profit) as u64))
                };

                tracing::info!(
                    multiplier = format!("{:.2}x", multiplier_display),
                    games = stat.total_games,
                    winners = stat.total_winners,
                    losers = stat.total_losers,
                    win_rate = format!("{:.2}%", win_rate),
                    total_bet = %bitcoin::Amount::from_sat(stat.total_bet_amount as u64),
                    total_payout = %bitcoin::Amount::from_sat(stat.total_payout_amount as u64),
                    house_profit = house_profit_display,
                    "  🎯 Multiplier stats"
                );
            }
        }
        Commands::CatchupMissedPayouts { dry_run } => {
            if dry_run {
                tracing::info!("🔍 Starting missed games catchup process (DRY RUN - no changes will be made)...");
            } else {
                tracing::info!("🔍 Starting missed games catchup process...");
            }


            // Run the missed games recovery
            let client_arc = std::sync::Arc::new(client);
            match satoshi_dice::recovery::process_missed_payouts(
                client_arc,
                &pool,
                dry_run,
            )
                .await
            {
                Ok(()) => {
                    if dry_run {
                        tracing::info!("✅ Missed games catchup dry run completed successfully");
                    } else {
                        tracing::info!("✅ Missed games catchup completed successfully");
                    }
                }
                Err(e) => {
                    tracing::error!("❌ Missed games catchup failed: {:#}", e);
                    return Err(e);
                }
            }
        }

        Commands::CatchupMissedGames { dry_run } => {
            if dry_run {
                tracing::info!("🔍 Starting missed games catchup process (DRY RUN - no changes will be made)...");
            } else {
                tracing::info!("🔍 Starting missed games catchup process...");
            }

            // Create nonce service
            let nonce_service = satoshi_dice::nonce_service::spawn_nonce_service(pool.clone(), 1, 1).await;

            // Run the missed games recovery
            let client_arc = std::sync::Arc::new(client);
            match satoshi_dice::recovery::process_missed_games(
                client_arc,
                &pool,
                &nonce_service,
                config.max_payout_sats,
                dry_run,
            )
            .await
            {
                Ok(()) => {
                    if dry_run {
                        tracing::info!("✅ Missed games catchup dry run completed successfully");
                    } else {
                        tracing::info!("✅ Missed games catchup completed successfully");
                    }
                }
                Err(e) => {
                    tracing::error!("❌ Missed games catchup failed: {:#}", e);
                    return Err(e);
                }
            }
        }
    }

    Ok(())
}
