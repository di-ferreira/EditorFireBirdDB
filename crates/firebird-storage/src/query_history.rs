//! Repositório de histórico de consultas (Spec 09, `query_history`).
//!
//! O histórico pode ser desligado/limpo pelo usuário; por isso [`clear`]
//! apaga tudo e devolve quantas linhas saíram. `error_message` e `query_text`
//! podem conter texto digitado pelo usuário — o `Debug` do registro é redigido.

use crate::StorageError;

/// Uma execução registrada no histórico local.
#[derive(Clone, PartialEq, Eq)]
pub struct QueryHistoryEntry {
    /// Identificador da entrada.
    pub id: String,
    /// Perfil usado na execução, quando houver.
    pub connection_profile_id: Option<String>,
    /// Texto SQL executado.
    pub query_text: String,
    /// Situação da execução (`ok`, `erro`, `cancelado`...).
    pub status: String,
    /// Duração em milissegundos, quando medida.
    pub duration_ms: Option<i64>,
    /// Linhas retornadas, quando aplicável.
    pub rows_returned: Option<i64>,
    /// Código do erro, quando houve.
    pub error_code: Option<String>,
    /// Mensagem do erro, quando houve.
    pub error_message: Option<String>,
    /// Instante da execução, em UTC (`2026-10-15T12:34:56Z`).
    pub executed_at: String,
    /// Modo de execução (`manual`, `ia`, `script`).
    pub execution_mode: String,
}

/// O `Debug` é manual: histórico carrega texto do usuário e mensagens de erro
/// vindas do driver, que podem conter credenciais digitadas por engano (FR-016).
impl std::fmt::Debug for QueryHistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryHistoryEntry")
            .field("id", &self.id)
            .field("connection_profile_id", &self.connection_profile_id)
            .field(
                "query_text",
                &crate::redact::redact_secrets(&self.query_text),
            )
            .field("status", &self.status)
            .field("duration_ms", &self.duration_ms)
            .field("rows_returned", &self.rows_returned)
            .field("error_code", &self.error_code)
            .field(
                "error_message",
                &self
                    .error_message
                    .as_deref()
                    .map(crate::redact::redact_secrets),
            )
            .field("executed_at", &self.executed_at)
            .field("execution_mode", &self.execution_mode)
            .finish()
    }
}

/// Operações de persistência do histórico de consultas.
pub trait QueryHistoryRepository {
    /// Registra uma execução.
    fn add(&self, e: &QueryHistoryEntry) -> Result<(), StorageError>;
    /// Lista até `limite` entradas, da mais recente para a mais antiga.
    fn list(&self, limite: usize) -> Result<Vec<QueryHistoryEntry>, StorageError>;
    /// Apaga todo o histórico; devolve quantas entradas foram removidas.
    fn clear(&self) -> Result<usize, StorageError>;
}

/// Colunas do histórico, na ordem usada por [`linha_para_entrada`].
const COLUNAS: &str = "id, connection_profile_id, query_text, status, duration_ms, rows_returned, \
     error_code, error_message, executed_at, execution_mode";

impl QueryHistoryRepository for crate::Db {
    fn add(&self, e: &QueryHistoryEntry) -> Result<(), StorageError> {
        if e.id.trim().is_empty() {
            return Err(StorageError::Invalid(
                "id do histórico é obrigatório".into(),
            ));
        }
        if e.query_text.trim().is_empty() {
            return Err(StorageError::Invalid("query_text é obrigatório".into()));
        }

        self.conn().execute(
            "INSERT OR REPLACE INTO query_history (id, connection_profile_id, query_text, status,
                 duration_ms, rows_returned, error_code, error_message, executed_at, execution_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                e.id,
                e.connection_profile_id,
                e.query_text,
                e.status,
                e.duration_ms,
                e.rows_returned,
                e.error_code,
                e.error_message,
                e.executed_at,
                e.execution_mode,
            ],
        )?;
        Ok(())
    }

    fn list(&self, limite: usize) -> Result<Vec<QueryHistoryEntry>, StorageError> {
        // O `id` desempata execuções com o mesmo segundo, para a paginação ser
        // estável; o limite é parâmetro, nunca concatenado.
        let sql = format!(
            "SELECT {COLUNAS} FROM query_history
             ORDER BY executed_at DESC, id DESC
             LIMIT ?1"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let entradas = stmt.query_map([limite as i64], linha_para_entrada)?;
        let lista = entradas.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(lista)
    }

    fn clear(&self) -> Result<usize, StorageError> {
        let removidas = self.conn().execute("DELETE FROM query_history", [])?;
        Ok(removidas)
    }
}

/// Projeção de uma linha para [`QueryHistoryEntry`].
fn linha_para_entrada(row: &rusqlite::Row<'_>) -> rusqlite::Result<QueryHistoryEntry> {
    Ok(QueryHistoryEntry {
        id: row.get(0)?,
        connection_profile_id: row.get(1)?,
        query_text: row.get(2)?,
        status: row.get(3)?,
        duration_ms: row.get(4)?,
        rows_returned: row.get(5)?,
        error_code: row.get(6)?,
        error_message: row.get(7)?,
        executed_at: row.get(8)?,
        execution_mode: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{QueryHistoryEntry, QueryHistoryRepository};
    use crate::Db;

    fn entrada(id: &str, executed_at: &str) -> QueryHistoryEntry {
        QueryHistoryEntry {
            id: id.to_string(),
            connection_profile_id: Some("p1".to_string()),
            query_text: "SELECT * FROM rdb$relations".to_string(),
            status: "ok".to_string(),
            duration_ms: Some(12),
            rows_returned: Some(42),
            error_code: None,
            error_message: None,
            executed_at: executed_at.to_string(),
            execution_mode: "manual".to_string(),
        }
    }

    fn db_com_schema() -> Db {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        db.migrate().expect("aplicando migração");
        db
    }

    #[test]
    fn add_preserva_todos_os_campos() {
        let db = db_com_schema();
        let original = entrada("h1", "2026-10-15T12:00:00Z");
        db.add(&original).expect("registrando execução");

        let lido = db.list(10).expect("listando").remove(0);
        assert_eq!(lido, original);
    }

    #[test]
    fn add_preserva_campos_nulos_e_texto_unicode() {
        let db = db_com_schema();
        let original = QueryHistoryEntry {
            id: "h1".to_string(),
            connection_profile_id: None,
            query_text: "SELECT 'ação — Čatã' FROM RDB$DATABASE".to_string(),
            status: "erro".to_string(),
            duration_ms: None,
            rows_returned: None,
            error_code: Some("-901".to_string()),
            error_message: Some("token de cancelamento".to_string()),
            executed_at: "2026-10-15T12:00:00Z".to_string(),
            execution_mode: "ia".to_string(),
        };
        db.add(&original).expect("registrando execução com nulos");

        let lido = db.list(10).expect("listando").remove(0);
        assert_eq!(lido, original);
        assert_eq!(lido.connection_profile_id, None);
    }

    #[test]
    fn list_devolve_da_mais_recente_para_a_mais_antiga() {
        let db = db_com_schema();
        db.add(&entrada("h1", "2026-10-15T10:00:00Z"))
            .expect("add h1");
        db.add(&entrada("h2", "2026-10-15T12:00:00Z"))
            .expect("add h2");
        db.add(&entrada("h3", "2026-10-15T11:00:00Z"))
            .expect("add h3");

        let lista = db.list(10).expect("listando");
        let ids: Vec<&str> = lista.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["h2", "h3", "h1"]);
    }

    #[test]
    fn list_respeita_o_limite_mas_recente_primeiro() {
        let db = db_com_schema();
        for i in 1..=5 {
            db.add(&entrada(
                &format!("h{i}"),
                &format!("2026-10-15T1{i}:00:00Z"),
            ))
            .expect("add");
        }

        let lista = db.list(2).expect("listando com limite 2");
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].id, "h5");
        assert_eq!(lista[1].id, "h4");
    }

    #[test]
    fn list_com_limite_zero_devolve_vazio() {
        let db = db_com_schema();
        db.add(&entrada("h1", "2026-10-15T12:00:00Z")).expect("add");
        assert!(db.list(0).expect("listando com limite 0").is_empty());
    }

    #[test]
    fn list_em_historico_vazio_devolve_lista_vazia() {
        let db = db_com_schema();
        assert!(db.list(50).expect("listando vazio").is_empty());
    }

    #[test]
    fn clear_apaga_todo_o_historico_e_devolve_a_quantidade() {
        let db = db_com_schema();
        for i in 1..=3 {
            db.add(&entrada(&format!("h{i}"), "2026-10-15T12:00:00Z"))
                .expect("add");
        }

        assert_eq!(db.clear().expect("limpando histórico"), 3);
        assert!(db.list(50).expect("listando após limpeza").is_empty());
    }

    #[test]
    fn clear_em_historico_vazio_devolve_zero() {
        let db = db_com_schema();
        assert_eq!(db.clear().expect("limpando histórico vazio"), 0);
    }

    #[test]
    fn add_substitui_entrada_com_o_mesmo_id() {
        let db = db_com_schema();
        db.add(&entrada("h1", "2026-10-15T12:00:00Z")).expect("add");

        let mut atualizada = entrada("h1", "2026-10-15T12:00:00Z");
        atualizada.query_text = "SELECT 1 FROM RDB$DATABASE".to_string();
        db.add(&atualizada).expect("add com mesmo id");

        let lista = db.list(10).expect("listando");
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].query_text, "SELECT 1 FROM RDB$DATABASE");
    }

    #[test]
    fn query_text_com_aspas_e_comentario_nao_e_interpretado_como_sql() {
        let db = db_com_schema();
        let malicioso = entrada("h1", "2026-10-15T12:00:00Z");
        db.add(&QueryHistoryEntry {
            query_text: "'; DROP TABLE query_history; --".to_string(),
            ..malicioso.clone()
        })
        .expect("texto com SQL deve ser dado, não comando");

        let lista = db.list(10).expect("listando");
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].query_text, "'; DROP TABLE query_history; --");
    }

    #[test]
    fn debug_da_entrada_nao_expoe_segredo_da_mensagem_de_erro() {
        let entrada = QueryHistoryEntry {
            error_message: Some("login failed: password=segredo123".to_string()),
            ..entrada("h1", "2026-10-15T12:00:00Z")
        };
        let texto = format!("{entrada:?}");
        assert!(
            !texto.contains("segredo123"),
            "a senha não pode aparecer no Debug do histórico: {texto}"
        );
        assert!(texto.contains("password=***"), "{texto}");
    }
}
