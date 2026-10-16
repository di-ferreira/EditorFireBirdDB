//! Repositório de perfis de provedor de IA (Spec 09, `ai_provider_profiles`).
//!
//! **A chave da API nunca é gravada aqui**: a coluna `secret_ref` aponta para o
//! cofre do sistema (ADR-003). O `Debug` do perfil imprime o `secret_ref` — que
//! é um identificador opaco — e nunca um segredo (FR-016).

use crate::StorageError;
use rusqlite::OptionalExtension;

/// Um provedor de IA configurado localmente.
#[derive(Clone, PartialEq, Eq)]
pub struct AiProviderProfile {
    /// Identificador do perfil.
    pub id: String,
    /// Nome exibido na UI.
    pub name: String,
    /// Tipo do provedor (`openai_compat`, …).
    pub provider_type: String,
    /// URL base da API (Ollama/LM Studio/OpenAI-compatíveis).
    pub base_url: String,
    /// Modelo usado nas requisições.
    pub model_name: String,
    /// Referência no cofre do sistema (chave da API mora lá, nunca aqui).
    pub secret_ref: Option<String>,
    /// Timeout da chamada, em milissegundos.
    pub timeout_ms: i64,
    /// `true` quando o usuário autoriza enviar contexto a este provedor remoto.
    pub remote_context_confirmation: bool,
    /// Criação em UTC.
    pub created_at: String,
    /// Última alteração em UTC.
    pub updated_at: String,
}

impl std::fmt::Debug for AiProviderProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiProviderProfile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("provider_type", &self.provider_type)
            .field("base_url", &self.base_url)
            .field("model_name", &self.model_name)
            .field("secret_ref", &self.secret_ref)
            .field("timeout_ms", &self.timeout_ms)
            .field(
                "remote_context_confirmation",
                &self.remote_context_confirmation,
            )
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

/// Operações de persistência dos perfis de provedor de IA.
pub trait AiProviderProfileRepository {
    /// Insere ou atualiza o perfil (upsert por `id`).
    fn save(&self, p: &AiProviderProfile) -> Result<(), StorageError>;
    /// Lista todos os perfis, ordenados por nome.
    fn list(&self) -> Result<Vec<AiProviderProfile>, StorageError>;
    /// Busca um perfil pelo identificador.
    fn get(&self, id: &str) -> Result<Option<AiProviderProfile>, StorageError>;
    /// Remove o perfil; `true` quando algo foi removido.
    fn delete(&self, id: &str) -> Result<bool, StorageError>;
}

const COLUNAS: &str = "id, name, provider_type, base_url, model_name, secret_ref, timeout_ms, \
     remote_context_confirmation, created_at, updated_at";

impl AiProviderProfileRepository for crate::Db {
    fn save(&self, p: &AiProviderProfile) -> Result<(), StorageError> {
        if p.id.trim().is_empty() {
            return Err(StorageError::Invalid(
                "id do provedor de IA é obrigatório".into(),
            ));
        }
        if p.name.trim().is_empty() {
            return Err(StorageError::Invalid(
                "nome do provedor de IA é obrigatório".into(),
            ));
        }
        if p.base_url.trim().is_empty() {
            return Err(StorageError::Invalid(
                "base_url do provedor de IA é obrigatório".into(),
            ));
        }
        if p.model_name.trim().is_empty() {
            return Err(StorageError::Invalid(
                "modelo do provedor de IA é obrigatório".into(),
            ));
        }

        self.conn().execute(
            "INSERT OR REPLACE INTO ai_provider_profiles (id, name, provider_type, base_url, \
                 model_name, secret_ref, timeout_ms, remote_context_confirmation, created_at, \
                 updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                p.id,
                p.name,
                p.provider_type,
                p.base_url,
                p.model_name,
                p.secret_ref,
                p.timeout_ms,
                i64::from(p.remote_context_confirmation),
                p.created_at,
                p.updated_at,
            ],
        )?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<AiProviderProfile>, StorageError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUNAS} FROM ai_provider_profiles ORDER BY name"
        ))?;
        let rows = stmt.query_map([], linha_para_perfil)?;
        let mut out = Vec::new();
        for linha in rows {
            out.push(linha?);
        }
        Ok(out)
    }

    fn get(&self, id: &str) -> Result<Option<AiProviderProfile>, StorageError> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUNAS} FROM ai_provider_profiles WHERE id = ?1"
        ))?;
        let linha = stmt.query_row([id], linha_para_perfil).optional()?;
        Ok(linha)
    }

    fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let afetadas = self
            .conn()
            .execute("DELETE FROM ai_provider_profiles WHERE id = ?1", [id])?;
        Ok(afetadas > 0)
    }
}

fn linha_para_perfil(linha: &rusqlite::Row<'_>) -> rusqlite::Result<AiProviderProfile> {
    Ok(AiProviderProfile {
        id: linha.get(0)?,
        name: linha.get(1)?,
        provider_type: linha.get(2)?,
        base_url: linha.get(3)?,
        model_name: linha.get(4)?,
        secret_ref: linha.get(5)?,
        timeout_ms: linha.get(6)?,
        remote_context_confirmation: linha.get::<_, i64>(7)? != 0,
        created_at: linha.get(8)?,
        updated_at: linha.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perfil(id: &str) -> AiProviderProfile {
        AiProviderProfile {
            id: id.to_string(),
            name: "Ollama local".to_string(),
            provider_type: "openai_compat".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            model_name: "qwen2.5-coder".to_string(),
            secret_ref: None,
            timeout_ms: 30_000,
            remote_context_confirmation: false,
            created_at: "2026-10-16T10:00:00Z".to_string(),
            updated_at: "2026-10-16T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn salva_lista_busca_e_apaga_perfil() {
        let db = crate::Db::open_in_memory().expect("banco em memória");
        db.migrate().expect("migrações");
        db.save(&perfil("p1")).expect("salvando");

        let todos = db.list().expect("listando");
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].model_name, "qwen2.5-coder");
        assert!(db.get("p1").expect("buscando").is_some());
        assert!(db.delete("p1").expect("apagando"));
        assert!(db.get("p1").expect("buscando").is_none());
    }

    #[test]
    fn upsert_atualiza_o_mesmo_perfil() {
        let db = crate::Db::open_in_memory().expect("banco em memória");
        db.migrate().expect("migrações");
        db.save(&perfil("p1")).expect("salvando");
        let mut atualizado = perfil("p1");
        atualizado.model_name = "llama3".to_string();
        atualizado.remote_context_confirmation = true;
        db.save(&atualizado).expect("atualizando");

        let lido = db.get("p1").expect("buscando").expect("perfil");
        assert_eq!(lido.model_name, "llama3");
        assert!(lido.remote_context_confirmation);
        assert_eq!(db.list().expect("listando").len(), 1);
    }

    #[test]
    fn recusa_perfil_sem_url_ou_sem_modelo() {
        let db = crate::Db::open_in_memory().expect("banco em memória");
        db.migrate().expect("migrações");

        let mut sem_url = perfil("p2");
        sem_url.base_url = "  ".to_string();
        assert!(db.save(&sem_url).is_err(), "base_url é obrigatória");

        let mut sem_modelo = perfil("p3");
        sem_modelo.model_name = String::new();
        assert!(db.save(&sem_modelo).is_err(), "modelo é obrigatório");
    }

    #[test]
    fn debug_nao_revela_segredo() {
        let mut p = perfil("p4");
        p.secret_ref = Some("secret-ref-opaco".to_string());
        let texto = format!("{p:?}");
        assert!(texto.contains("secret-ref-opaco"));
        assert!(!texto.contains("sk-"));
    }
}
