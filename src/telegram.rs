use crate::db;
use anyhow::Result;
use rand::Rng;
use sqlx::{Pool, Sqlite};
use teloxide::prelude::*;
use teloxide::types::{ChatId, ParseMode};
use teloxide::utils::command::BotCommands;
use tracing::{error, info, warn};

/// Generate a random registration secret
pub fn generate_registration_secret() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    const SECRET_LENGTH: usize = 12;

    let mut rng = rand::thread_rng();
    (0..SECRET_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Satoshi Dice notification bot")]
enum Command {
    #[command(description = "Subscribe to game notifications (requires invite secret)")]
    Start(String),
    #[command(description = "Unsubscribe from notifications")]
    Stop,
    #[command(description = "Check your subscription status")]
    Status,
    #[command(description = "Show help")]
    Help,
}

/// Start the Telegram bot
pub async fn run_telegram_bot(pool: Pool<Sqlite>, token: String, secret: String) -> Result<()> {
    info!("📱 Starting Telegram bot...");

    let bot = Bot::new(token);

    let handler = Update::filter_message().branch(
        dptree::entry()
            .filter_command::<Command>()
            .endpoint(handle_command),
    );

    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![pool, secret])
        .build();

    info!("✓ Telegram bot started and listening for commands");

    dispatcher.dispatch().await;

    Ok(())
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    pool: Pool<Sqlite>,
    secret: String,
) -> ResponseResult<()> {
    let chat_id = msg.chat.id;

    match cmd {
        Command::Start(provided_secret) => {
            handle_start(bot, chat_id, &msg, provided_secret, pool, secret).await?
        }
        Command::Stop => handle_stop(bot, chat_id, pool).await?,
        Command::Status => handle_status(bot, chat_id, pool).await?,
        Command::Help => handle_help(bot, chat_id).await?,
    }

    Ok(())
}

async fn handle_start(
    bot: Bot,
    chat_id: ChatId,
    msg: &Message,
    provided_secret: String,
    pool: Pool<Sqlite>,
    expected_secret: String,
) -> ResponseResult<()> {
    // Check if the secret is correct
    if provided_secret.trim() != expected_secret {
        bot.send_message(
            chat_id,
            "❌ Invalid invite secret. Please contact the admin for the correct secret.",
        )
        .await?;
        warn!(
            "Failed subscription attempt from chat_id {} with invalid secret",
            chat_id
        );
        return Ok(());
    }

    // Get user info
    let username = msg.from.as_ref().and_then(|u| u.username.clone());
    let first_name = msg.from.as_ref().map(|u| u.first_name.clone());

    // Add subscriber to database
    match db::register_telegram_chat(&pool, &chat_id.0.to_string()).await {
        Ok(_) => {
            let display_name = username
                .as_ref()
                .map(|u| format!("@{}", u))
                .or(first_name)
                .unwrap_or("Unknown".to_string());

            bot.send_message(
                chat_id,
                format!(
                    "✅ Welcome, {}! You are now subscribed to Satoshi Dice notifications.\n\n\
                     You'll receive alerts for:\n\
                     • New games played\n\
                     • Winning games 🎉\n\
                     • Lost games\n\
                     • Donations received 💝\n\n\
                     Use /stop to unsubscribe\n\
                     Use /status to check your subscription",
                    display_name
                ),
            )
            .await?;

            info!(
                "New subscriber: chat_id={}, username={:?}",
                chat_id, username
            );
        }
        Err(e) => {
            error!("Failed to add subscriber: {}", e);
            bot.send_message(chat_id, "❌ Failed to subscribe. Please try again later.")
                .await?;
        }
    }

    Ok(())
}

async fn handle_stop(bot: Bot, chat_id: ChatId, pool: Pool<Sqlite>) -> ResponseResult<()> {
    match db::unregister_telegram_chat(&pool, &chat_id.0.to_string()).await {
        Ok(_) => {
            bot.send_message(
                chat_id,
                "✅ You have been unsubscribed from Satoshi Dice notifications.\n\n\
                 You can resubscribe anytime with /start <secret>",
            )
            .await?;

            info!("User unsubscribed: chat_id={}", chat_id);
        }
        Err(e) => {
            error!("Failed to remove subscriber: {}", e);
            bot.send_message(chat_id, "❌ Failed to unsubscribe. Please try again later.")
                .await?;
        }
    }

    Ok(())
}

async fn handle_status(bot: Bot, chat_id: ChatId, pool: Pool<Sqlite>) -> ResponseResult<()> {
    match db::is_telegram_chat_registered(&pool, &chat_id.0.to_string()).await {
        Ok(true) => {
            bot.send_message(
                chat_id,
                "✅ You are subscribed and receiving game notifications.",
            )
            .await?;
        }
        Ok(false) => {
            bot.send_message(
                chat_id,
                "❌ You are not subscribed.\n\nUse /start <secret> to subscribe.",
            )
            .await?;
        }
        Err(e) => {
            error!("Failed to check status for chat {}: {}", chat_id, e);
            bot.send_message(chat_id, "❌ Failed to check status. Please try again later.")
                .await?;
        }
    }

    Ok(())
}

async fn handle_help(bot: Bot, chat_id: ChatId) -> ResponseResult<()> {
    let help_text = "\
🎲 Satoshi Dice Notification Bot

Commands:
/start <secret> - Subscribe to game notifications
/stop - Unsubscribe from notifications
/status - Check your subscription status
/help - Show this help message

This bot sends real-time notifications about game activities.";

    bot.send_message(chat_id, help_text).await?;

    Ok(())
}

/// Send a notification to all subscribers
pub async fn broadcast_message(pool: &Pool<Sqlite>, token: &str, message: &str) -> Result<()> {
    let bot = Bot::new(token);

    let chat_ids = db::get_registered_telegram_chats(pool).await?;

    if chat_ids.is_empty() {
        info!("No telegram subscribers to notify");
        return Ok(());
    }

    tracing::debug!(
        "Broadcasting message to {} subscribers: {}",
        chat_ids.len(),
        message
    );


    for chat_id_str in chat_ids {
        if let Ok(chat_id_i64) = chat_id_str.parse::<i64>() {
            let chat_id = ChatId(chat_id_i64);

            if let Err(e) = bot
                .send_message(chat_id, message)
                .parse_mode(ParseMode::Html)
                .await
            {
                error!("Failed to send message to chat_id {}: {}", chat_id_str, e);
                // Optionally remove subscriber if bot is blocked
                if e.to_string().contains("bot was blocked") {
                    warn!("Removing blocked subscriber: {}", chat_id_str);
                    if let Err(e) = db::unregister_telegram_chat(pool, &chat_id_str).await {
                        error!("Failed to remove blocked subscriber: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Helper function to notify about a new game
pub async fn notify_game_played(
    pool: &Pool<Sqlite>,
    token: &str,
    player: &str,
    amount_sats: u64,
    multiplier: f64,
    game_tx_id: &str,
) -> Result<()> {
    let message = format!(
        "🎲 <b>New Game!</b>\n\
        \n\
        👤 Player: <code>{}</code>\n\
        💰 Bet: {} sats\n\
        🎯 Multiplier: {}x\n\
        🔗 TX: <code>{}</code>",
        truncate_address(player),
        amount_sats,
        multiplier,
        truncate_txid(game_tx_id)
    );

    broadcast_message(pool, token, &message).await
}

/// Helper function to notify about a win
pub async fn notify_win(
    pool: &Pool<Sqlite>,
    token: &str,
    player: &str,
    bet_amount: u64,
    payout_amount: u64,
    multiplier: f64,
    rolled_number: i64,
    target_number: u16,
    game_tx_id: &str,
    payout_tx_id: &str,
) -> Result<()> {
    let message = format!(
        "🎰 <b>WINNER!</b> 🎉\n\
        \n\
        👤 Player: <code>{}</code>\n\
        💰 Bet: {} sats\n\
        🎯 Multiplier: {}x\n\
        🎲 Rolled: {} (needed &lt; {})\n\
        💸 Payout: <b>{} sats</b>\n\
        📥 Game TX: <code>{}</code>\n\
        📤 Payout TX: <code>{}</code>",
        truncate_address(player),
        bet_amount,
        multiplier,
        rolled_number,
        target_number,
        payout_amount,
        truncate_txid(game_tx_id),
        truncate_txid(payout_tx_id)
    );

    broadcast_message(pool, token, &message).await
}

/// Helper function to notify about a loss
pub async fn notify_loss(
    pool: &Pool<Sqlite>,
    token: &str,
    player: &str,
    bet_amount: u64,
    multiplier: f64,
    rolled_number: i64,
    target_number: u16,
    game_tx_id: &str,
) -> Result<()> {
    let message = format!(
        "😢 <b>Loss</b>\n\
        \n\
        👤 Player: <code>{}</code>\n\
        💰 Bet: {} sats\n\
        🎯 Multiplier: {}x\n\
        🎲 Rolled: {} (needed &lt; {})\n\
        🔗 TX: <code>{}</code>",
        truncate_address(player),
        bet_amount,
        multiplier,
        rolled_number,
        target_number,
        truncate_txid(game_tx_id)
    );

    broadcast_message(pool, token, &message).await
}

/// Helper function to notify about a donation
pub async fn notify_donation(
    pool: &Pool<Sqlite>,
    token: &str,
    donor: &str,
    amount_sats: u64,
    game_tx_id: &str,
) -> Result<()> {
    let message = format!(
        "💝 <b>Donation Received!</b>\n\
        \n\
        👤 From: <code>{}</code>\n\
        💰 Amount: {} sats\n\
        🔗 TX: <code>{}</code>\n\
        \n\
        Thank you for your support! 🙏",
        truncate_address(donor),
        amount_sats,
        truncate_txid(game_tx_id)
    );

    broadcast_message(pool, token, &message).await
}

fn truncate_address(address: &str) -> String {
    if address.len() > 20 {
        format!("{}...{}", &address[..10], &address[address.len() - 6..])
    } else {
        address.to_string()
    }
}

fn truncate_txid(txid: &str) -> String {
    if txid.len() > 16 {
        format!("{}...{}", &txid[..8], &txid[txid.len() - 8..])
    } else {
        txid.to_string()
    }
}
