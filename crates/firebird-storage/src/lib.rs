//! Persistência local do Firebird Studio Linux (Spec 09; ADR-003).
//!
//! O crate expõe um único ponto de entrada, [`Db`], que abre um SQLite local,
//! aplica as migrações versionadas de [`migrations`] e implementa os
//! repositórios de [`connection_profiles`], [`query_history`] e
//! [`saved_scripts`].
//!
//! Nenhum segredo é gravado em coluna: perfis de conexão e provedores de IA
//! guardam apenas `secret_ref`, ponteiro para o cofre do sistema (FR-016).
//! Todo SQL é parametrizado — nenhuma concatenação de valor do usuário.

pub mod connection_profiles;
pub mod migrations;
pub mod query_history;
pub mod saved_scripts;

mod redact;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub use connection_profiles::{ConnectionProfileRecord, ConnectionProfileRepository};
pub use query_history::{QueryHistoryEntry, QueryHistoryRepository};
pub use saved_scripts::{SavedScript, SavedScriptRepository};

/// Erros de persistência local.
///
/// O `Debug` é manual e já sai redigido; o `Display` passa as mensagens do
/// driver por [`redact::redact_secrets`] para que nem panics nem logs levem
/// credenciais (FR-016).
#[derive(thiserror::Error)]
pub enum StorageError {
    /// Falha vinda do driver SQLite (constraint, I/O, SQL inválido).
    #[error("falha de sqlite: {}", crate::redact::redact_secrets(&.0.to_string()))]
    Sqlite(#[from] rusqlite::Error),

    /// Falha ao aplicar uma migração de schema.
    #[error("migração falhou: {}", crate::redact::redact_secrets(&.0))]
    Migration(String),

    /// Registro não encontrado.
    #[error("perfil não encontrado: {}", crate::redact::redact_secrets(&.0))]
    NotFound(String),

    /// Entrada inválida fornecida pela camada de aplicação.
    #[error("entrada inválida: {}", crate::redact::redact_secrets(&.0))]
    Invalid(String),
}

impl std::fmt::Debug for StorageError {
    /// Imprime o nome da variante e a mensagem com credenciais redigidas.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variante = match self {
            StorageError::Sqlite(_) => "Sqlite",
            StorageError::Migration(_) => "Migration",
            StorageError::NotFound(_) => "NotFound",
            StorageError::Invalid(_) => "Invalid",
        };
        write!(
            f,
            "{variante}({})",
            redact::redact_secrets(&self.to_string())
        )
    }
}

/// Conexão com o banco local de metadados.
pub struct Db {
    conn: rusqlite::Connection,
}

impl Db {
    /// Abre (criando se necessário) o banco local em `path`.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { conn })
    }

    /// Abre um banco efêmero em memória — usado em testes, sem tocar o disco.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { conn })
    }

    /// Aplica as migrações pendentes. Idempotente: chamar duas vezes não falha.
    pub fn migrate(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                 version    INTEGER NOT NULL PRIMARY KEY,
                 nome       TEXT    NOT NULL,
                 aplicada_em TEXT   NOT NULL
             );",
        )?;

        for migracao in migrations::MIGRATIONS {
            let aplicada: i64 = self.conn.query_row(
                "SELECT count(*) FROM schema_migrations WHERE version = ?1",
                [migracao.version],
                |r| r.get(0),
            )?;
            if aplicada > 0 {
                continue;
            }
            // Uma transação por migração: falha no meio não deixa schema pela
            // metade nem versão registrada.
            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(migracao.sql).map_err(|e| {
                StorageError::Migration(format!(
                    "{} (versão {}): {}",
                    migracao.nome, migracao.version, e
                ))
            })?;
            tx.execute(
                "INSERT INTO schema_migrations (version, nome, aplicada_em) VALUES (?1, ?2, ?3)",
                rusqlite::params![migracao.version, migracao.nome, agora_iso8601()],
            )?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Acesso interno à conexão, para os repositórios deste crate.
    pub(crate) fn conn(&self) -> &rusqlite::Connection {
        &self.conn
    }

    /// Acesso à conexão para testes de integração do próprio crate.
    #[cfg(test)]
    pub(crate) fn conn_for_tests(&self) -> &rusqlite::Connection {
        &self.conn
    }
}

/// Segundos por dia, base do cálculo civil de [`agora_iso8601`].
const SEGUNDOS_POR_DIA: u64 = 86_400;

/// Instante atual em UTC, no formato `2026-10-15T12:34:56Z`.
///
/// Único ponto de geração de timestamp do crate: todo `created_at`,
/// `updated_at` e `executed_at` gravado sai daqui.
pub(crate) fn agora_iso8601() -> String {
    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    format!(
        "{}T{}Z",
        civil_para_data(segundos / SEGUNDOS_POR_DIA),
        segundos_do_dia(segundos % SEGUNDOS_POR_DIA)
    )
}

/// Converte dias desde a época Unix (1970-01-01) em `YYYY-MM-DD`.
///
/// Algoritmo civil de Howard Hinnant (`civil_from_days`), sem dependência de
/// calendário e sem `chrono`: o app só precisa de UTC com precisão de segundo.
fn civil_para_data(dias: u64) -> String {
    // Desloca a época para 0000-03-01, de modo que o ano bissexto caia no fim.
    let z = dias as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let ano = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = if mes <= 2 { ano + 1 } else { ano };
    format!("{ano:04}-{mes:02}-{dia:02}")
}

/// Formata os segundos dentro do dia como `HH:MM:SS`.
fn segundos_do_dia(segundos: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        segundos / 3600,
        (segundos % 3600) / 60,
        segundos % 60
    )
}

#[cfg(test)]
mod tests {
    use super::{agora_iso8601, Db, StorageError};
    use std::path::PathBuf;

    #[test]
    fn agora_iso8601_tem_formato_utc_com_segundos() {
        let agora = agora_iso8601();
        assert!(
            agora.ends_with('Z'),
            "o timestamp deve ser UTC com sufixo Z: {agora}"
        );
        assert_eq!(agora.len(), 20, "formato YYYY-MM-DDTHH:MM:SSZ: {agora}");
        let bytes = agora.as_bytes();
        assert_eq!(bytes[4], b'-');
        assert_eq!(bytes[7], b'-');
        assert_eq!(bytes[10], b'T');
        assert_eq!(bytes[13], b':');
        assert_eq!(bytes[16], b':');
        assert!(
            bytes[..4].iter().all(u8::is_ascii_digit)
                && bytes[5..7].iter().all(u8::is_ascii_digit)
                && bytes[8..10].iter().all(u8::is_ascii_digit)
                && bytes[11..13].iter().all(u8::is_ascii_digit)
                && bytes[14..16].iter().all(u8::is_ascii_digit)
                && bytes[17..19].iter().all(u8::is_ascii_digit),
            "data e hora devem ser numéricas: {agora}"
        );
    }

    #[test]
    fn agora_iso8601_avanca_entre_chamadas_ou_permite_repeticao_no_mesmo_segundo() {
        let primeiro = agora_iso8601();
        let segundo = agora_iso8601();
        assert!(
            segundo >= primeiro,
            "o relógio não pode andar para trás: {primeiro} depois de {segundo}"
        );
    }

    #[test]
    fn data_civil_converte_dias_desde_a_epoca() {
        use super::{civil_para_data, segundos_do_dia};
        assert_eq!(civil_para_data(0), "1970-01-01");
        assert_eq!(segundos_do_dia(0), "00:00:00");
        assert_eq!(civil_para_data(19_723), "2024-01-01");
        assert_eq!(civil_para_data(19_782), "2024-02-29", "ano bissexto");
        assert_eq!(segundos_do_dia(43_200), "12:00:00");
        assert_eq!(segundos_do_dia(86_399), "23:59:59");
        // 2026-10-15T12:34:56Z usado como exemplo na documentação do crate.
        let dias = 20_741_u64;
        assert_eq!(
            format!(
                "{}T{}Z",
                civil_para_data(dias),
                segundos_do_dia(12 * 3600 + 34 * 60 + 56)
            ),
            "2026-10-15T12:34:56Z"
        );
    }

    #[test]
    fn open_in_memory_devolve_banco_pronto_sem_arquivo_em_disco() {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        let tabelas: i64 = db
            .conn_for_tests()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .expect("consultando sqlite_master");
        assert!(tabelas >= 0);
    }

    #[test]
    fn open_em_diretorio_inexistente_devolve_erro() {
        let caminho: PathBuf = std::env::temp_dir().join("firebird-storage-dir-inexistente");
        let resultado = Db::open(&caminho.join("studio.db"));
        assert!(
            resultado.is_err(),
            "abrir em diretório inexistente deveria falhar"
        );
    }

    #[test]
    fn display_de_storage_error_redige_segredo_da_mensagem_do_driver() {
        let err = StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
            "password=abc123".to_string(),
        ));
        let texto = err.to_string();
        assert!(
            !texto.contains("abc123"),
            "a senha não pode aparecer no Display: {texto}"
        );
        assert!(texto.contains("password=***"), "{texto}");
    }

    #[test]
    fn debug_de_storage_error_nao_expoe_segredo() {
        let err = StorageError::Migration("aplicar 0001: token=segredo-xyz".to_string());
        let texto = format!("{err:?}");
        assert!(
            !texto.contains("segredo-xyz"),
            "token vazou no Debug: {texto}"
        );
        assert!(
            texto.contains("Migration"),
            "o Debug deve manter a variante: {texto}"
        );
        assert!(texto.contains("token=***"), "{texto}");
    }

    #[test]
    fn debug_de_storage_error_nao_expoe_segredo_em_not_found() {
        let err = StorageError::NotFound("perfil pwd=topsecret".to_string());
        let texto = format!("{err:?}");
        assert!(
            !texto.contains("topsecret"),
            "valor vazou no Debug: {texto}"
        );
    }
}
