CREATE TABLE game_results
(
    id                 INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    nonce              TEXT                              NOT NULL,
    rolled_number      INTEGER                           NOT NULL,
    input_tx_id        TEXT                              NOT NULL,
    output_tx_id       TEXT,
    bet_amount         INTEGER                           NOT NULL,
    winning_amount     INTEGER,
    player_address     TEXT                              NOT NULL,
    is_winner          BOOLEAN                           NOT NULL,
    payment_successful BOOLEAN                           NOT NULL DEFAULT FALSE,
    timestamp          DATETIME                          NOT NULL DEFAULT CURRENT_TIMESTAMP
);