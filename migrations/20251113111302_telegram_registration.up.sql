CREATE TABLE telegram_registrations
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    chat_id    TEXT                              NOT NULL UNIQUE,
    registered_at DATETIME                       NOT NULL DEFAULT CURRENT_TIMESTAMP
);
