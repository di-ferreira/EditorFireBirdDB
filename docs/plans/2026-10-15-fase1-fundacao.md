# Fase 1 — Fundação — Implementation Plan (modo entrega)

**Objetivo:** transformar o PoC em fundação de produto — CI, persistência SQLite, cofre de
segredos, domínio de conexões e a fronteira Tauri que os expõe. Escopo conforme
`docs/specs/14-plano-entrega.md` (Fase 1) e specs 03, 09, 10, 11.

**Decisão de processo (pedido do usuário):** modo entrega — verificação enxuta
(`cargo test` + `clippy -D warnings` + `fmt` + `pnpm build` verdes no fim), sem ciclo de
revisão por task. Execução **paralela** com conjuntos de arquivos disjuntos (desvio do
cerimônio SDD, aceito pelo usuário).

## Tarefas

| # | Tarefa | Arquivos (disjuntos) | Agente |
|---|---|---|---|
| A | SQLite + migrações + repositórios | `crates/firebird-storage/**` | rust-domain-dev |
| B | Cofre de segredos | `crates/firebird-security/**` | rust-domain-dev |
| C | UI de conexões + `package.json` raiz | `apps/desktop/src/**`, `apps/desktop/package.json`, `package.json` (raiz) | frontend-dev |
| D | CI | `.github/workflows/**` | general |
| E | Domínio de conexão + Connection Manager + comandos Tauri (depende de A e B) | `crates/firebird-domain/src/connection.rs`, `apps/desktop/src-tauri/src/{application,commands}/**`, `apps/desktop/src-tauri/Cargo.toml` | rust-domain-dev |

Workspace: adicionar `crates/*` já é glob — A e B entram sozinhos. `apps/desktop/src-tauri`
já é member.

## Interfaces (contrato entre os agentes — usar literalmente)

### A — `firebird-storage`

```rust
// crates/firebird-storage/src/lib.rs
pub mod migrations;
pub mod connection_profiles;
pub mod query_history;
pub mod saved_scripts;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("falha de sqlite: {0}")] Sqlite(#[from] rusqlite::Error),
    #[error("migração falhou: {0}")] Migration(String),
    #[error("perfil não encontrado: {0}")] NotFound(String),
    #[error("entrada inválida: {0}")] Invalid(String),
}

pub struct Db { /* conn */ }
impl Db {
    pub fn open(path: &std::path::Path) -> Result<Self, StorageError>;
    pub fn open_in_memory() -> Result<Self, StorageError>;
    pub fn migrate(&self) -> Result<(), StorageError>;
}

pub struct ConnectionProfileRecord {
    pub id: String, pub name: String, pub host: String, pub port: i64,
    pub database_path: String, pub username: String, pub role_name: Option<String>,
    pub charset: String, pub sql_dialect: Option<i64>,
    pub client_library_path: Option<String>, pub connect_timeout_ms: i64,
    pub secret_ref: Option<String>, pub created_at: String, pub updated_at: String,
}
pub trait ConnectionProfileRepository {
    fn save(&self, p: &ConnectionProfileRecord) -> Result<(), StorageError>;
    fn get(&self, id: &str) -> Result<Option<ConnectionProfileRecord>, StorageError>;
    fn list(&self) -> Result<Vec<ConnectionProfileRecord>, StorageError>;
    fn delete(&self, id: &str) -> Result<bool, StorageError>;
}
impl ConnectionProfileRepository for Db { /* SQL parametrizado, NUNCA string concat */ }

pub struct QueryHistoryEntry { /* id, connection_profile_id, query_text, status,
    duration_ms, rows_returned, error_code, error_message, executed_at, execution_mode */ }
pub trait QueryHistoryRepository {
    fn add(&self, e: &QueryHistoryEntry) -> Result<(), StorageError>;
    fn list(&self, limite: usize) -> Result<Vec<QueryHistoryEntry>, StorageError>;
    fn clear(&self) -> Result<usize, StorageError>;
}
pub struct SavedScript { /* id, name, content, connection_profile_id, created_at, updated_at */ }
pub trait SavedScriptRepository { /* save, list, get, delete */ }
```

Migrações em `crates/firebird-storage/migrations/0001_inicial.sql` (SQL das tabelas da
Spec 09 — `connection_profiles`, `query_history`, `saved_scripts`, `ai_provider_profiles`,
`schema_descriptions`, `ai_interactions`, `app_settings`) + `schema_migrations`; `include_str!`
versionado; idempotente; **sem senha em lugar nenhum** (apenas `secret_ref`).

### B — `firebird-security`

```rust
// crates/firebird-security/src/lib.rs
#[derive(Debug, thiserror::Error)]
pub enum SecurityError { #[error("cofre indisponível: {0}")] Unavailable(String), #[error("segredo ausente")] NotFound }

pub trait SecretStore: Send + Sync {
    fn set(&self, ref_: &str, secret: &str) -> Result<(), SecurityError>;
    fn get(&self, ref_: &str) -> Result<Option<String>, SecurityError>;
    fn delete(&self, ref_: &str) -> Result<(), SecurityError>;
}
pub struct KeyringSecretStore { /* keyring::Entry com service "firebird-studio" */ }
impl SecretStore for KeyringSecretStore { /* Linux: Secret Service/libsecret */ }

#[cfg(any(test, feature = "in-memory"))]
pub struct InMemorySecretStore { /* Mutex<BTreeMap> */ }
#[cfg(any(test, feature = "in-memory"))]
impl SecretStore for InMemorySecretStore { /* segredos em RAM, Debug redigido */ }

/// Chave do cofre para um perfil (determinística, sem PII).
pub fn secret_ref(profile_id: &str) -> String; // "firebird-studio/profile/<id>"
```

`Debug`/`Display` de qualquer tipo que carregue segredo: **sempre redigido** (reutilizar
`firebird_domain::DomainError::redacted` ou padrão `***`).

### E — Domínio + Manager + Comandos (após A e B)

```rust
// crates/firebird-domain/src/connection.rs (novo, TDD)
pub struct ConnectionProfile { id: String, name: String, host: String, port: u16,
    database_path: String, username: String, role_name: Option<String>, charset: String,
    sql_dialect: Option<SqlDialect>, client_library_path: Option<String>, connect_timeout_ms: u64 }
pub enum SqlDialect { V1, V3 }
pub enum ConnectionStatus { Disconnected, Connecting, Connected, Reconnecting, Error(String) }
// Converte de/para ConnectionProfileRecord (A) e ConnParams (adapter) — sem campos de senha.
```

`apps/desktop/src-tauri/src/application/connection_manager.rs`: sessão em `Mutex<Option<SessaoAtiva>>`,
`abrir(profile, segredo) -> Result<SessionId, DomainError>`, `fechar(id)`, `status(id)`.
`apps/desktop/src-tauri/src/commands/connections.rs`: `connection_list_profiles`,
`connection_save_profile`, `connection_test_profile`, `connection_open`, `connection_close`,
`connection_get_status`. Erros serializados como `{ code, message }` com `code` ∈
`ConnectionError | UnsupportedServerFeature | QueryError | PermissionDenied | ProviderError`.

### C — Frontend
`apps/desktop/src/features/connections/` (api.ts com `invoke` tipado, tipos, painel com
loading/vazio/erro/sucesso) + integração em `App.tsx` (abas "Saúde" e "Conexões").
`package.json` raiz com `"private": true` e scripts agregadores (`build`, `test`, `lint`).

## Critérios de pronto (verificação enxuta, no fim)

```bash
cargo test --workspace          # todas as suites verdes
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cd apps/desktop && pnpm build   # tsc strict + vite
```

## Fora de escopo (Fase 2+)

Explorer, SQL Editor, IA, SQLite de histórico de query na UI, E2E, SBOM, `.deb`.