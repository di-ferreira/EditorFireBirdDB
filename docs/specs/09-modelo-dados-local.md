# Spec 09 — Modelo de Dados Local (SQLite)

**Documento de origem:** Seção 8 da especificação técnica
**ADRs relacionadas:** ADR-003 (persistência SQLite e keyring do sistema)

O SQLite armazena somente metadados da aplicação; não substitui nem copia o banco Firebird.

## 1. Tabelas propostas

### `connection_profiles`
| Coluna | Tipo | Observação |
|---|---|---|
| `id` | TEXT PK | |
| `name` | TEXT NOT NULL | |
| `host` | TEXT NOT NULL | |
| `port` | INTEGER NOT NULL | |
| `database_path` | TEXT NOT NULL | |
| `username` | TEXT NOT NULL | |
| `role_name` | TEXT NULL | |
| `charset` | TEXT NOT NULL | |
| `sql_dialect` | INTEGER NULL | 1 ou 3 |
| `client_library_path` | TEXT NULL | |
| `connect_timeout_ms` | INTEGER NOT NULL | |
| `secret_ref` | TEXT NULL | referência no cofre do sistema |
| `created_at` / `updated_at` | TEXT NOT NULL | |

**Não incluir senha em coluna SQLite.** `secret_ref` identifica a entrada no cofre.

### `query_history`
`id` TEXT PK · `connection_profile_id` TEXT NULL · `query_text` TEXT NOT NULL ·
`status` TEXT NOT NULL · `duration_ms` INTEGER NULL · `rows_returned` INTEGER NULL ·
`error_code` TEXT NULL · `error_message` TEXT NULL · `executed_at` TEXT NOT NULL ·
`execution_mode` TEXT NOT NULL

A opção de não persistir histórico precisa ser respeitada. O SQL do histórico pode conter
informação sensível escrita pelo usuário; a aplicação deve oferecer limpeza e desativação.

### `saved_scripts`
`id` TEXT PK · `name` TEXT NOT NULL · `content` TEXT NOT NULL ·
`connection_profile_id` TEXT NULL · `created_at`/`updated_at` TEXT NOT NULL

### `ai_provider_profiles`
`id` TEXT PK · `name` TEXT NOT NULL · `provider_type` TEXT NOT NULL ·
`base_url` TEXT NOT NULL · `model_name` TEXT NOT NULL · `secret_ref` TEXT NULL ·
`timeout_ms` INTEGER NOT NULL · `remote_context_confirmation` INTEGER NOT NULL ·
`created_at`/`updated_at` TEXT NOT NULL

### `schema_descriptions`
`id` TEXT PK · `connection_fingerprint` TEXT NOT NULL · `object_type` TEXT NOT NULL ·
`object_name` TEXT NOT NULL · `member_name` TEXT NULL · `description` TEXT NOT NULL ·
`source` TEXT NOT NULL (`user`, `ai_draft`, `imported`) · `review_status` TEXT NOT NULL ·
`updated_at` TEXT NOT NULL

### `ai_interactions`
`id` TEXT PK · `provider_profile_id` TEXT NULL · `connection_fingerprint` TEXT NULL ·
`task_type` TEXT NOT NULL · `prompt_text` TEXT NOT NULL · `context_summary` TEXT NOT NULL ·
`response_text` TEXT NOT NULL · `accepted_sql` TEXT NULL · `created_at` TEXT NOT NULL

O histórico de IA deve poder ser desativado ou removido. Nenhum token secreto ou
credencial será guardado nessas tabelas.

### `app_settings`
`key` TEXT PK · `value_json` TEXT NOT NULL · `updated_at` TEXT NOT NULL

## 2. Migrações

- Versionar o schema com tabela `schema_migrations`.
- Migrações transacionais e idempotentes quando possível.
- Cópia de segurança do banco local antes de migrações destrutivas.
- Não colocar informações sigilosas em mensagens de migração/log.
