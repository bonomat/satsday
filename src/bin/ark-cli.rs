use anyhow::Result;
use clap::Parser;
use satoshi_dice::{ArkClient, Config, db, utils::init_tracing};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;

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
    init_tracing();

    let cli = Cli::parse();

    let config = Config::from_file(&cli.config)?;

    let db_url = config.database.clone();
    let pool = SqlitePoolOptions::new().connect(db_url.as_str()).await?;
    MIGRATOR.run(&pool).await?;

    let client = ArkClient::new(config).await?;

    match cli.command {
        Commands::Start { port } => {
            let game_addresses = client.get_game_addresses();
            println!("🎲 Starting Satoshi Dice server...");
            println!("📍 Offchain address: {}", client.get_address());
            println!("🚢 Boarding address: {}", client.get_boarding_address());
            for (multiplier, address) in game_addresses {
                println!("👾Game Address {}: {}", multiplier, address.encode());
            }

            let balance = client.get_balance().await?;
            println!("💰 Balance: {:?}", balance);

            satoshi_dice::server::start_server(client, port, pool).await?;
        }
        Commands::Balance => {
            let balance = client.get_balance().await?;
            println!(
                "Offchain balance: spendable = {}, expired = {}",
                balance.offchain_spendable, balance.offchain_expired
            );
            println!(
                "Boarding balance: spendable = {}, expired = {}, pending = {}",
                balance.boarding_spendable, balance.boarding_expired, balance.boarding_pending
            );
        }
        Commands::Address => {
            println!("Offchain address: {}", client.get_address());
        }
        Commands::GameAddresses => {
            let game_addresses = client.get_game_addresses();
            for (multiplier, address) in game_addresses {
                println!("👾Game Address {}: {}", multiplier, address.encode());
            }
        }
        Commands::BoardingAddress => {
            println!("Boarding address: {}", client.get_boarding_address());
        }
        Commands::Send { address, amount } => {
            let ark_address = ark_core::ArkAddress::decode(&address)?;
            let amount = bitcoin::Amount::from_sat(amount);
            let txid = client.send(&ark_address, amount).await?;

            println!("Sent {} to {} in transaction {}", amount, address, txid);
            db::insert_own_transaction(&pool, txid.to_string().as_str(), "manual_send").await?;
        }
        Commands::Settle => match client.settle().await? {
            Some(txid) => {
                println!("Settlement completed. Round TXID: {}", txid);
                db::insert_own_transaction(&pool, txid.to_string().as_str(), "consolidation")
                    .await?;
            }
            None => println!("No boarding outputs or VTXOs to settle"),
        },
    }

    Ok(())
}
