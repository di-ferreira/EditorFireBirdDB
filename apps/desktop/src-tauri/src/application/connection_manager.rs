//! Gerenciador de sessões de conexão (Spec 03; ADR-005).
//!
//! O *Connection Manager* é o único dono do estado de sessão do app: perfil
//! salvo (persistido) e sessão ativa (em memória) são coisas distintas, e só
//! este módulo sabe transitar entre elas. Os comandos Tauri são casca fina
//! sobre ele — a lógica aqui é testada sem Tauri, sem rede e sem servidor.
//!
//! Segurança (FR-016, Spec 03 §1):
//!
//! * a senha entra **apenas** por [`ConnectionManager::abrir`], que a grava no
//!   cofre do sistema pela `secret_ref` do perfil e nunca a devolve, nem em
//!   log, panic ou DTO;
//! * a senha é lida do cofre e existindo, o [`ConnParams`] redige `Debug` e
//!   `Display`, então mesmo um `{:?}` no caminho de erro não vaza;
//! * mensagens de erro passam por
//!   [`firebird_domain::redigir_segredo`] antes de virarem status ou erro.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use firebird_adapter::{ClientLibrary, ConnParams, FirebirdAdapter, RsFbClientAdapter};
use firebird_domain::{
    redigir_segredo, ConnectionProfile, ConnectionStatus, DadosPerfil, DomainError, ServerVersion,
};
use firebird_security::{secret_ref, SecretStore, SecurityError};
use firebird_storage::{agora_iso8601, ConnectionProfileRepository, StorageError};

use crate::storage::{perfil_de_registro, registro_de_perfil, BancoLocal};

/// Adaptador de conexão aberto, mantido vivo enquanto a sessão existir.
pub type AdaptadorAberto = Box<dyn FirebirdAdapter + Send>;

/// Meios de abrir uma conexão Firebird.
///
/// Existe para que os testes do gerenciador rodem **sem servidor e sem
/// `libfbclient`**: o fake implementa este trait e devolve um adaptador
/// sintético. A implementação real é [`AdaptadorFirebird`].
pub trait Connector: Send + Sync {
    /// Abre a conexão com os parâmetros informados (a senha já está neles e
    /// nunca aparece em `Debug`/`Display`).
    fn connect(&self, params: &ConnParams) -> Result<AdaptadorAberto, DomainError>;
}

/// [`Connector`] real, sobre `rsfbclient` com carregamento dinâmico da
/// `libfbclient.so` escolhida (ADR-002).
#[derive(Debug, Default, Clone, Copy)]
pub struct AdaptadorFirebird;

impl AdaptadorFirebird {
    /// Cria o conector real (sem estado — a biblioteca é carregada por
    /// [`Connector::connect`], a partir do caminho resolvido).
    pub fn new() -> Self {
        Self
    }
}

impl Connector for AdaptadorFirebird {
    fn connect(&self, params: &ConnParams) -> Result<AdaptadorAberto, DomainError> {
        let adaptador = RsFbClientAdapter::connect(params)?;
        Ok(Box::new(adaptador))
    }
}

/// Erros do gerenciador, já sem segredo.
///
/// O `Debug` é manual e redigido: um `unwrap()` em teste ou um
/// `log!("{err:?}")` não podem levar a senha (FR-016).
#[derive(thiserror::Error)]
pub enum ErroGerente {
    /// O perfil informado não existe no armazenamento local.
    #[error("perfil de conexão não encontrado: {0}")]
    PerfilNaoEncontrado(String),

    /// O perfil é inválido (campo obrigatório, porta, timeout ou dialeto).
    #[error("perfil de conexão inválido: {0}")]
    PerfilInvalido(String),

    /// Não há senha no cofre para este perfil.
    #[error("a senha do perfil '{0}' não está guardada no cofre: informe-a ao conectar")]
    SegredoAusente(String),

    /// O cofre do sistema operacional não pôde ser usado.
    #[error("cofre indisponível: {0}")]
    CofreIndisponivel(String),

    /// Falha ao conectar ao servidor (mensagem já redigida).
    #[error("{0}")]
    Conexao(String),

    /// O armazenamento local ou o cofre não inicializou no boot.
    #[error("{0}")]
    Indisponivel(String),
}

impl std::fmt::Debug for ErroGerente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variante = match self {
            ErroGerente::PerfilNaoEncontrado(_) => "PerfilNaoEncontrado",
            ErroGerente::PerfilInvalido(_) => "PerfilInvalido",
            ErroGerente::SegredoAusente(_) => "SegredoAusente",
            ErroGerente::CofreIndisponivel(_) => "CofreIndisponivel",
            ErroGerente::Conexao(_) => "Conexao",
            ErroGerente::Indisponivel(_) => "Indisponivel",
        };
        write!(f, "{variante}({})", redigir_segredo(&self.to_string()))
    }
}

impl ErroGerente {
    /// Constrói um erro de conexão com a mensagem **já redigida**.
    fn conexao(mensagem: impl std::fmt::Display) -> Self {
        ErroGerente::Conexao(redigir_segredo(&mensagem.to_string()))
    }

    /// Constrói um erro de cofre com a mensagem **já redigida**.
    fn cofre(mensagem: impl std::fmt::Display) -> Self {
        ErroGerente::CofreIndisponivel(redigir_segredo(&mensagem.to_string()))
    }
}

impl From<StorageError> for ErroGerente {
    fn from(erro: StorageError) -> Self {
        match erro {
            StorageError::NotFound(id) => ErroGerente::PerfilNaoEncontrado(id),
            StorageError::Invalid(motivo) => ErroGerente::PerfilInvalido(motivo),
            outra => ErroGerente::Indisponivel(redigir_segredo(&outra.to_string())),
        }
    }
}

impl From<SecurityError> for ErroGerente {
    fn from(erro: SecurityError) -> Self {
        ErroGerente::cofre(erro)
    }
}

/// Estado de uma sessão mantido em memória.
struct EstadoSessao {
    /// Situação atual (Spec 03 §1).
    status: ConnectionStatus,
    /// Identificador da sessão, enquanto existir.
    session_id: Option<String>,
    /// Biblioteca cliente efetivamente carregada.
    client_library_path: Option<String>,
    /// Conexão viva; [`None`] quando não há sessão (o adapter é devolvido em
    /// [`ConnectionManager::fechar`]).
    adaptador: Option<AdaptadorAberto>,
    /// Versão do servidor reportada na abertura (informativa).
    server_version: Option<ServerVersion>,
}

/// Situação pública de um perfil, separada do perfil salvo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InformacoesStatus {
    /// Perfil consultado.
    pub profile_id: String,
    /// Situação da sessão.
    pub status: ConnectionStatus,
    /// Sessão ativa, quando houver.
    pub session_id: Option<String>,
    /// Biblioteca cliente carregada, quando houver.
    pub client_library_path: Option<String>,
}

impl InformacoesStatus {
    /// Mensagem do estado de erro, quando houver.
    pub fn mensagem(&self) -> Option<&str> {
        self.status.mensagem()
    }
}

/// Resultado de `connection_open`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessaoAberta {
    /// Perfil conectado.
    pub profile_id: String,
    /// Identificador da sessão criada.
    pub session_id: String,
    /// Situação resultante (sempre [`ConnectionStatus::Connected`] no sucesso).
    pub status: ConnectionStatus,
    /// Biblioteca cliente efetivamente carregada (Spec 03 §2).
    pub client_library_path: Option<String>,
    /// Versão do servidor, quando o driver informou.
    pub server_version: Option<String>,
}

/// Resultado do teste de conexão, que **não** persiste nada (Spec 03 §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TesteConexao {
    /// `true` quando a conexão de teste foi estabelecida.
    pub ok: bool,
    /// Biblioteca cliente efetivamente carregada.
    pub client_library_path: Option<String>,
    /// Versão do servidor informada.
    pub server_version: Option<String>,
    /// Detalhe legível (falha ou observação), sempre redigido.
    pub message: Option<String>,
}

/// Dono do estado de sessão e das operações de conexão.
pub struct ConnectionManager {
    /// Perfis persistidos (SQLite local).
    repos: Arc<dyn ConnectionProfileRepository + Send + Sync>,
    /// Cofre de segredos do sistema operacional.
    secrets: Arc<dyn SecretStore>,
    /// Meio de abrir conexão (real ou falso, para teste).
    connector: Arc<dyn Connector>,
    /// Sessões vivas, por `profile_id`.
    sessoes: Mutex<HashMap<String, EstadoSessao>>,
}

impl ConnectionManager {
    /// Monta o gerenciador com seus colaboradores.
    pub fn novo(
        repos: Arc<dyn ConnectionProfileRepository + Send + Sync>,
        secrets: Arc<dyn SecretStore>,
        connector: Arc<dyn Connector>,
    ) -> Self {
        Self {
            repos,
            secrets,
            connector,
            sessoes: Mutex::new(HashMap::new()),
        }
    }

    /// Monta o gerenciador sobre um banco já aberto e o cofre informado.
    pub fn com_banco(
        banco: Arc<BancoLocal>,
        secrets: Arc<dyn SecretStore>,
        connector: Arc<dyn Connector>,
    ) -> Self {
        Self::novo(banco, secrets, connector)
    }

    /// Salva (cria ou atualiza) um perfil.
    ///
    /// `dados.id` vazio cria um perfil novo com id gerado; com id preenchido,
    /// o perfil precisa **existir** — editar um id desconhecido é
    /// [`ErroGerente::PerfilNaoEncontrado`], nunca inserção acidental. O
    /// `created_at` original é preservado na edição.
    pub fn salvar(&self, dados: DadosPerfil) -> Result<ConnectionProfile, ErroGerente> {
        let agora = agora_iso8601();
        let existente = if dados.id.trim().is_empty() {
            None
        } else {
            self.repos.get(&dados.id)?
        };
        if !dados.id.trim().is_empty() && existente.is_none() {
            return Err(ErroGerente::PerfilNaoEncontrado(dados.id.clone()));
        }

        let id = match &existente {
            Some(reg) => reg.id.clone(),
            None => novo_id("perfil"),
        };
        let perfil = ConnectionProfile::validar(DadosPerfil { id, ..dados })
            .map_err(|e| ErroGerente::PerfilInvalido(e.to_string()))?;

        // A `secret_ref` é determinística e não contém PII: se o perfil já
        // tinha referência gravada, ela é preservada; caso contrário aponta
        // para a chave que `abrir` vai gravar.
        let secret_ref = existente
            .as_ref()
            .and_then(|reg| reg.secret_ref.clone())
            .or_else(|| Some(secret_ref(perfil.id())));
        let created_at = existente
            .as_ref()
            .map(|reg| reg.created_at.clone())
            .unwrap_or_else(|| agora.clone());

        self.repos
            .save(&registro_de_perfil(&perfil, secret_ref, created_at, agora))?;
        Ok(perfil)
    }

    /// Lista os perfis salvos, convertendo cada registro em perfil validado.
    ///
    /// Um registro inválido (dialeto desconhecido, porta fora de faixa)
    /// interrompe a lista com erro nomeando o id: esconder a linha deixaria o
    /// usuário sem como descobrir o perfil quebrado.
    pub fn listar(&self) -> Result<Vec<ConnectionProfile>, ErroGerente> {
        self.repos
            .list()?
            .iter()
            .map(|registro| {
                perfil_de_registro(registro)
                    .map_err(|e| ErroGerente::PerfilInvalido(format!("{}: {}", registro.id, e)))
            })
            .collect()
    }

    /// Obtem um perfil validado pelo id.
    pub fn obter(&self, profile_id: &str) -> Result<ConnectionProfile, ErroGerente> {
        let registro = self
            .repos
            .get(profile_id)?
            .ok_or_else(|| ErroGerente::PerfilNaoEncontrado(profile_id.to_string()))?;
        perfil_de_registro(&registro)
            .map_err(|e| ErroGerente::PerfilInvalido(format!("{profile_id}: {e}")))
    }

    /// Abre uma sessão: grava a senha no cofre e conecta.
    ///
    /// `senha` é a única entrada de segredo do app (Spec 03 §1). Quando vem
    /// preenchida, é gravada na `secret_ref` do perfil **antes** de conectar —
    /// assim o próximo `abrir` sem senha (reconexão, outro atalho) funciona;
    /// quando vem vazia, a senha é lida do cofre e a ausência é
    /// [`ErroGerente::SegredoAusente`], nunca senha vazia silenciosa.
    pub fn abrir(
        &self,
        profile_id: &str,
        senha: Option<&str>,
    ) -> Result<SessaoAberta, ErroGerente> {
        let perfil = self.obter(profile_id)?;
        let senha = match senha {
            Some(senha) if !senha.is_empty() => {
                self.secrets.set(&secret_ref(profile_id), senha)?;
                senha.to_string()
            }
            _ => self
                .secrets
                .get(&secret_ref(profile_id))?
                .ok_or_else(|| ErroGerente::SegredoAusente(profile_id.to_string()))?,
        };

        self.marcar(profile_id, |estado| {
            estado.status = ConnectionStatus::Connecting;
            estado.session_id = None;
            estado.mensagem_erro_limpa();
        });

        match self.conectar(&perfil, &senha) {
            Ok((adaptador, biblioteca, versao)) => {
                let session_id = novo_id("sessao");
                let client_library_path = biblioteca.clone();
                let versao_texto = versao.map(texto_da_versao);
                self.marcar(profile_id, |estado| {
                    estado.status = ConnectionStatus::Connected;
                    estado.session_id = Some(session_id.clone());
                    estado.client_library_path = client_library_path.clone();
                    estado.adaptador = Some(adaptador);
                    estado.server_version = versao;
                });
                Ok(SessaoAberta {
                    profile_id: profile_id.to_string(),
                    session_id,
                    status: ConnectionStatus::Connected,
                    client_library_path,
                    server_version: versao_texto,
                })
            }
            Err(erro) => {
                let erro = ErroGerente::conexao(&erro);
                let mensagem = redigir_segredo(&erro.to_string());
                self.marcar(profile_id, |estado| {
                    estado.status = ConnectionStatus::Error(mensagem.clone());
                    estado.session_id = None;
                    estado.adaptador = None;
                });
                Err(erro)
            }
        }
    }

    /// Fecha a sessão do perfil. Idempotente.
    ///
    /// Fechar um perfil **sem sessão** é sucesso e devolve
    /// [`ConnectionStatus::Disconnected`] — `connection_close` pode ser
    /// clicado duas vezes sem efeito colateral. Fechar um perfil inexistente,
    /// sim, é erro ([`ErroGerente::PerfilNaoEncontrado`]): o id nunca existiu,
    /// e isso costuma ser bug de chamador, não ação do usuário.
    pub fn fechar(&self, profile_id: &str) -> Result<ConnectionStatus, ErroGerente> {
        // Confere a existência do perfil antes: só então a idempotência vale.
        self.obter(profile_id)?;
        self.marcar(profile_id, |estado| {
            estado.adaptador = None;
            estado.session_id = None;
            estado.status = ConnectionStatus::Disconnected;
        });
        Ok(ConnectionStatus::Disconnected)
    }

    /// Situação atual do perfil (perfil salvo e sessão são coisas distintas).
    pub fn status(&self, profile_id: &str) -> Result<InformacoesStatus, ErroGerente> {
        self.obter(profile_id)?;
        Ok(self.informacoes(profile_id))
    }

    /// Executa `acao` contra o adaptador vivo do perfil conectado.
    /// Falha com erro de conexão quando o perfil não existe, não está
    /// conectado ou não tem adaptador — nunca aloca sem sessão. O lock da
    /// tabela de sessões é mantido durante a chamada (a ação não pode reentrar
    /// neste gerenciador); queries de catálogo aceitam isso.
    pub fn executar_no_adaptador_conectado<F, T>(
        &self,
        profile_id: &str,
        acao: F,
    ) -> Result<T, ErroGerente>
    where
        F: FnOnce(&mut AdaptadorAberto) -> Result<T, ErroGerente>,
    {
        // Confere que o perfil salvo existe, antes de anything else.

        self.obter(profile_id)?;
        let mut sessoes = self.lock();
        let estado = sessoes
            .get_mut(profile_id)
            .ok_or_else(|| ErroGerente::PerfilNaoEncontrado(profile_id.to_string()))?;
        if !matches!(estado.status, ConnectionStatus::Connected) {
            return Err(ErroGerente::Conexao(format!(
                "o perfil {} não está conectado",
                profile_id
            )));
        }
        let adaptador = estado.adaptador.as_mut().ok_or_else(|| {
            ErroGerente::Conexao("sem adaptador vivo para esta sessão".to_string())
        })?;
        acao(adaptador)
    }

    /// Reconecta o perfil, usando a senha já guardada no cofre.
    ///
    /// Passa por [`ConnectionStatus::Reconnecting`] antes da nova tentativa,
    /// para que a UI mostre "reconectando" em vez de parecer ociosa
    /// (Spec 03 §1).
    pub fn reconectar(&self, profile_id: &str) -> Result<InformacoesStatus, ErroGerente> {
        let perfil = self.obter(profile_id)?;
        self.marcar(profile_id, |estado| {
            estado.status = ConnectionStatus::Reconnecting;
            estado.mensagem_erro_limpa();
        });

        let senha = self
            .secrets
            .get(&secret_ref(profile_id))?
            .ok_or_else(|| ErroGerente::SegredoAusente(profile_id.to_string()))?;

        match self.conectar(&perfil, &senha) {
            Ok((adaptador, biblioteca, versao)) => {
                self.marcar(profile_id, |estado| {
                    estado.status = ConnectionStatus::Connected;
                    estado.session_id = Some(novo_id("sessao"));
                    estado.client_library_path = biblioteca.clone();
                    estado.adaptador = Some(adaptador);
                    estado.server_version = versao;
                });
            }
            Err(erro) => {
                let erro = ErroGerente::conexao(&erro);
                let mensagem = redigir_segredo(&erro.to_string());
                self.marcar(profile_id, |estado| {
                    estado.status = ConnectionStatus::Error(mensagem.clone());
                    estado.session_id = None;
                    estado.adaptador = None;
                });
                return Err(erro);
            }
        }
        Ok(self.informacoes(profile_id))
    }

    /// Testa a conectividade **sem persistir nada** (Spec 03 §1).
    ///
    /// Não grava perfil, não grava senha no cofre e não abre sessão: apenas
    /// monta os parâmetros e conecta. Falha de conexão vira
    /// [`TesteConexao { ok: false, .. }`] com mensagem acionável — a UI
    /// mostra o erro sem travar (Spec 03 §2).
    pub fn testar(
        &self,
        entrada: DadosPerfil,
        senha: Option<&str>,
    ) -> Result<TesteConexao, ErroGerente> {
        let id_existente = entrada.id.trim();
        let senha = match senha {
            Some(senha) if !senha.is_empty() => senha.to_string(),
            _ if !id_existente.is_empty() => self
                .secrets
                .get(&secret_ref(id_existente))
                .ok()
                .flatten()
                .unwrap_or_default(),
            _ => String::new(),
        };
        // O id é irrelevante para o teste: um perfil novo (sem id) é testado
        // com os dados da tela, sem tocar no armazenamento. O PLACEHOLDER só
        // satisfaz a validação do domínio — nunca é persistido.
        const PLACEHOLDER: &str = "perfil-em-teste";
        let dados = DadosPerfil {
            id: if id_existente.is_empty() {
                PLACEHOLDER.to_string()
            } else {
                id_existente.to_string()
            },
            ..entrada
        };
        let perfil = ConnectionProfile::validar(dados)
            .map_err(|e| ErroGerente::PerfilInvalido(e.to_string()))?;

        match self.conectar(&perfil, &senha) {
            Ok((adaptador, biblioteca, versao)) => {
                drop(adaptador);
                Ok(TesteConexao {
                    ok: true,
                    client_library_path: biblioteca.clone(),
                    server_version: versao.map(texto_da_versao),
                    message: None,
                })
            }
            Err(erro) => Ok(TesteConexao {
                ok: false,
                client_library_path: None,
                server_version: None,
                message: Some(redigir_segredo(&erro.to_string())),
            }),
        }
    }

    /// Snapshot do estado em memória de um perfil (sem tocar o armazenamento).
    fn informacoes(&self, profile_id: &str) -> InformacoesStatus {
        let sessoes = self.lock();
        match sessoes.get(profile_id) {
            Some(estado) => InformacoesStatus {
                profile_id: profile_id.to_string(),
                status: estado.status.clone(),
                session_id: estado.session_id.clone(),
                client_library_path: estado.client_library_path.clone(),
            },
            None => InformacoesStatus {
                profile_id: profile_id.to_string(),
                status: ConnectionStatus::Disconnected,
                session_id: None,
                client_library_path: None,
            },
        }
    }

    /// Aplica `mudanca` ao estado do perfil, criando a entrada se preciso.
    fn marcar<F>(&self, profile_id: &str, mudanca: F)
    where
        F: FnOnce(&mut EstadoSessao),
    {
        let mut sessoes = self.lock();
        let estado = sessoes
            .entry(profile_id.to_string())
            .or_insert(EstadoSessao {
                status: ConnectionStatus::Disconnected,
                session_id: None,
                client_library_path: None,
                adaptador: None,
                server_version: None,
            });
        mudanca(estado);
    }

    /// Conecta de fato: resolve a biblioteca cliente, monta os parâmetros e
    /// pede a conexão ao [`Connector`].
    fn conectar(
        &self,
        perfil: &ConnectionProfile,
        senha: &str,
    ) -> Result<(AdaptadorAberto, Option<String>, Option<ServerVersion>), DomainError> {
        let biblioteca = resolver_biblioteca(perfil)?;
        let params = ConnParams {
            host: perfil.host().to_string(),
            port: perfil.port(),
            database: perfil.database_path().to_string(),
            user: perfil.username().to_string(),
            password: senha.to_string(),
            charset: perfil.charset().to_string(),
            client_library_path: biblioteca.clone(),
        };
        let mut adaptador = self.connector.connect(&params)?;
        // A versão é informativa: uma sessão conectada que não informa
        // versão continua sendo sessão válida.
        let versao = adaptador.server_version().ok();
        Ok((adaptador, Some(biblioteca.display().to_string()), versao))
    }

    /// Trava o mapa de sessões ignorando envenenamento.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, EstadoSessao>> {
        self.sessoes.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl EstadoSessao {
    /// Limpa a mensagem do estado de erro ao iniciar uma nova tentativa.
    fn mensagem_erro_limpa(&mut self) {
        if let ConnectionStatus::Error(_) = self.status {
            self.status = ConnectionStatus::Disconnected;
        }
    }
}

/// Chave de ordenação de uma versão, para escolher a `libfbclient.so` mais
/// nova entre as descobertas (o nome sem versão perde para qualquer
/// versionada).
fn chave_de_versao(versao: Option<ServerVersion>) -> (u8, u8, u8) {
    match versao {
        Some(v) => (v.major, v.minor, v.patch.unwrap_or(0)),
        None => (0, 0, 0),
    }
}

/// Texto legível da versão do servidor (`major.minor[.patch]`).
fn texto_da_versao(versao: ServerVersion) -> String {
    match versao.patch {
        Some(patch) => format!("{}.{}.{}", versao.major, versao.minor, patch),
        None => format!("{}.{}", versao.major, versao.minor),
    }
}

/// Escolhe a biblioteca cliente do perfil ou a de maior versão descoberta.
fn resolver_biblioteca(perfil: &ConnectionProfile) -> Result<PathBuf, DomainError> {
    if let Some(caminho) = perfil.client_library_path() {
        let caminho = PathBuf::from(caminho);
        validar_arquivo(&caminho)?;
        return Ok(caminho);
    }

    let candidatas = ClientLibrary::discover();
    let escolhida = candidatas
        .iter()
        .filter(|lib| lib.validate_arch().is_ok())
        .max_by_key(|lib| chave_de_versao(lib.version))
        .ok_or_else(|| DomainError::Driver {
            message: "nenhuma libfbclient.so compatível foi encontrada; \
                          informe o caminho em clientLibraryPath"
                .to_string(),
        })?;
    Ok(escolhida.path.clone())
}

/// Falha com erro acionável quando o arquivo escolhido não é uma biblioteca
/// carregável (inexistente, sem permissão ou de outra arquitetura).
fn validar_arquivo(caminho: &Path) -> Result<(), DomainError> {
    let biblioteca = ClientLibrary {
        path: caminho.to_path_buf(),
        version: None,
    };
    biblioteca.validate_arch().map_err(|_| DomainError::Driver {
        message: format!(
            "a biblioteca cliente '{}' não pôde ser carregada: verifique se existe \
             e é compatível com esta arquitetura",
            caminho.display()
        ),
    })
}

/// Contador monotonicamente crescente, para não repetir id dentro do processo.
static CONTADOR_ID: AtomicU64 = AtomicU64::new(0);

/// Gera um identificador local sem dependência externa.
///
/// Timestamp em nanos + pid + contador: monotônico dentro do processo e único
/// o bastante para a escala de um app desktop. **Não** é usado como chave de
/// segurança em lugar nenhum.
pub fn novo_id(prefixo: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let contador = CONTADOR_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefixo}-{nanos:x}-{:x}-{contador:x}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::application::fakes::{biblioteca_cliente_falsa, ConectorFalso};
    use firebird_security::InMemorySecretStore;

    const SENHA: &str = "Abc@123-secreta";

    fn dados(nome: &str) -> DadosPerfil {
        DadosPerfil {
            id: String::new(),
            name: nome.to_string(),
            host: "servidor.local".to_string(),
            port: 3050,
            database_path: "/dados/app.fdb".to_string(),
            username: "SYSDBA".to_string(),
            role_name: None,
            charset: "UTF8".to_string(),
            sql_dialect: Some(firebird_domain::SqlDialect::V3),
            client_library_path: Some(biblioteca_cliente_falsa().display().to_string()),
            connect_timeout_ms: 5_000,
        }
    }

    /// Ambiente de teste: banco em memória, cofre em RAM e conector falso.
    fn gerente(
        conector: Arc<dyn Connector>,
    ) -> (ConnectionManager, Arc<BancoLocal>, Arc<InMemorySecretStore>) {
        let banco = Arc::new(BancoLocal::em_memoria().expect("banco em memória"));
        let cofre = Arc::new(InMemorySecretStore::new());
        (
            ConnectionManager::com_banco(banco.clone(), cofre.clone(), conector),
            banco,
            cofre,
        )
    }

    fn gerente_ok() -> (ConnectionManager, Arc<BancoLocal>, Arc<InMemorySecretStore>) {
        gerente(ConectorFalso::ok())
    }

    fn perfil_criado(gerente: &ConnectionManager, nome: &str) -> ConnectionProfile {
        gerente.salvar(dados(nome)).expect("criando perfil")
    }

    #[test]
    fn salvar_cria_perfil_com_id_e_secret_ref_deterministica() {
        let (gerente, banco, _cofre) = gerente_ok();
        let perfil = perfil_criado(&gerente, "Produção");

        assert!(!perfil.id().is_empty(), "o id deve ser gerado");
        let registro = banco.get(perfil.id()).expect("buscando").expect("perfil");
        assert_eq!(perfil.name(), "Produção");
        assert_eq!(
            registro.secret_ref.as_deref(),
            Some(secret_ref(perfil.id()).as_str()),
            "a secret_ref aponta para a chave do cofre, sem PII"
        );
        assert!(
            !registro.secret_ref.unwrap_or_default().contains("SYSDBA"),
            "a secret_ref não pode conter dado do usuário"
        );
    }

    #[test]
    fn salvar_edicao_preserva_created_at_e_muda_updated_at() {
        let (gerente, banco, _cofre) = gerente_ok();
        let perfil = perfil_criado(&gerente, "Original");
        let antes = banco.get(perfil.id()).expect("buscando").expect("perfil");

        gerente
            .salvar(DadosPerfil {
                id: perfil.id().to_string(),
                name: "Renomeado".to_string(),
                ..dados("Original")
            })
            .expect("editando perfil");

        let depois = banco.get(perfil.id()).expect("buscando").expect("perfil");
        assert_eq!(depois.name, "Renomeado");
        assert_eq!(depois.created_at, antes.created_at, "created_at é imutável");
        assert!(
            depois.updated_at >= antes.updated_at,
            "updated_at deve avançar: {} -> {}",
            antes.updated_at,
            depois.updated_at
        );
    }

    #[test]
    fn salvar_com_id_inexistente_devolve_erro_e_nao_insere() {
        let (gerente, banco, _cofre) = gerente_ok();
        let erro = gerente
            .salvar(DadosPerfil {
                id: "fantasma".to_string(),
                ..dados("Fantasma")
            })
            .expect_err("id inexistente deve falhar");
        assert!(matches!(erro, ErroGerente::PerfilNaoEncontrado(_)));
        assert!(
            banco.list().expect("listando").is_empty(),
            "nada foi inserido"
        );
    }

    #[test]
    fn salvar_perfil_invalido_devolve_erro_de_perfil() {
        let (gerente, banco, _cofre) = gerente_ok();
        let erro = gerente
            .salvar(DadosPerfil {
                name: String::new(),
                ..dados("Sem nome")
            })
            .expect_err("nome vazio deve falhar");
        assert!(matches!(erro, ErroGerente::PerfilInvalido(_)));
        assert!(banco.list().expect("listando").is_empty());
    }

    #[test]
    fn listar_devolve_perfis_salvos_e_erro_de_perfil_inexistente() {
        let (gerente, _banco, _cofre) = gerente_ok();
        perfil_criado(&gerente, "Alfa");
        perfil_criado(&gerente, "Zeta");

        let listados = gerente.listar().expect("listando");
        let nomes: Vec<&str> = listados.iter().map(ConnectionProfile::name).collect();
        assert_eq!(nomes, vec!["Alfa", "Zeta"]);

        let erro = gerente.obter("nao-existe").expect_err("id desconhecido");
        assert!(matches!(erro, ErroGerente::PerfilNaoEncontrado(_)));
    }

    #[test]
    fn abrir_grava_a_senha_no_cofre_e_conecta_com_ela() {
        let conector = ConectorFalso::ok();
        let (gerente, _banco, cofre) = gerente(conector.clone());
        let perfil = perfil_criado(&gerente, "Produção");

        let sessao = gerente.abrir(perfil.id(), Some(SENHA)).expect("abrindo");

        assert_eq!(sessao.status, ConnectionStatus::Connected);
        assert!(!sessao.session_id.is_empty());
        assert_eq!(sessao.server_version.as_deref(), Some("4.0.1"));
        assert_eq!(
            cofre.get(&secret_ref(perfil.id())).expect("lendo cofre"),
            Some(SENHA.to_string()),
            "a senha deve estar no cofre, não no perfil"
        );
        let recebidos = conector.recebidos();
        assert_eq!(recebidos.len(), 1);
        assert_eq!(recebidos[0].password, SENHA, "o driver recebe a senha");
        assert_eq!(recebidos[0].host, "servidor.local");
        assert_eq!(recebidos[0].port, 3050);
    }

    #[test]
    fn abrir_sem_senha_usa_o_segredo_ja_guardado_no_cofre() {
        let conector = ConectorFalso::ok();
        let (gerente, _banco, cofre) = gerente(conector.clone());
        let perfil = perfil_criado(&gerente, "Produção");
        cofre
            .set(&secret_ref(perfil.id()), SENHA)
            .expect("gravando");

        let sessao = gerente
            .abrir(perfil.id(), None)
            .expect("abrindo com o cofre");
        assert_eq!(sessao.status, ConnectionStatus::Connected);
        assert_eq!(conector.recebidos()[0].password, SENHA);
    }

    #[test]
    fn abrir_sem_senha_e_sem_segredo_devolve_erro_tipado() {
        let conector = ConectorFalso::ok();
        let (gerente, _banco, _cofre) = gerente(conector.clone());
        let perfil = perfil_criado(&gerente, "Produção");

        let erro = gerente.abrir(perfil.id(), None).expect_err("sem senha");

        assert!(
            matches!(erro, ErroGerente::SegredoAusente(_)),
            "esperava SegredoAusente, veio {erro:?}"
        );
        assert!(erro.to_string().contains("informe-a ao conectar"));
        assert!(
            conector.recebidos().is_empty(),
            "não deve tentar conectar sem senha no cofre"
        );
    }

    #[test]
    fn abrir_perfil_inexistente_devolve_erro_sem_tocar_o_cofre() {
        let (gerente, _banco, cofre) = gerente_ok();
        let erro = gerente
            .abrir("fantasma", Some(SENHA))
            .expect_err("perfil inexistente");
        assert!(matches!(erro, ErroGerente::PerfilNaoEncontrado(_)));
        assert!(cofre.is_empty(), "nada deve ser gravado no cofre");
    }

    #[test]
    fn falha_de_conexao_marca_status_error_com_mensagem_redigida() {
        let conector = ConectorFalso::falhando("login failed: password=S3nh4! para SYSDBA");
        let (gerente, _banco, cofre) = gerente(conector);
        let perfil = perfil_criado(&gerente, "Produção");

        let erro = gerente
            .abrir(perfil.id(), Some(SENHA))
            .expect_err("conexão falha");

        assert!(matches!(erro, ErroGerente::Conexao(_)));
        let status = gerente.status(perfil.id()).expect("status");
        assert!(matches!(status.status, ConnectionStatus::Error(_)));
        let mensagem = status.mensagem().expect("mensagem de erro");
        assert!(!mensagem.contains("S3nh4!"), "a senha vazou: {mensagem}");
        assert!(mensagem.contains("password=***"), "{mensagem}");

        // A senha continua no cofre: o próximo `abrir` sem senha funciona.
        assert_eq!(
            cofre.get(&secret_ref(perfil.id())).expect("cofre"),
            Some(SENHA.to_string())
        );
    }

    #[test]
    fn status_de_perfil_salvo_sem_sessao_e_desconectado() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let perfil = perfil_criado(&gerente, "Produção");

        let status = gerente.status(perfil.id()).expect("status");
        assert_eq!(status.status, ConnectionStatus::Disconnected);
        assert_eq!(status.session_id, None);
        assert_eq!(status.client_library_path, None);
        assert_eq!(status.mensagem(), None);
    }

    #[test]
    fn status_de_perfil_desconhecido_devolve_erro() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let erro = gerente.status("fantasma").expect_err("perfil inexistente");
        assert!(matches!(erro, ErroGerente::PerfilNaoEncontrado(_)));
    }

    #[test]
    fn status_mostra_session_id_e_biblioteca_enquanto_conectado() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let perfil = perfil_criado(&gerente, "Produção");
        let sessao = gerente.abrir(perfil.id(), Some(SENHA)).expect("abrindo");

        let status = gerente.status(perfil.id()).expect("status");
        assert_eq!(status.status, ConnectionStatus::Connected);
        assert_eq!(
            status.session_id.as_deref(),
            Some(sessao.session_id.as_str())
        );
        assert_eq!(
            status.client_library_path.as_deref(),
            Some(biblioteca_cliente_falsa().to_str().expect("caminho utf-8"))
        );
    }

    #[test]
    fn close_e_idempotente_e_deixa_o_perfil_desconectado() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let perfil = perfil_criado(&gerente, "Produção");
        gerente.abrir(perfil.id(), Some(SENHA)).expect("abrindo");

        assert_eq!(
            gerente.fechar(perfil.id()).expect("fechando"),
            ConnectionStatus::Disconnected
        );
        assert_eq!(
            gerente.fechar(perfil.id()).expect("fechar de novo"),
            ConnectionStatus::Disconnected,
            "fechar duas vezes não é erro"
        );
        let status = gerente.status(perfil.id()).expect("status");
        assert_eq!(status.status, ConnectionStatus::Disconnected);
        assert_eq!(status.session_id, None);
    }

    #[test]
    fn close_de_perfil_sem_sessao_e_idempotente() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let perfil = perfil_criado(&gerente, "Produção");
        assert_eq!(
            gerente.fechar(perfil.id()).expect("fechar sem sessão"),
            ConnectionStatus::Disconnected
        );
        assert!(gerente.fechar(perfil.id()).is_ok());
    }

    #[test]
    fn close_de_perfil_inexistente_devolve_erro() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let erro = gerente.fechar("fantasma").expect_err("perfil inexistente");
        assert!(matches!(erro, ErroGerente::PerfilNaoEncontrado(_)));
    }

    #[test]
    fn reconectar_usa_o_cofre_e_passa_por_reconectando() {
        let conector = ConectorFalso::ok();
        let (gerente, _banco, cofre) = gerente(conector.clone());
        let perfil = perfil_criado(&gerente, "Produção");
        cofre
            .set(&secret_ref(perfil.id()), SENHA)
            .expect("gravando");

        let status = gerente.reconectar(perfil.id()).expect("reconectando");
        assert_eq!(status.status, ConnectionStatus::Connected);
        assert!(status.session_id.is_some());
        assert_eq!(conector.recebidos()[0].password, SENHA);
    }

    #[test]
    fn reconectar_sem_senha_no_cofre_devolve_segredo_ausente() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let perfil = perfil_criado(&gerente, "Produção");
        let erro = gerente.reconectar(perfil.id()).expect_err("sem senha");
        assert!(matches!(erro, ErroGerente::SegredoAusente(_)));
    }

    #[test]
    fn testar_nao_persiste_perfil_nem_senha() {
        let conector = ConectorFalso::ok();
        let (gerente, banco, cofre) = gerente(conector.clone());

        let resultado = gerente
            .testar(dados("Perfil Novo"), Some(SENHA))
            .expect("testando");

        assert!(resultado.ok);
        assert!(
            banco.list().expect("listando").is_empty(),
            "não persiste perfil"
        );
        assert!(cofre.is_empty(), "não grava senha no cofre");
        assert_eq!(conector.recebidos().len(), 1);
        assert_eq!(
            conector.recebidos()[0].password,
            SENHA,
            "a senha informada é usada só nesta tentativa"
        );
    }

    #[test]
    fn testar_relata_falha_sem_erro_de_comando_e_com_mensagem_redigida() {
        let conector = ConectorFalso::falhando("connection refused (password=Abc@123)");
        let (gerente, banco, cofre) = gerente(conector);

        let resultado = gerente
            .testar(dados("Produção"), Some(SENHA))
            .expect("teste com falha não é erro de comando");

        assert!(!resultado.ok);
        let mensagem = resultado.message.expect("mensagem");
        assert!(!mensagem.contains("Abc@123"), "vazou: {mensagem}");
        assert!(mensagem.contains("password=***"), "{mensagem}");
        assert!(banco.list().expect("listando").is_empty());
        assert!(cofre.is_empty());
    }

    #[test]
    fn testar_usa_a_senha_do_cofre_quando_o_perfil_ja_existe() {
        let conector = ConectorFalso::ok();
        let (gerente, _banco, cofre) = gerente(conector.clone());
        let perfil = perfil_criado(&gerente, "Produção");
        cofre
            .set(&secret_ref(perfil.id()), SENHA)
            .expect("gravando");

        let mut entrada = dados("Produção");
        entrada.id = perfil.id().to_string();
        let resultado = gerente.testar(entrada, None).expect("testando");

        assert!(resultado.ok);
        assert_eq!(conector.recebidos()[0].password, SENHA);
    }

    #[test]
    fn testar_rejeita_perfil_invalido_com_erro_de_perfil() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let erro = gerente
            .testar(
                DadosPerfil {
                    port: 0,
                    ..dados("Ruim")
                },
                Some(SENHA),
            )
            .expect_err("porta zero");
        assert!(matches!(erro, ErroGerente::PerfilInvalido(_)));
    }

    #[test]
    fn testar_com_biblioteca_cliente_inexistente_devolve_ok_falso_com_orientacao() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let resultado = gerente
            .testar(
                DadosPerfil {
                    client_library_path: Some("/nao/existe/libfbclient.so.9".to_string()),
                    ..dados("Sem lib")
                },
                Some(SENHA),
            )
            .expect("falha de biblioteca não é erro de comando");

        assert!(!resultado.ok);
        let mensagem = resultado.message.expect("mensagem");
        assert!(
            mensagem.contains("/nao/existe/libfbclient.so.9"),
            "{mensagem}"
        );
    }

    #[test]
    fn abrir_com_biblioteca_inexistente_devolve_erro_de_conexao() {
        let (gerente, _banco, _cofre) = gerente_ok();
        let erro = gerente
            .salvar(DadosPerfil {
                client_library_path: Some("/nao/existe/libfbclient.so.9".to_string()),
                ..dados("Sem lib")
            })
            .expect("perfil salvo");
        let erro = gerente
            .abrir(erro.id(), Some(SENHA))
            .expect_err("sem biblioteca não há conexão");
        assert!(matches!(erro, ErroGerente::Conexao(_)), "{erro:?}");
        assert!(!erro.to_string().contains(SENHA));
    }

    #[test]
    fn debug_de_erro_do_gerente_nao_expoe_senha() {
        let erro = ErroGerente::Conexao("falhou com password=S3nh4! ao conectar".to_string());
        let texto = format!("{erro:?}");
        assert!(!texto.contains("S3nh4!"), "vazou: {texto}");
        assert!(
            texto.contains("Conexao"),
            "a variante deve aparecer: {texto}"
        );
    }

    #[test]
    fn cofre_indisponivel_vira_erro_tipado_na_abertura() {
        let banco = Arc::new(BancoLocal::em_memoria().expect("banco em memória"));
        let cofre = Arc::new(crate::security::CofreIndisponivel::novo(
            "nenhum serviço de segredo disponível nesta sessão",
        ));
        let gerente = ConnectionManager::com_banco(
            banco.clone(),
            cofre,
            ConectorFalso::ok() as Arc<dyn Connector>,
        );
        let perfil = gerente.salvar(dados("Produção")).expect("perfil");

        let erro = gerente
            .abrir(perfil.id(), Some(SENHA))
            .expect_err("cofre indisponível");
        assert!(
            matches!(erro, ErroGerente::CofreIndisponivel(_)),
            "esperava CofreIndisponivel, veio {erro:?}"
        );
        assert!(erro.to_string().contains("nenhum serviço de segredo"));
    }

    #[test]
    fn novo_id_e_unico_e_prefixado() {
        let a = novo_id("perfil");
        let b = novo_id("perfil");
        assert_ne!(a, b);
        assert!(a.starts_with("perfil-"));
    }

    #[test]
    fn ids_gerados_sao_seguros_para_o_id_do_sql() {
        for _ in 0..32 {
            let id = novo_id("perfil");
            // O id vai para coluna TEXT via parâmetro $1; o teste garante que
            // não carrega aspas nem separadores que quebrem aprepared statement.
            for proibido in ['\'', '"', ';', ' ', '\n', '\0'] {
                assert!(
                    !id.contains(proibido),
                    "id com caractere perigoso {proibido:?}: {id}"
                );
            }
            assert!(id.is_ascii(), "id deve ser ASCII: {id}");
        }
    }
}
