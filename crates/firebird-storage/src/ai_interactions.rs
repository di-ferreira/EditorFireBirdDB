//! Repositório de interações com a IA (Spec 09, `ai_interactions`).
//!
//! Registra **o que foi pedido, o contexto resumido e o que respondeu**.
//! Prompt e resposta são texto do usuário e podem conter trechos de SQL/dados
//! copiados por engano, então o `Debug` é redigido (FR-016). Nenhum segredo é
//! gravado aqui: a chave da API vive no cofre e não faz parte do registro.

use crate::StorageError;
use rusqlite::OptionalExtension;

/// Uma interação registrada com o assistente de IA.
#[derive(Clone, PartialEq, Eq)]
pub struct AiInteraction {
    /// Identificador da interação.
    pub id: String,
    /// Provedor usado, quando houver.
    pub provider_profile_id: Option<String>,
    /// Impressão digital do schema usado como contexto.
    pub connection_fingerprint: Option<String>,
    /// Tarefa (`gerar_sql`, `explicar_sql`, …).
    pub task_type: String,
    /// Prompt enviado (texto do usuário + instruções).
    pub prompt_text: String,
    /// Resumo do contexto enviado (nomes de relações/tabelas, sem dados).
    pub context_summary: String,
    /// Resposta do modelo.
    pub response_text: String,
    /// SQL sugerido que o usuário aceitou, quando aceitou.
    pub accepted_sql: Option<String>,
    /// Instante em UTC.
    pub created_at: String,
}

impl std::fmt::Debug for AiInteraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiInteraction")
            .field("id", &self.id)
            .field("provider_profile_id", &self.provider_profile_id)
            .field("connection_fingerprint", &self.connection_fingerprint)
            .field("task_type", &self.task_type)
            .field(
                "prompt_text",
                &crate::redact::redact_secrets(&self.prompt_text),
            )
            .field(
                "context_summary",
                &crate::redact::redact_secrets(&self.context_summary),
            )
            .field(
                "response_text",
                &crate::redact::redact_secrets(&self.response_text),
            )
            .field(
                "accepted_sql",
                &self
                    .accepted_sql
                    .as_deref()
                    .map(crate::redact::redact_secrets),
            )
            .field("created_at", &self.created_at)
            .finish()
    }
}

/// Operações de persistência das interações com IA.
pub trait AiInteractionRepository {
    /// Insere a interação.
    fn save(&self, i: &AiInteraction) -> Result<(), StorageError>;
    /// Lista até `limite` interações, da mais recente para a mais antiga.
    fn list(&self, limite: usize) -> Result<Vec<AiInteraction>, StorageError>;
    /// Busca uma interação pelo identificador.
    fn get(&self, id: &str) -> Result<Option<AiInteraction>, StorageError>;
}

const COLUNAS: &str = "id, provider_profile_id, connection_fingerprint, task_type, prompt_text, \
     context_summary, response_text, accepted_sql, created_at";

impl AiInteractionRepository for crate::Db {
    fn save(&self, i: &AiInteraction) -> Result<(), StorageError> {
        if i.id.trim().is_empty() {
            return Err(StorageError::Invalid(
                "id da interação é obrigatório".into(),
            ));
        }
        if i.task_type.trim().is_empty() {
            return Err(StorageError::Invalid(
                "task_type da interação é obrigatório".into(),
            ));
        }
        self.conn().execute(
            "INSERT OR REPLACE INTO ai_interactions (id, provider_profile_id, \
                 connection_fingerprint, task_type, prompt_text, context_summary, response_text, \
                 accepted_sql, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                i.id,
                i.provider_profile_id,
                i.connection_fingerprint,
                i.task_type,
                i.prompt_text,
                i.context_summary,
                i.response_text,
                i.accepted_sql,
                i.created_at,
            ],
        )?;
        Ok(())
    }

    fn list(&self, limite: usize) -> Result<Vec<AiInteraction>, StorageError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUNAS} FROM ai_interactions ORDER BY created_at DESC, id DESC LIMIT ?1"
        ))?;
        let rows = stmt.query_map([limite as i64], linha_para_interacao)?;
        let mut out = Vec::new();
        for linha in rows {
            out.push(linha?);
        }
        Ok(out)
    }

    fn get(&self, id: &str) -> Result<Option<AiInteraction>, StorageError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUNAS} FROM ai_interactions WHERE id = ?1"
        ))?;
        Ok(stmt.query_row([id], linha_para_interacao).optional()?)
    }
}

fn linha_para_interacao(linha: &rusqlite::Row<'_>) -> rusqlite::Result<AiInteraction> {
    Ok(AiInteraction {
        id: linha.get(0)?,
        provider_profile_id: linha.get(1)?,
        connection_fingerprint: linha.get(2)?,
        task_type: linha.get(3)?,
        prompt_text: linha.get(4)?,
        context_summary: linha.get(5)?,
        response_text: linha.get(6)?,
        accepted_sql: linha.get(7)?,
        created_at: linha.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interacao(id: &str) -> AiInteraction {
        AiInteraction {
            id: id.to_string(),
            provider_profile_id: Some("ia-1".to_string()),
            connection_fingerprint: Some("fp-abc".to_string()),
            task_type: "gerar_sql".to_string(),
            prompt_text: "liste os clientes".to_string(),
            context_summary: "TABELA CLIENTES(ID:INTEGER)".to_string(),
            response_text: "SELECT FIRST 10 ID FROM CLIENTES".to_string(),
            accepted_sql: None,
            created_at: "2026-10-16T12:00:00Z".to_string(),
        }
    }

    fn db() -> crate::Db {
        let db = crate::Db::open_in_memory().expect("banco em memória");
        db.migrate().expect("migrações");
        db
    }

    #[test]
    fn salva_lista_e_busca_interacao() {
        let db = db();
        db.save(&interacao("i1")).expect("salvando");
        db.save(&interacao("i2")).expect("salvando");

        let todas = db.list(10).expect("listando");
        assert_eq!(todas.len(), 2);
        assert!(db.get("i1").expect("buscando").is_some());
        assert!(db.get("inexistente").expect("buscando").is_none());
    }

    #[test]
    fn limite_corta_a_listagem() {
        let db = db();
        db.save(&interacao("i1")).expect("salvando");
        db.save(&interacao("i2")).expect("salvando");
        assert_eq!(db.list(1).expect("listando").len(), 1);
    }

    #[test]
    fn recusa_interacao_sem_id_ou_tarefa() {
        let db = db();
        let mut sem_id = interacao("");
        assert!(db.save(&sem_id).is_err());

        sem_id.id = "i3".to_string();
        sem_id.task_type = String::new();
        assert!(db.save(&sem_id).is_err());
    }

    #[test]
    fn debug_redige_credencial_no_prompt() {
        let mut i = interacao("i4");
        i.prompt_text = "use password=segredo123 para conectar".to_string();
        let texto = format!("{i:?}");
        assert!(!texto.contains("segredo123"), "prompt redigido: {texto}");
    }
}
