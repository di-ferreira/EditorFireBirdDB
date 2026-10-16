//! Estado do app: as dependências que os comandos Tauri recebem por
//! [`tauri::State`].
//!
//! O estado é montado uma vez, no boot. Se o armazenamento local ou o cofre
//! do sistema não inicializarem, o app **degrada**: o estado carrega o motivo
//! acionável e os comandos devolvem `CommandError` explicando o que fazer —
//! nunca `panic!` no startup e nunca um "sucesso" falso (Spec 03 §2).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use firebird_adapter::ClientLibrary;
use firebird_security::{KeyringSecretStore, SecretStore};

use crate::application::connection_manager::{AdaptadorFirebird, ConnectionManager, Connector};
use crate::commands::error::CommandError;
use crate::security::CofreIndisponivel;
use crate::storage::BancoLocal;

/// Dependências compartilhadas por todos os comandos de conexão.
pub struct AppState {
    /// Perfis persistidos no SQLite local.
    repos: Arc<BancoLocal>,
    /// Cofre de segredos (real, ou degradado com [`CofreIndisponivel`]).
    secrets: Arc<dyn SecretStore>,
    /// Gerenciador de sessões.
    manager: ConnectionManager,
    /// `false` quando o armazenamento local não abriu.
    armazenamento_disponivel: bool,
    /// `false` quando o cofre do sistema operacional não respondeu.
    cofre_disponivel: bool,
    /// Consultas em execução que podem ser canceladas por `queryId`.
    cancelamentos: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Avisos de degradação, prontos para exibição.
    avisos: Vec<String>,
    /// Biblioteca cliente escolhida no boot (quando há mais de uma).
    biblioteca_escolhida: Option<String>,
}

impl AppState {
    /// Monta o estado real: banco em `caminho` e cofre do sistema.
    ///
    /// Nenhuma das duas falhas é fatal — o que falha vira aviso e o comando
    /// correspondente explica o motivo ao usuário.
    pub fn montar(caminho: Option<&Path>) -> Self {
        let (repos, armazenamento_disponivel, mut avisos) = match caminho {
            Some(caminho) => match BancoLocal::abrir(caminho) {
                Ok(banco) => (Arc::new(banco), true, Vec::new()),
                Err(erro) => (
                    Arc::new(BancoLocal::em_memoria().expect("banco em memória")),
                    false,
                    vec![format!(
                        "Não foi possível abrir o armazenamento local em {}: {}. \
                         Os perfis não serão salvos nesta sessão.",
                        caminho.display(),
                        erro
                    )],
                ),
            },
            None => (
                Arc::new(BancoLocal::em_memoria().expect("banco em memória")),
                false,
                vec!["O diretório de dados do aplicativo não está disponível; \
                     os perfis não serão salvos nesta sessão."
                    .to_string()],
            ),
        };

        let (secrets, cofre_disponivel) = match KeyringSecretStore::new() {
            Ok(cofre) if cofre.is_available() => (Arc::new(cofre) as Arc<dyn SecretStore>, true),
            Ok(_) => (
                cofre_indisponivel(
                    "o serviço de segredos do sistema não respondeu à verificação de \
                     disponibilidade",
                ),
                false,
            ),
            Err(erro) => {
                avisos.push(format!(
                    "Sem cofre de segredos: {}. A senha só poderá ser usada na \
                     conexão em que for informada.",
                    erro
                ));
                (cofre_indisponivel(erro.to_string()), false)
            }
        };

        let manager = ConnectionManager::com_banco(
            repos.clone(),
            secrets.clone(),
            Arc::new(AdaptadorFirebird::new()),
        );

        Self {
            repos,
            secrets,
            manager,
            armazenamento_disponivel,
            cofre_disponivel,
            avisos,
            cancelamentos: Mutex::new(HashMap::new()),
            biblioteca_escolhida: melhor_biblioteca(),
        }
    }

    /// Monta o estado com dependências injetadas e o conector real.
    pub fn com_dependencias(
        repos: Arc<BancoLocal>,
        secrets: Arc<dyn SecretStore>,
        cofre_disponivel: bool,
    ) -> Self {
        Self::com_conector(
            repos,
            secrets,
            cofre_disponivel,
            Arc::new(AdaptadorFirebird::new()),
        )
    }

    /// Monta o estado com todas as dependências injetadas, inclusive o
    /// conector — é o ponto único por onde os testes trocam o driver real
    /// pelo duplo sintético.
    pub fn com_conector(
        repos: Arc<BancoLocal>,
        secrets: Arc<dyn SecretStore>,
        cofre_disponivel: bool,
        connector: Arc<dyn Connector>,
    ) -> Self {
        let manager = ConnectionManager::com_banco(repos.clone(), secrets.clone(), connector);
        Self {
            repos,
            secrets,
            manager,
            armazenamento_disponivel: true,
            cofre_disponivel,
            avisos: Vec::new(),
            cancelamentos: Mutex::new(HashMap::new()),
            biblioteca_escolhida: melhor_biblioteca(),
        }
    }

    /// Gerenciador de sessões.
    pub fn manager(&self) -> &ConnectionManager {
        &self.manager
    }

    /// Perfis persistidos.
    pub fn repos(&self) -> &Arc<BancoLocal> {
        &self.repos
    }

    /// Cofre de segredos em uso.
    pub fn secrets(&self) -> &Arc<dyn SecretStore> {
        &self.secrets
    }

    /// `true` quando o cofre do sistema respondeu.
    pub fn cofre_disponivel(&self) -> bool {
        self.cofre_disponivel
    }

    /// `true` quando o armazenamento local abriu.
    pub fn armazenamento_disponivel(&self) -> bool {
        self.armazenamento_disponivel
    }

    /// Biblioteca cliente preferida descoberta no boot.
    pub fn biblioteca_escolhida(&self) -> Option<&str> {
        self.biblioteca_escolhida.as_deref()
    }

    /// Avisos de degradação acumulados no boot.
    pub fn avisos(&self) -> &[String] {
        &self.avisos
    }

    /// Registra uma consulta em execução e devolve a bandeira de cancelamento.
    ///
    /// A entrada é removida quando a consulta termina (sucesso ou erro), então
    /// o mapa só guarda consultas de fato em voo.
    pub fn registrar_cancelamento(&self, query_id: &str) -> Arc<AtomicBool> {
        let bandeira = Arc::new(AtomicBool::new(false));
        if let Ok(mut mapa) = self.cancelamentos.lock() {
            mapa.insert(query_id.to_string(), bandeira.clone());
        }
        bandeira
    }

    /// Remove a consulta do registro (fim da execução).
    pub fn encerrar_cancelamento(&self, query_id: &str) {
        if let Ok(mut mapa) = self.cancelamentos.lock() {
            mapa.remove(query_id);
        }
    }

    /// Pede o cancelamento de uma consulta em execução.
    ///
    /// Devolve `false` quando não há consulta viva com esse `queryId` — a UI
    /// trata como "já terminou" em vez de erro.
    pub fn cancelar(&self, query_id: &str) -> bool {
        match self.cancelamentos.lock() {
            Ok(mapa) => match mapa.get(query_id) {
                Some(bandeira) => {
                    bandeira.store(true, std::sync::atomic::Ordering::Relaxed);
                    true
                }
                None => false,
            },
            Err(_) => false,
        }
    }

    /// Falha se o armazenamento local não estiver disponível.
    ///
    /// Usada por todos os comandos que leem ou gravam perfis — inventar um
    /// "salvo com sucesso" quando não houve gravação seria pior que erro.
    pub fn exigir_armazenamento(&self) -> Result<(), CommandError> {
        if self.armazenamento_disponivel {
            return Ok(());
        }
        Err(CommandError::indisponivel(
            self.avisos
                .first()
                .cloned()
                .unwrap_or_else(|| "armazenamento local indisponível".to_string()),
        ))
    }

    /// Cofre de segredos, ou erro acionável quando degradado.
    ///
    /// Só `abrir` exige isso: gravar a senha sem cofre seria impossível sem
    /// mentir para o usuário sobre onde ela ficou.
    pub fn exigir_cofre(&self) -> Result<&Arc<dyn SecretStore>, CommandError> {
        if self.cofre_disponivel {
            return Ok(&self.secrets);
        }
        // `PermissionDenied`: repetir a tentativa não resolve — o usuário
        // precisa desbloquear o cofre do sistema primeiro.
        Err(CommandError::negado(
            self.avisos
                .iter()
                .find(|aviso| aviso.contains("cofre"))
                .cloned()
                .unwrap_or_else(|| {
                    "cofre de segredos indisponível: desbloqueie o cofre do sistema \
                     e tente novamente"
                        .to_string()
                }),
        ))
    }
}

/// Cofre degradado com a mensagem já montada.
fn cofre_indisponivel(motivo: impl Into<String>) -> Arc<dyn SecretStore> {
    Arc::new(CofreIndisponivel::novo(motivo))
}

/// Melhor `libfbclient.so` encontrada no sistema (maior versão), se houver.
fn melhor_biblioteca() -> Option<String> {
    ClientLibrary::discover()
        .into_iter()
        .filter(|lib| lib.validate_arch().is_ok())
        .max_by_key(|lib| match lib.version {
            Some(v) => (v.major, v.minor, v.patch.unwrap_or(0)),
            None => (0, 0, 0),
        })
        .map(|lib| lib.path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use firebird_domain::{DadosPerfil, SqlDialect};
    use firebird_security::InMemorySecretStore;
    use firebird_storage::ConnectionProfileRepository;

    fn estado_de_teste() -> AppState {
        AppState::com_dependencias(
            Arc::new(BancoLocal::em_memoria().expect("banco em memória")),
            Arc::new(InMemorySecretStore::new()),
            true,
        )
    }

    #[test]
    fn cancelar_consulta_marca_a_bandeira_e_some_ao_encerrar() {
        let estado = AppState::com_dependencias(
            Arc::new(BancoLocal::em_memoria().expect("banco em memória")),
            Arc::new(firebird_security::InMemorySecretStore::new()),
            true,
        );

        let bandeira = estado.registrar_cancelamento("q-1");
        assert!(!bandeira.load(std::sync::atomic::Ordering::Relaxed));
        assert!(estado.cancelar("q-1"), "consulta viva deve ser cancelável");
        assert!(bandeira.load(std::sync::atomic::Ordering::Relaxed));

        estado.encerrar_cancelamento("q-1");
        assert!(
            !estado.cancelar("q-1"),
            "consulta encerrada não existe mais"
        );
    }

    #[test]
    fn estado_de_teste_tem_armazenamento_e_cofre_disponiveis() {
        let estado = estado_de_teste();
        assert!(estado.armazenamento_disponivel());
        assert!(estado.cofre_disponivel());
        assert!(estado.exigir_armazenamento().is_ok());
        assert!(estado.exigir_cofre().is_ok());
    }

    #[test]
    fn estado_degradado_exige_armazenamento_e_explica_o_motivo() {
        let cofre = Arc::new(CofreIndisponivel::novo(
            "nenhum serviço de segredo disponível nesta sessão",
        ));
        let estado = AppState::com_dependencias(
            Arc::new(BancoLocal::em_memoria().expect("banco em memória")),
            cofre,
            false,
        );
        assert!(!estado.cofre_disponivel());
        let erro = match estado.exigir_cofre() {
            Ok(_) => panic!("cofre degradado deveria falhar"),
            Err(erro) => erro,
        };
        assert!(erro.message.contains("cofre"), "mensagem acionável: {erro}");
    }

    #[test]
    fn montar_com_diretorio_inexistente_degrada_sem_panico() {
        let caminho = std::env::temp_dir().join("firebird-studio-sem-dados");
        let _ = std::fs::remove_dir_all(&caminho);
        let estado = AppState::montar(Some(&caminho.join("nested/studio.db")));

        assert!(!estado.armazenamento_disponivel());
        let erro = estado.exigir_armazenamento().expect_err("degradação");
        assert!(
            erro.message.contains("perfis não serão salvos"),
            "a mensagem deve orientar o usuário: {erro}"
        );
        assert!(!estado.avisos().is_empty());
    }

    #[test]
    fn montar_sem_diretorio_de_dados_degrada_sem_panico() {
        let estado = AppState::montar(None);
        assert!(!estado.armazenamento_disponivel());
        assert!(estado.exigir_armazenamento().is_err());
    }

    #[test]
    fn montar_usa_o_caminho_informado_quando_existe() {
        let dir = std::env::temp_dir().join(format!("firebird-studio-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("criando diretório");
        let caminho = dir.join("studio.db");
        let estado = AppState::montar(Some(&caminho));
        assert!(estado.armazenamento_disponivel(), "{:?}", estado.avisos());
        assert!(caminho.exists(), "o arquivo do banco deve existir");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn montar_sem_entrar_em_panico_quando_o_cofre_do_sistema_falta() {
        // Em CI/headless o cofre não existe: o app tem de continuar com aviso.
        let estado = AppState::montar(None);
        let _ = estado.cofre_disponivel();
        let _ = estado.exigir_cofre();
    }

    #[test]
    fn manager_do_estado_persiste_perfil_recem_criado() {
        use firebird_domain::ConnectionProfile;

        let estado = estado_de_teste();
        let perfil = estado
            .manager()
            .salvar(DadosPerfil {
                id: String::new(),
                name: "Produção".to_string(),
                host: "localhost".to_string(),
                port: 3050,
                database_path: "/dados/app.fdb".to_string(),
                username: "SYSDBA".to_string(),
                role_name: None,
                charset: "UTF8".to_string(),
                sql_dialect: Some(SqlDialect::V3),
                client_library_path: None,
                connect_timeout_ms: 5_000,
            })
            .expect("salvando perfil");

        assert_eq!(
            estado
                .repos()
                .get(perfil.id())
                .expect("buscando")
                .expect("perfil")
                .name,
            "Produção"
        );
        assert_eq!(
            estado.manager().listar().expect("listando").len(),
            1,
            "o gerenciador enxerga o que salvou"
        );
        assert_eq!(ConnectionProfile::name(&perfil), "Produção");
    }
}
