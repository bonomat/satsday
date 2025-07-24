CREATE TABLE nonces
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    nonce      TEXT                              NOT NULL UNIQUE,
    created_at DATETIME                          NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME                          NOT NULL
);
