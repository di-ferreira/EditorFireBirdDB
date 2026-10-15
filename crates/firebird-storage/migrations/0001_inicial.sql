-- Migração inicial do Firebird Studio Linux (Spec 09 — Modelo de Dados Local).
--
-- Regras desta migração:
-- * NENHUMA senha/token/segredo em coluna: apenas `secret_ref`, que aponta para
--   o cofre do sistema (ADR-003).
-- * Idempotente quando possível: `IF NOT EXISTS` em todas as tabelas e índices.
-- * Sem literais que descrevam dados do usuário (nada de nome de host, caminho
--   de banco ou segredo em mensagem/log desta migração).

CREATE TABLE IF NOT EXISTS connection_profiles (
    id                  TEXT    NOT NULL PRIMARY KEY,
    name                TEXT    NOT NULL,
    host                TEXT    NOT NULL,
    port                INTEGER NOT NULL,
    database_path       TEXT    NOT NULL,
    username            TEXT    NOT NULL,
    role_name           TEXT    NULL,
    charset             TEXT    NOT NULL,
    sql_dialect         INTEGER NULL,
    client_library_path TEXT    NULL,
    connect_timeout_ms  INTEGER NOT NULL,
    secret_ref          TEXT    NULL,
    created_at          TEXT    NOT NULL,
    updated_at          TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS query_history (
    id                    TEXT    NOT NULL PRIMARY KEY,
    connection_profile_id TEXT    NULL,
    query_text            TEXT    NOT NULL,
    status                TEXT    NOT NULL,
    duration_ms           INTEGER NULL,
    rows_returned         INTEGER NULL,
    error_code            TEXT    NULL,
    error_message         TEXT    NULL,
    executed_at           TEXT    NOT NULL,
    execution_mode        TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_query_history_executed_at
    ON query_history (executed_at DESC);

CREATE TABLE IF NOT EXISTS saved_scripts (
    id                    TEXT NOT NULL PRIMARY KEY,
    name                  TEXT NOT NULL,
    content               TEXT NOT NULL,
    connection_profile_id TEXT NULL,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_provider_profiles (
    id                             TEXT    NOT NULL PRIMARY KEY,
    name                           TEXT    NOT NULL,
    provider_type                  TEXT    NOT NULL,
    base_url                       TEXT    NOT NULL,
    model_name                     TEXT    NOT NULL,
    secret_ref                     TEXT    NULL,
    timeout_ms                     INTEGER NOT NULL,
    remote_context_confirmation    INTEGER NOT NULL,
    created_at                     TEXT    NOT NULL,
    updated_at                     TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_descriptions (
    id                     TEXT NOT NULL PRIMARY KEY,
    connection_fingerprint TEXT NOT NULL,
    object_type            TEXT NOT NULL,
    object_name            TEXT NOT NULL,
    member_name            TEXT NULL,
    description            TEXT NOT NULL,
    source                 TEXT NOT NULL,
    review_status          TEXT NOT NULL,
    updated_at             TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_interactions (
    id                     TEXT NOT NULL PRIMARY KEY,
    provider_profile_id    TEXT NULL,
    connection_fingerprint TEXT NULL,
    task_type              TEXT NOT NULL,
    prompt_text            TEXT NOT NULL,
    context_summary        TEXT NOT NULL,
    response_text          TEXT NOT NULL,
    accepted_sql           TEXT NULL,
    created_at             TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS app_settings (
    key        TEXT NOT NULL PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);