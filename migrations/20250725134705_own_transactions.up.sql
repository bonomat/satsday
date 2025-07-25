CREATE TABLE own_transactions
(
    id               INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    tx_id            TEXT                              NOT NULL UNIQUE,
    transaction_type TEXT                              NOT NULL, -- 'payout' or 'consolidation'
    created_at       DATETIME                          NOT NULL DEFAULT CURRENT_TIMESTAMP
);