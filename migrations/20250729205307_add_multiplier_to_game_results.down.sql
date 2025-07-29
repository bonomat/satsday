-- SQLite doesn't support dropping columns directly, so we need to recreate the table
CREATE TABLE game_results_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nonce TEXT NOT NULL,
    rolled_number INTEGER NOT NULL,
    input_tx_id TEXT NOT NULL,
    output_tx_id TEXT,
    bet_amount INTEGER NOT NULL,
    winning_amount INTEGER,
    player_address TEXT NOT NULL,
    is_winner BOOLEAN NOT NULL,
    payment_successful BOOLEAN NOT NULL DEFAULT FALSE,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO game_results_new SELECT id, nonce, rolled_number, input_tx_id, output_tx_id, bet_amount, winning_amount, player_address, is_winner, payment_successful, timestamp FROM game_results;

DROP TABLE game_results;

ALTER TABLE game_results_new RENAME TO game_results;