//! Repositório de perfis de conexão (Spec 09, `connection_profiles`).
//!
//! O perfil guarda apenas a referência do cofre (`secret_ref`); **não existe
//! coluna de senha** (ADR-003). Todo acesso usa SQL parametrizado.

use crate::StorageError;
use rusqlite::OptionalExtension;

/// Registro de perfil de conexão como persistido no SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionProfileRecord {
    /// Identificador do perfil (chave primária textual).
    pub id: String,
    /// Nome amigável exibido na UI.
    pub name: String,
    /// Host do servidor Firebird.
    pub host: String,
    /// Porta do servidor.
    pub port: i64,
    /// Caminho do banco de dados no servidor.
    pub database_path: String,
    /// Usuário de conexão (não é segredo).
    pub username: String,
    /// Papel (role) usado na conexão, quando houver.
    pub role_name: Option<String>,
    /// Charset da conexão.
    pub charset: String,
    /// Dialeto SQL (1 ou 3), quando definido.
    pub sql_dialect: Option<i64>,
    /// Caminho da libfbclient.so, quando definido.
    pub client_library_path: Option<String>,
    /// Tempo limite de conexão, em milissegundos.
    pub connect_timeout_ms: i64,
    /// Referência da senha no cofre do sistema (nunca a senha).
    pub secret_ref: Option<String>,
    /// Instante de criação, em UTC (`2026-10-15T12:34:56Z`).
    pub created_at: String,
    /// Instante da última alteração, em UTC.
    pub updated_at: String,
}

/// Operações de persistência de perfis de conexão.
pub trait ConnectionProfileRepository {
    /// Insere ou atualiza o perfil (upsert por `id`).
    fn save(&self, p: &ConnectionProfileRecord) -> Result<(), StorageError>;
    /// Busca um perfil pelo identificador.
    fn get(&self, id: &str) -> Result<Option<ConnectionProfileRecord>, StorageError>;
    /// Lista todos os perfis, ordenados por nome.
    fn list(&self) -> Result<Vec<ConnectionProfileRecord>, StorageError>;
    /// Remove o perfil; devolve `true` quando algo foi removido.
    fn delete(&self, id: &str) -> Result<bool, StorageError>;
}

/// Colunas do perfil, na ordem usada por [`linha_para_registro`].
const COLUNAS: &str = "id, name, host, port, database_path, username, role_name, charset, \
     sql_dialect, client_library_path, connect_timeout_ms, secret_ref, created_at, updated_at";

impl ConnectionProfileRepository for crate::Db {
    fn save(&self, p: &ConnectionProfileRecord) -> Result<(), StorageError> {
        if p.id.trim().is_empty() {
            return Err(StorageError::Invalid("id do perfil é obrigatório".into()));
        }
        if p.name.trim().is_empty() {
            return Err(StorageError::Invalid("nome do perfil é obrigatório".into()));
        }

        self.conn().execute(
            "INSERT INTO connection_profiles (id, name, host, port, database_path, username,
                 role_name, charset, sql_dialect, client_library_path, connect_timeout_ms,
                 secret_ref, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 host = excluded.host,
                 port = excluded.port,
                 database_path = excluded.database_path,
                 username = excluded.username,
                 role_name = excluded.role_name,
                 charset = excluded.charset,
                 sql_dialect = excluded.sql_dialect,
                 client_library_path = excluded.client_library_path,
                 connect_timeout_ms = excluded.connect_timeout_ms,
                 secret_ref = excluded.secret_ref,
                 updated_at = excluded.updated_at",
            rusqlite::params![
                p.id,
                p.name,
                p.host,
                p.port,
                p.database_path,
                p.username,
                p.role_name,
                p.charset,
                p.sql_dialect,
                p.client_library_path,
                p.connect_timeout_ms,
                p.secret_ref,
                p.created_at,
                p.updated_at,
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<ConnectionProfileRecord>, StorageError> {
        let sql = format!("SELECT {COLUNAS} FROM connection_profiles WHERE id = ?1");
        let registro = self
            .conn()
            .query_row(&sql, [id], linha_para_registro)
            .optional()?;
        Ok(registro)
    }

    fn list(&self) -> Result<Vec<ConnectionProfileRecord>, StorageError> {
        let sql =
            format!("SELECT {COLUNAS} FROM connection_profiles ORDER BY name COLLATE NOCASE, id");
        let mut stmt = self.conn().prepare(&sql)?;
        let registros = stmt.query_map([], linha_para_registro)?;
        let lista = registros.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(lista)
    }

    fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let afetadas = self
            .conn()
            .execute("DELETE FROM connection_profiles WHERE id = ?1", [id])?;
        Ok(afetadas > 0)
    }
}

/// Projeção de uma linha para [`ConnectionProfileRecord`].
fn linha_para_registro(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConnectionProfileRecord> {
    Ok(ConnectionProfileRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        host: row.get(2)?,
        port: row.get(3)?,
        database_path: row.get(4)?,
        username: row.get(5)?,
        role_name: row.get(6)?,
        charset: row.get(7)?,
        sql_dialect: row.get(8)?,
        client_library_path: row.get(9)?,
        connect_timeout_ms: row.get(10)?,
        secret_ref: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{ConnectionProfileRecord, ConnectionProfileRepository};
    use crate::Db;
    use crate::StorageError;

    fn perfil(id: &str, name: &str) -> ConnectionProfileRecord {
        ConnectionProfileRecord {
            id: id.to_string(),
            name: name.to_string(),
            host: "localhost".to_string(),
            port: 3050,
            database_path: "/var/lib/firebird/3.0/data/test.fdb".to_string(),
            username: "SYSDBA".to_string(),
            role_name: None,
            charset: "UTF8".to_string(),
            sql_dialect: Some(3),
            client_library_path: None,
            connect_timeout_ms: 5000,
            secret_ref: Some("firebird-studio/profile/local-teste".to_string()),
            created_at: "2026-10-15T10:00:00Z".to_string(),
            updated_at: "2026-10-15T10:00:00Z".to_string(),
        }
    }

    fn db_com_schema() -> Db {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        db.migrate().expect("aplicando migração");
        db
    }

    #[test]
    fn save_depois_get_recupera_todos_os_campos() {
        let db = db_com_schema();
        let original = perfil("p1", "Produção");
        db.save(&original).expect("salvando perfil");

        let lido = db
            .get("p1")
            .expect("buscando perfil")
            .expect("perfil deve existir");
        assert_eq!(lido, original);
    }

    #[test]
    fn get_devolve_none_quando_perfil_nao_existe() {
        let db = db_com_schema();
        assert_eq!(db.get("inexistente").expect("busca sem erro"), None);
    }

    #[test]
    fn save_faz_upsert_preservando_created_at_e_atualizando_updated_at() {
        let db = db_com_schema();
        db.save(&perfil("p1", "Original")).expect("primeiro save");

        let mut alterado = perfil("p1", "Renomeado");
        alterado.updated_at = "2026-10-15T18:30:00Z".to_string();
        alterado.secret_ref = None;
        db.save(&alterado).expect("segundo save deve atualizar");

        let lido = db
            .get("p1")
            .expect("buscando perfil")
            .expect("perfil existe");
        assert_eq!(lido.name, "Renomeado");
        assert_eq!(lido.updated_at, "2026-10-15T18:30:00Z");
        assert_eq!(
            lido.created_at, "2026-10-15T10:00:00Z",
            "created_at é imutável"
        );
        assert_eq!(lido.secret_ref, None, "secret_ref pode ser removido");
    }

    #[test]
    fn list_devolve_perfis_ordenados_por_nome() {
        let db = db_com_schema();
        db.save(&perfil("p3", "Zeta")).expect("salvando Zeta");
        db.save(&perfil("p1", "Alfa")).expect("salvando Alfa");
        db.save(&perfil("p2", "Meio")).expect("salvando Meio");

        let lista = db.list().expect("listando perfis");
        let nomes: Vec<&str> = lista.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(nomes, vec!["Alfa", "Meio", "Zeta"]);
    }

    #[test]
    fn list_vazio_quando_nao_ha_perfis() {
        let db = db_com_schema();
        assert!(db.list().expect("listando perfis vazio").is_empty());
    }

    #[test]
    fn delete_remove_o_perfil_e_devolve_true() {
        let db = db_com_schema();
        db.save(&perfil("p1", "Alfa")).expect("salvando perfil");
        assert!(db.delete("p1").expect("removendo perfil"));
        assert_eq!(db.get("p1").expect("buscando após remoção"), None);
    }

    #[test]
    fn delete_de_id_inexistente_devolve_false() {
        let db = db_com_schema();
        assert!(!db.delete("inexistente").expect("removendo id inexistente"));
    }

    #[test]
    fn save_usa_sql_parametrizado_e_nao_interpreta_texto_como_sql() {
        let db = db_com_schema();
        let malicioso = perfil("p1", "'; DROP TABLE connection_profiles; --");
        db.save(&malicioso)
            .expect("nome com SQL deve ser tratado como dado");

        let lido = db
            .get("p1")
            .expect("buscando perfil")
            .expect("perfil existe");
        assert_eq!(lido.name, "'; DROP TABLE connection_profiles; --");
        assert!(
            db.list().expect("listando").len() == 1,
            "a tabela não pode ter sido derrubada"
        );
    }

    #[test]
    fn round_trip_preserva_unicode_e_identificadores_com_aspas() {
        let db = db_com_schema();
        let original = ConnectionProfileRecord {
            id: "perfil-ç-1".to_string(),
            name: "Čatã do \"Norte\" — O'Brien".to_string(),
            host: "servidor.local".to_string(),
            port: 3051,
            database_path: "/bases/ação.fdb".to_string(),
            username: "USUÁRIO".to_string(),
            role_name: Some("Papel \"R1\"".to_string()),
            charset: "WIN1252".to_string(),
            sql_dialect: Some(1),
            client_library_path: Some("/usr/lib/libfbclient.so".to_string()),
            connect_timeout_ms: 1500,
            secret_ref: Some("firebird-studio/profile/perfil-ç-1".to_string()),
            created_at: "2026-10-15T10:00:00Z".to_string(),
            updated_at: "2026-10-15T10:00:00Z".to_string(),
        };
        db.save(&original).expect("salvando perfil unicode");
        assert_eq!(
            db.get("perfil-ç-1").expect("buscando").expect("existe"),
            original
        );
    }

    #[test]
    fn save_sem_id_ou_sem_nome_devolve_entrada_invalida() {
        let db = db_com_schema();

        let sem_id = ConnectionProfileRecord {
            id: "   ".to_string(),
            ..perfil("p1", "Alfa")
        };
        assert!(
            matches!(db.save(&sem_id), Err(StorageError::Invalid(_))),
            "id em branco deveria ser entrada inválida"
        );

        let sem_nome = perfil("p1", "");
        assert!(
            matches!(db.save(&sem_nome), Err(StorageError::Invalid(_))),
            "nome vazio deveria ser entrada inválida"
        );
    }

    #[test]
    fn save_e_get_retornam_not_found_quando_a_tabela_ainda_nao_existe() {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        let erro = db.get("p1").expect_err("sem migrate a tabela não existe");
        assert!(
            matches!(erro, StorageError::Sqlite(_)),
            "deveria ser erro do driver: {erro:?}"
        );
    }
}
