//! Persistência local do backend desktop (Spec 09; ADR-003).
//!
//! O crate `firebird-storage` devolve `Db`, que envolve uma
//! [`rusqlite::Connection`] — `Send`, porém **não `Sync`**. O estado do Tauri
//! exige `Send + Sync`, então o banco vive aqui atrás de um
//! [`Mutex`], que satisfaz os dois sem serializar todo o acesso das
//! threads da UI (as operações são curtas e parametrizadas).
//!
//! [`BancoLocal`] também centraliza a conversão entre o registro do storage e
//! o [`ConnectionProfile`] do domínio, para que nem os comandos Tauri nem o
//! [`ConnectionManager`] conheçam o formato do SQLite.

use std::path::Path;
use std::sync::Mutex;

use firebird_domain::{ConnectionProfile, DadosPerfil, PersistedProfile, ProfileError};
use firebird_storage::{
    AiInteraction, AiInteractionRepository, AiProviderProfile, AiProviderProfileRepository,
    ConnectionProfileRecord, ConnectionProfileRepository, Db, QueryHistoryRepository,
    SavedScriptRepository, StorageError,
};

/// Banco local de metadados, seguro para compartilhar entre threads.
pub struct BancoLocal {
    /// Conexão SQLite serializada por `Mutex` (o tipo do driver não é `Sync`).
    db: Mutex<Db>,
}

impl BancoLocal {
    /// Abre (criando o arquivo se preciso) o banco em `caminho` e aplica as
    /// migrações pendentes.
    pub fn abrir(caminho: &Path) -> Result<Self, StorageError> {
        let db = Db::open(caminho)?;
        db.migrate()?;
        Ok(Self { db: Mutex::new(db) })
    }

    /// Banco efêmero em memória, já migrado — usado em testes e como
    /// degradação explícita quando o diretório de dados não está disponível.
    pub fn em_memoria() -> Result<Self, StorageError> {
        let db = Db::open_in_memory()?;
        db.migrate()?;
        Ok(Self { db: Mutex::new(db) })
    }

    /// Trava o banco ignorando envenenamento: um panic em outra thread não
    /// pode transformar toda operação de armazenamento em panic (e o panic
    /// poderia carregar dados do usuário no log).
    fn db(&self) -> std::sync::MutexGuard<'_, Db> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Converte o registro persistido no perfil de domínio validado.
pub fn perfil_de_registro(
    registro: &ConnectionProfileRecord,
) -> Result<ConnectionProfile, ProfileError> {
    ConnectionProfile::try_from(PersistedProfile {
        id: registro.id.clone(),
        name: registro.name.clone(),
        host: registro.host.clone(),
        port: registro.port,
        database_path: registro.database_path.clone(),
        username: registro.username.clone(),
        role_name: registro.role_name.clone(),
        charset: registro.charset.clone(),
        sql_dialect: registro.sql_dialect,
        client_library_path: registro.client_library_path.clone(),
        connect_timeout_ms: registro.connect_timeout_ms,
    })
}

/// Converte um perfil validado no registro a persistir, com a referência do
/// segredo e os carimbos de tempo fornecidos pela camada de aplicação.
pub fn registro_de_perfil(
    perfil: &ConnectionProfile,
    secret_ref: Option<String>,
    created_at: String,
    updated_at: String,
) -> ConnectionProfileRecord {
    let persistido = PersistedProfile::from(perfil);
    ConnectionProfileRecord {
        id: persistido.id,
        name: persistido.name,
        host: persistido.host,
        port: persistido.port,
        database_path: persistido.database_path,
        username: persistido.username,
        role_name: persistido.role_name,
        charset: persistido.charset,
        sql_dialect: persistido.sql_dialect,
        client_library_path: persistido.client_library_path,
        connect_timeout_ms: persistido.connect_timeout_ms,
        secret_ref,
        created_at,
        updated_at,
    }
}

/// Dados de entrada de um perfil ainda sem `id` nem validação.
pub type DadosDoPerfil = DadosPerfil;

impl ConnectionProfileRepository for BancoLocal {
    fn save(&self, p: &ConnectionProfileRecord) -> Result<(), StorageError> {
        ConnectionProfileRepository::save(&*self.db(), p)
    }

    fn get(&self, id: &str) -> Result<Option<ConnectionProfileRecord>, StorageError> {
        ConnectionProfileRepository::get(&*self.db(), id)
    }

    fn list(&self) -> Result<Vec<ConnectionProfileRecord>, StorageError> {
        ConnectionProfileRepository::list(&*self.db())
    }

    fn delete(&self, id: &str) -> Result<bool, StorageError> {
        ConnectionProfileRepository::delete(&*self.db(), id)
    }
}

impl BancoLocal {
    /// Grava uma entrada no histórico de consultas.
    pub fn add_history(
        &self,
        e: &firebird_storage::QueryHistoryEntry,
    ) -> Result<(), firebird_storage::StorageError> {
        QueryHistoryRepository::add(&*self.db(), e)
    }

    /// Lista entradas recentes, da mais nova para a mais antiga (limite).
    pub fn history(
        &self,
        limite: usize,
    ) -> Result<Vec<firebird_storage::QueryHistoryEntry>, firebird_storage::StorageError> {
        QueryHistoryRepository::list(&*self.db(), limite)
    }

    /// Apaga o histórico e devolve o total removido.
    pub fn clear_history(&self) -> Result<usize, firebird_storage::StorageError> {
        QueryHistoryRepository::clear(&*self.db())
    }
}

impl BancoLocal {
    /// Época: infelizmente `Db` expõe `SavedScriptRepository`; embrulhar manualmente.
    pub fn save_script(
        &self,
        s: &firebird_storage::SavedScript,
    ) -> Result<(), firebird_storage::StorageError> {
        SavedScriptRepository::save(&*self.db(), s)
    }

    pub fn scripts(
        &self,
    ) -> Result<Vec<firebird_storage::SavedScript>, firebird_storage::StorageError> {
        SavedScriptRepository::list(&*self.db())
    }

    pub fn get_script(
        &self,
        id: &str,
    ) -> Result<Option<firebird_storage::SavedScript>, firebird_storage::StorageError> {
        SavedScriptRepository::get(&*self.db(), id)
    }

    pub fn delete_script(&self, id: &str) -> Result<bool, firebird_storage::StorageError> {
        ConnectionProfileRepository::delete(&*self.db(), id)
    }
}

impl BancoLocal {
    /// Salva (upsert) o perfil do provedor de IA.
    pub fn salvar_provedor_ia(&self, p: &AiProviderProfile) -> Result<(), StorageError> {
        AiProviderProfileRepository::save(&*self.db(), p)
    }

    /// Lista os provedores de IA configurados.
    pub fn provedores_ia(&self) -> Result<Vec<AiProviderProfile>, StorageError> {
        AiProviderProfileRepository::list(&*self.db())
    }

    /// Busca um provedor de IA pelo id.
    pub fn obter_provedor_ia(&self, id: &str) -> Result<Option<AiProviderProfile>, StorageError> {
        AiProviderProfileRepository::get(&*self.db(), id)
    }

    /// Remove um provedor de IA; `true` quando algo foi removido.
    pub fn apagar_provedor_ia(&self, id: &str) -> Result<bool, StorageError> {
        AiProviderProfileRepository::delete(&*self.db(), id)
    }
}

impl BancoLocal {
    /// Registra uma interação com a IA.
    pub fn add_interacao_ia(&self, i: &AiInteraction) -> Result<(), StorageError> {
        AiInteractionRepository::save(&*self.db(), i)
    }

    /// Lista interações recentes com a IA (mais nova primeiro).
    pub fn interacoes_ia(&self, limite: usize) -> Result<Vec<AiInteraction>, StorageError> {
        AiInteractionRepository::list(&*self.db(), limite)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use firebird_storage::agora_iso8601;

    fn perfil_valido() -> ConnectionProfile {
        ConnectionProfile::validar(DadosPerfil {
            id: "p1".to_string(),
            name: "Produção".to_string(),
            host: "servidor.local".to_string(),
            port: 3050,
            database_path: "/dados/app.fdb".to_string(),
            username: "SYSDBA".to_string(),
            role_name: Some("RDB$ADMIN".to_string()),
            charset: "UTF8".to_string(),
            sql_dialect: Some(firebird_domain::SqlDialect::V3),
            client_library_path: Some("/usr/lib/libfbclient.so.3".to_string()),
            connect_timeout_ms: 5_000,
        })
        .expect("perfil válido")
    }

    #[test]
    fn banco_em_memoria_aceita_save_get_list_delete() {
        let banco = BancoLocal::em_memoria().expect("banco em memória");
        let agora = agora_iso8601();
        banco
            .save(&registro_de_perfil(
                &perfil_valido(),
                Some("firebird-studio/profile/p1".to_string()),
                agora.clone(),
                agora,
            ))
            .expect("salvando");

        assert_eq!(banco.list().expect("listando").len(), 1);
        assert!(banco.get("p1").expect("buscando").is_some());
        assert!(banco.delete("p1").expect("removendo"));
    }

    #[test]
    fn round_trip_registro_para_perfil_preserva_unicode_e_secret_ref() {
        let banco = BancoLocal::em_memoria().expect("banco em memória");
        let agora = agora_iso8601();
        let original = ConnectionProfileRecord {
            id: "perfil-ç-1".to_string(),
            name: "Čatã do \"Norte\" — O'Brien".to_string(),
            host: "servidor.local".to_string(),
            port: 3050,
            database_path: "/bases/ação.fdb".to_string(),
            username: "USUÁRIO".to_string(),
            role_name: Some("Papel \"R1\"".to_string()),
            charset: "UTF8".to_string(),
            sql_dialect: Some(1),
            client_library_path: None,
            connect_timeout_ms: 1_500,
            secret_ref: Some("firebird-studio/profile/perfil-ç-1".to_string()),
            created_at: agora.clone(),
            updated_at: agora,
        };
        banco.save(&original).expect("salvando unicode");

        let lido = banco
            .get("perfil-ç-1")
            .expect("buscando")
            .expect("perfil existe");
        let perfil = perfil_de_registro(&lido).expect("perfil de domínio válido");

        assert_eq!(perfil.id(), "perfil-ç-1");
        assert_eq!(perfil.name(), "Čatã do \"Norte\" — O'Brien");
        assert_eq!(perfil.sql_dialect(), Some(firebird_domain::SqlDialect::V1));
        assert_eq!(
            lido.secret_ref.as_deref(),
            Some("firebird-studio/profile/perfil-ç-1"),
            "a secret_ref fica só no registro, nunca no perfil"
        );
    }

    #[test]
    fn registro_com_dialeto_invalido_vira_erro_de_perfil() {
        let agora = agora_iso8601();
        let registro = registro_de_perfil(&perfil_valido(), None, agora.clone(), agora);
        let registro = ConnectionProfileRecord {
            sql_dialect: Some(2),
            ..registro
        };
        assert_eq!(
            perfil_de_registro(&registro),
            Err(ProfileError::DialetoDesconhecido(2))
        );
    }

    #[test]
    fn banco_com_migracoes_e_sem_arquivo_em_disco() {
        let banco = BancoLocal::em_memoria().expect("banco em memória");
        assert!(banco.list().expect("listando").is_empty());
    }

    #[test]
    fn abrir_em_diretorio_inexistente_devolve_erro_sem_panico() {
        let caminho = std::env::temp_dir().join("firebird-studio-dir-inexistente");
        let _ = std::fs::remove_dir_all(&caminho);
        assert!(BancoLocal::abrir(&caminho.join("studio.db")).is_err());
    }
}
