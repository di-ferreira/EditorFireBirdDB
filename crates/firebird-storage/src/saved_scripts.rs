//! Repositório de scripts SQL salvos (Spec 09, `saved_scripts`).
//!
//! O conteúdo do script é texto do usuário e pode conter credenciais escritas
//! por engano; por isso o `Debug` de [`SavedScript`] é redigido (FR-016).

use crate::StorageError;
use rusqlite::OptionalExtension;

/// Um script SQL salvo localmente.
#[derive(Clone, PartialEq, Eq)]
pub struct SavedScript {
    /// Identificador do script.
    pub id: String,
    /// Nome do script.
    pub name: String,
    /// Conteúdo SQL.
    pub content: String,
    /// Perfil associado, quando houver.
    pub connection_profile_id: Option<String>,
    /// Instante de criação, em UTC (`2026-10-15T12:34:56Z`).
    pub created_at: String,
    /// Instante da última alteração, em UTC.
    pub updated_at: String,
}

/// O `Debug` é manual para não vazar credenciais digitadas no script (FR-016).
impl std::fmt::Debug for SavedScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavedScript")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("content", &crate::redact::redact_secrets(&self.content))
            .field("connection_profile_id", &self.connection_profile_id)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

/// Operações de persistência de scripts salvos.
pub trait SavedScriptRepository {
    /// Insere ou atualiza o script (upsert por `id`).
    fn save(&self, s: &SavedScript) -> Result<(), StorageError>;
    /// Lista todos os scripts, ordenados por nome.
    fn list(&self) -> Result<Vec<SavedScript>, StorageError>;
    /// Busca um script pelo identificador.
    fn get(&self, id: &str) -> Result<Option<SavedScript>, StorageError>;
    /// Remove o script; devolve `true` quando algo foi removido.
    fn delete(&self, id: &str) -> Result<bool, StorageError>;
}

/// Colunas do script, na ordem usada por [`linha_para_script`].
const COLUNAS: &str = "id, name, content, connection_profile_id, created_at, updated_at";

impl SavedScriptRepository for crate::Db {
    fn save(&self, s: &SavedScript) -> Result<(), StorageError> {
        if s.id.trim().is_empty() {
            return Err(StorageError::Invalid("id do script é obrigatório".into()));
        }
        if s.name.trim().is_empty() {
            return Err(StorageError::Invalid("nome do script é obrigatório".into()));
        }

        self.conn().execute(
            "INSERT INTO saved_scripts (id, name, content, connection_profile_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 content = excluded.content,
                 connection_profile_id = excluded.connection_profile_id,
                 updated_at = excluded.updated_at",
            rusqlite::params![
                s.id,
                s.name,
                s.content,
                s.connection_profile_id,
                s.created_at,
                s.updated_at,
            ],
        )?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<SavedScript>, StorageError> {
        let sql = format!("SELECT {COLUNAS} FROM saved_scripts ORDER BY name COLLATE NOCASE, id");
        let mut stmt = self.conn().prepare(&sql)?;
        let scripts = stmt.query_map([], linha_para_script)?;
        let lista = scripts.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(lista)
    }

    fn get(&self, id: &str) -> Result<Option<SavedScript>, StorageError> {
        let sql = format!("SELECT {COLUNAS} FROM saved_scripts WHERE id = ?1");
        let script = self
            .conn()
            .query_row(&sql, [id], linha_para_script)
            .optional()?;
        Ok(script)
    }

    fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let afetadas = self
            .conn()
            .execute("DELETE FROM saved_scripts WHERE id = ?1", [id])?;
        Ok(afetadas > 0)
    }
}

/// Projeção de uma linha para [`SavedScript`].
fn linha_para_script(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedScript> {
    Ok(SavedScript {
        id: row.get(0)?,
        name: row.get(1)?,
        content: row.get(2)?,
        connection_profile_id: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{SavedScript, SavedScriptRepository};
    use crate::{Db, StorageError};

    fn script(id: &str, name: &str) -> SavedScript {
        SavedScript {
            id: id.to_string(),
            name: name.to_string(),
            content: "SELECT * FROM RDB$RELATIONS;".to_string(),
            connection_profile_id: Some("p1".to_string()),
            created_at: "2026-10-15T09:00:00Z".to_string(),
            updated_at: "2026-10-15T09:00:00Z".to_string(),
        }
    }

    fn db_com_schema() -> Db {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        db.migrate().expect("aplicando migração");
        db
    }

    #[test]
    fn save_depois_get_recupera_o_script_completo() {
        let db = db_com_schema();
        let original = script("s1", "Backup diário");
        db.save(&original).expect("salvando script");

        let lido = db
            .get("s1")
            .expect("buscando script")
            .expect("script deve existir");
        assert_eq!(lido, original);
    }

    #[test]
    fn get_devolve_none_quando_script_nao_existe() {
        let db = db_com_schema();
        assert_eq!(db.get("inexistente").expect("busca sem erro"), None);
    }

    #[test]
    fn save_faz_upsert_atualizando_conteudo_e_updated_at() {
        let db = db_com_schema();
        db.save(&script("s1", "Original")).expect("primeiro save");

        let mut alterado = script("s1", "Corrigido");
        alterado.content = "SELECT FIRST 10 * FROM RDB$DATABASE;".to_string();
        alterado.updated_at = "2026-10-15T20:00:00Z".to_string();
        db.save(&alterado).expect("segundo save deve atualizar");

        let lido = db.get("s1").expect("buscando").expect("existe");
        assert_eq!(lido.content, "SELECT FIRST 10 * FROM RDB$DATABASE;");
        assert_eq!(lido.updated_at, "2026-10-15T20:00:00Z");
        assert_eq!(
            lido.created_at, "2026-10-15T09:00:00Z",
            "created_at é imutável"
        );
    }

    #[test]
    fn list_devolve_scripts_ordenados_por_nome() {
        let db = db_com_schema();
        db.save(&script("s3", "Zeta")).expect("save");
        db.save(&script("s1", "Alfa")).expect("save");
        db.save(&script("s2", "Meio")).expect("save");

        let lista = db.list().expect("listando scripts");
        let nomes: Vec<&str> = lista.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(nomes, vec!["Alfa", "Meio", "Zeta"]);
    }

    #[test]
    fn list_vazio_quando_nao_ha_scripts() {
        let db = db_com_schema();
        assert!(db.list().expect("listando vazio").is_empty());
    }

    #[test]
    fn delete_remove_o_script_e_devolve_true() {
        let db = db_com_schema();
        db.save(&script("s1", "Alfa")).expect("save");
        assert!(db.delete("s1").expect("removendo script"));
        assert_eq!(db.get("s1").expect("buscando após remoção"), None);
    }

    #[test]
    fn delete_de_id_inexistente_devolve_false() {
        let db = db_com_schema();
        assert!(!db.delete("inexistente").expect("removendo id inexistente"));
    }

    #[test]
    fn round_trip_preserva_unicode_e_aspas_no_conteudo() {
        let db = db_com_schema();
        let original = SavedScript {
            id: "script-ção".to_string(),
            name: "Čatã \"O'Brien\"".to_string(),
            content: "SELECT 'ação — çãõ' AS \"Çampo\"\nFROM RDB$DATABASE;".to_string(),
            connection_profile_id: None,
            created_at: "2026-10-15T09:00:00Z".to_string(),
            updated_at: "2026-10-15T09:00:00Z".to_string(),
        };
        db.save(&original).expect("salvando script unicode");
        assert_eq!(
            db.get("script-ção").expect("buscando").expect("existe"),
            original
        );
    }

    #[test]
    fn save_sem_id_ou_sem_nome_devolve_entrada_invalida() {
        let db = db_com_schema();

        let sem_id = SavedScript {
            id: String::new(),
            ..script("s1", "Alfa")
        };
        assert!(
            matches!(db.save(&sem_id), Err(StorageError::Invalid(_))),
            "id vazio deveria ser entrada inválida"
        );

        let sem_nome = script("s1", "  ");
        assert!(
            matches!(db.save(&sem_nome), Err(StorageError::Invalid(_))),
            "nome em branco deveria ser entrada inválida"
        );
    }

    #[test]
    fn save_usa_sql_parametrizado_e_nao_interpreta_conteudo_como_sql() {
        let db = db_com_schema();
        let malicioso = SavedScript {
            content: "'; DROP TABLE saved_scripts; --".to_string(),
            ..script("s1", "Alfa")
        };
        db.save(&malicioso).expect("conteúdo com SQL deve ser dado");

        let lista = db.list().expect("listando");
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].content, "'; DROP TABLE saved_scripts; --");
    }

    #[test]
    fn debug_do_script_nao_expoe_segredo_escrito_no_conteudo() {
        let s = SavedScript {
            content: "CONNECT localhost/3050:db user=SYSDBA password=segredo;".to_string(),
            ..script("s1", "Alfa")
        };
        let texto = format!("{s:?}");
        assert!(
            !texto.contains("segredo"),
            "a senha não pode aparecer no Debug do script: {texto}"
        );
        assert!(texto.contains("password=***"), "{texto}");
    }
}
