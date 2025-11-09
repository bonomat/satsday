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
            let game_addresses = client.get_game_addresses();
            let game_addresses = game_addresses
                .into_iter()
                .map(|(_, _, address)| address)
                .collect::<Vec<_>>();

            let vtxos = client.list_vtxos(game_addresses.as_slice()).await?;
            tracing::info!(number = vtxos.len(), "Total games");
            for (game, multiplier, ark_address) in client.get_game_addresses() {
                let per_address = vtxos
                    .iter()
                    .filter(|vtxo| vtxo.script == ark_address.to_p2tr_script_pubkey())
                    .collect::<Vec<_>>();
                let total_received: bitcoin::Amount = per_address.iter().map(|v| v.amount).sum();
                tracing::info!(
                    number_of_games = per_address.len(),
                    total_received = %total_received,
                    address = ark_address.encode(),
                    "👾Game Address {game}-{multiplier}",

                );
            }
        }
    }

    Ok(())
}
