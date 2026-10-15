//! Domínio de conexão: perfil, dialeto SQL e situação da sessão.
//!
//! Este módulo é **puro**: não conhece Tauri, driver, SQLite nem cofre de
//! segredos. Um [`ConnectionProfile`] descreve *como* conectar e nunca carrega
//! senha — a senha vive no cofre do sistema operacional e o perfil persistido
//! guarda apenas a referência dela ([`PersistedProfile`] não tem campo de
//! segredo; a `secret_ref` é montada pela camada de aplicação).
//!
//! A conversão de/para o registro do SQLite é feita por [`PersistedProfile`],
//! uma projeção com os mesmos campos do registro persistido: assim o domínio
//! não depende do crate de storage (regra de dependência da Spec 02) e a
//! camada de aplicação só precisa copiar os campos.

/// Timeout padrão de conexão quando o perfil não informa um valor.
pub const TIMEOUT_PADRAO_MS: u64 = 15_000;

/// Teto do timeout de conexão: acima disso o erro deixa de ser acionável
/// (o usuário não distingue "servidor lento" de "app travado").
pub const TIMEOUT_MAXIMO_MS: u64 = 600_000;

/// Erros de validação do perfil, sem conteúdo de conexão em mensagem.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    /// Campo obrigatório ausente, em branco ou fora do limite aceito.
    #[error("campo inválido: {0}")]
    CampoInvalido(String),

    /// Dialeto SQL fora de `{1, 3}` vindo do registro persistido.
    #[error("dialeto SQL desconhecido: {0}")]
    DialetoDesconhecido(i64),
}

/// Dialeto SQL suportado pelo Firebird (1 = legado, 3 = Firebird 3+).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    /// Firebird 1.5/2.0 — sem `RDB$RELATION_TYPE` e com dicionário legado.
    V1,
    /// Firebird 3.0 ou superior.
    V3,
}

impl SqlDialect {
    /// Valor persistido na coluna `sql_dialect`.
    pub fn as_i64(self) -> i64 {
        match self {
            SqlDialect::V1 => 1,
            SqlDialect::V3 => 3,
        }
    }

    /// Rótulo estável (minúsculo) para UI e logs.
    pub fn as_str(self) -> &'static str {
        match self {
            SqlDialect::V1 => "v1",
            SqlDialect::V3 => "v3",
        }
    }

    /// Interpreta a coluna `sql_dialect`: `None` é "não informado" e segue
    /// como [`None`] (o driver decide); qualquer valor fora de `{1, 3}` é erro
    /// explícito — nunca um `default` silencioso.
    pub fn from_i64(bruto: Option<i64>) -> Result<Option<SqlDialect>, ProfileError> {
        match bruto {
            None => Ok(None),
            Some(1) => Ok(Some(SqlDialect::V1)),
            Some(3) => Ok(Some(SqlDialect::V3)),
            Some(outro) => Err(ProfileError::DialetoDesconhecido(outro)),
        }
    }
}

impl std::fmt::Display for SqlDialect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Situação de uma sessão (Spec 03 §1), separada do perfil salvo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Sem sessão ativa.
    Disconnected,
    /// Abertura em curso.
    Connecting,
    /// Sessão ativa.
    Connected,
    /// Reconexão automática em curso após perda de conexão.
    Reconnecting,
    /// Falha de conexão ou reconexão, com detalhe já redigido.
    Error(String),
}

impl ConnectionStatus {
    /// Nome estável em **minúsculas**, usado no DTO `status` da UI.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectionStatus::Disconnected => "disconnected",
            ConnectionStatus::Connecting => "connecting",
            ConnectionStatus::Connected => "connected",
            ConnectionStatus::Reconnecting => "reconnecting",
            ConnectionStatus::Error(_) => "error",
        }
    }

    /// Detalhe legível do erro, quando `status` é [`ConnectionStatus::Error`].
    ///
    /// A mensagem chega aqui já redigida: a camada de aplicação só grava em
    /// `Error` mensagens que passaram por redação de segredo (FR-016).
    pub fn mensagem(&self) -> Option<&str> {
        match self {
            ConnectionStatus::Error(msg) => Some(msg.as_str()),
            _ => None,
        }
    }

    /// `true` quando há sessão utilizável.
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionStatus::Connected)
    }
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Dados de entrada de um perfil, ainda sem validação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DadosPerfil {
    /// Identificador do perfil.
    pub id: String,
    /// Nome amigável exibido na UI.
    pub name: String,
    /// Host do servidor Firebird.
    pub host: String,
    /// Porta TCP (padrão do Firebird: 3050).
    pub port: u16,
    /// Caminho ou alias do banco no servidor.
    pub database_path: String,
    /// Usuário de autenticação (não é segredo).
    pub username: String,
    /// Papel (role) usado na conexão, quando houver.
    pub role_name: Option<String>,
    /// Charset da conexão (ex.: `UTF8`).
    pub charset: String,
    /// Dialeto SQL, quando informado.
    pub sql_dialect: Option<SqlDialect>,
    /// Caminho da `libfbclient.so` escolhida pelo usuário.
    pub client_library_path: Option<String>,
    /// Tempo limite de conexão em milissegundos.
    pub connect_timeout_ms: u64,
}

/// Perfil de conexão validado.
///
/// Não existe — e jamais deve existir — campo de senha: o segredo fica no
/// cofre do sistema e o perfil aponta para ele pela `secret_ref` da
/// persistência (Spec 03 §1, ADR-003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionProfile {
    id: String,
    name: String,
    host: String,
    port: u16,
    database_path: String,
    username: String,
    role_name: Option<String>,
    charset: String,
    sql_dialect: Option<SqlDialect>,
    client_library_path: Option<String>,
    connect_timeout_ms: u64,
}

impl ConnectionProfile {
    /// Valida [`DadosPerfil`] e devolve o perfil.
    ///
    /// Rejeita: identificador, nome, host, banco, usuário ou charset vazios;
    /// host com espaço em branco; porta zero; e timeout fora de
    /// `1..=[`[`TIMEOUT_MAXIMO_MS`]`]`.
    pub fn validar(dados: DadosPerfil) -> Result<Self, ProfileError> {
        let DadosPerfil {
            id,
            name,
            host,
            port,
            database_path,
            username,
            role_name,
            charset,
            sql_dialect,
            client_library_path,
            connect_timeout_ms,
        } = dados;

        obrigatorio("id", &id)?;
        obrigatorio("name", &name)?;
        obrigatorio("host", &host)?;
        obrigatorio("database_path", &database_path)?;
        obrigatorio("username", &username)?;
        obrigatorio("charset", &charset)?;

        if port == 0 {
            return Err(ProfileError::CampoInvalido("port".to_string()));
        }
        if host.chars().any(char::is_whitespace) {
            return Err(ProfileError::CampoInvalido("host".to_string()));
        }
        if connect_timeout_ms == 0 || connect_timeout_ms > TIMEOUT_MAXIMO_MS {
            return Err(ProfileError::CampoInvalido(
                "connect_timeout_ms".to_string(),
            ));
        }
        if let Some(role) = role_name.as_deref() {
            if role.trim().is_empty() {
                return Err(ProfileError::CampoInvalido("role_name".to_string()));
            }
        }
        if let Some(lib) = client_library_path.as_deref() {
            if lib.trim().is_empty() {
                return Err(ProfileError::CampoInvalido(
                    "client_library_path".to_string(),
                ));
            }
        }

        Ok(Self {
            id,
            name,
            host,
            port,
            database_path,
            username,
            role_name,
            charset,
            sql_dialect,
            client_library_path,
            connect_timeout_ms,
        })
    }

    /// Identificador do perfil.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Nome amigável do perfil.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Host do servidor.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Porta TCP do servidor.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Caminho ou alias do banco no servidor.
    pub fn database_path(&self) -> &str {
        &self.database_path
    }

    /// Usuário de autenticação.
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Papel usado na conexão, quando houver.
    pub fn role_name(&self) -> Option<&str> {
        self.role_name.as_deref()
    }

    /// Charset da conexão.
    pub fn charset(&self) -> &str {
        &self.charset
    }

    /// Dialeto SQL, quando informado.
    pub fn sql_dialect(&self) -> Option<SqlDialect> {
        self.sql_dialect
    }

    /// Caminho da biblioteca cliente escolhida, quando houver.
    pub fn client_library_path(&self) -> Option<&str> {
        self.client_library_path.as_deref()
    }

    /// Tempo limite de conexão em milissegundos.
    pub fn connect_timeout_ms(&self) -> u64 {
        self.connect_timeout_ms
    }
}

/// Garante que `campo` tem conteúdo além de espaços.
fn obrigatorio(campo: &str, valor: &str) -> Result<(), ProfileError> {
    if valor.trim().is_empty() {
        return Err(ProfileError::CampoInvalido(campo.to_string()));
    }
    Ok(())
}

/// Projeção dos campos persistidos de um perfil (`connection_profiles`).
///
/// Espelha o registro do crate de storage **sem** `secret_ref`,
/// `created_at` e `updated_at`: a referência do segredo e os timestamps são
/// responsabilidade da camada de aplicação, que é quem decide quando
/// gravar; o domínio converte apenas o que descreve a conexão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedProfile {
    /// Identificador do perfil.
    pub id: String,
    /// Nome amigável.
    pub name: String,
    /// Host do servidor.
    pub host: String,
    /// Porta persistida como `i64` (SQLite `INTEGER`).
    pub port: i64,
    /// Caminho do banco.
    pub database_path: String,
    /// Usuário de autenticação.
    pub username: String,
    /// Papel, quando houver.
    pub role_name: Option<String>,
    /// Charset da conexão.
    pub charset: String,
    /// Dialeto persistido como `i64`.
    pub sql_dialect: Option<i64>,
    /// Caminho da biblioteca cliente, quando houver.
    pub client_library_path: Option<String>,
    /// Timeout persistido como `i64`.
    pub connect_timeout_ms: i64,
}

impl TryFrom<PersistedProfile> for ConnectionProfile {
    type Error = ProfileError;

    fn try_from(p: PersistedProfile) -> Result<Self, Self::Error> {
        // O SQLite devolve `INTEGER` (i64): um registro gravado por outra
        // versão do app pode trazer valor fora da faixa — recusa é melhor que
        // truncamento silencioso para uma porta ou um timeout.
        let port = u16::try_from(p.port)
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| ProfileError::CampoInvalido("port".to_string()))?;
        let timeout = u64::try_from(p.connect_timeout_ms)
            .map_err(|_| ProfileError::CampoInvalido("connect_timeout_ms".to_string()))?;

        ConnectionProfile::validar(DadosPerfil {
            sql_dialect: SqlDialect::from_i64(p.sql_dialect)?,
            id: p.id,
            name: p.name,
            host: p.host,
            port,
            database_path: p.database_path,
            username: p.username,
            role_name: p.role_name,
            charset: p.charset,
            client_library_path: p.client_library_path,
            connect_timeout_ms: timeout,
        })
    }
}

impl From<&ConnectionProfile> for PersistedProfile {
    fn from(p: &ConnectionProfile) -> Self {
        PersistedProfile {
            id: p.id().to_string(),
            name: p.name().to_string(),
            host: p.host().to_string(),
            port: i64::from(p.port()),
            database_path: p.database_path().to_string(),
            username: p.username().to_string(),
            role_name: p.role_name().map(str::to_string),
            charset: p.charset().to_string(),
            sql_dialect: p.sql_dialect().map(SqlDialect::as_i64),
            client_library_path: p.client_library_path().map(str::to_string),
            connect_timeout_ms: p.connect_timeout_ms() as i64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dados_validos() -> DadosPerfil {
        DadosPerfil {
            id: "p1".to_string(),
            name: "Produção".to_string(),
            host: "servidor.local".to_string(),
            port: 3050,
            database_path: "/var/lib/firebird/3.0/data/app.fdb".to_string(),
            username: "SYSDBA".to_string(),
            role_name: None,
            charset: "UTF8".to_string(),
            sql_dialect: Some(SqlDialect::V3),
            client_library_path: None,
            connect_timeout_ms: 5_000,
        }
    }

    #[test]
    fn perfil_valido_preserva_todos_os_campos() {
        let p = ConnectionProfile::validar(dados_validos()).expect("perfil válido");
        assert_eq!(p.id(), "p1");
        assert_eq!(p.name(), "Produção");
        assert_eq!(p.host(), "servidor.local");
        assert_eq!(p.port(), 3050);
        assert_eq!(p.database_path(), "/var/lib/firebird/3.0/data/app.fdb");
        assert_eq!(p.username(), "SYSDBA");
        assert_eq!(p.role_name(), None);
        assert_eq!(p.charset(), "UTF8");
        assert_eq!(p.sql_dialect(), Some(SqlDialect::V3));
        assert_eq!(p.client_library_path(), None);
        assert_eq!(p.connect_timeout_ms(), 5_000);
    }

    #[test]
    fn perfil_aceita_campos_opcionais_preenchidos() {
        let dados = DadosPerfil {
            role_name: Some("RDB$ADMIN".to_string()),
            client_library_path: Some("/usr/lib/libfbclient.so.3.0.11".to_string()),
            sql_dialect: Some(SqlDialect::V1),
            ..dados_validos()
        };
        let p = ConnectionProfile::validar(dados).expect("perfil válido");
        assert_eq!(p.role_name(), Some("RDB$ADMIN"));
        assert_eq!(
            p.client_library_path(),
            Some("/usr/lib/libfbclient.so.3.0.11")
        );
        assert_eq!(p.sql_dialect(), Some(SqlDialect::V1));
    }

    #[test]
    fn perfil_rejeita_campo_obrigatorio_em_branco() {
        for campo in ["id", "name", "host", "database_path", "username", "charset"] {
            let base = dados_validos();
            let dados = DadosPerfil {
                id: if campo == "id" { "  ".into() } else { base.id },
                name: if campo == "name" {
                    String::new()
                } else {
                    base.name
                },
                host: if campo == "host" {
                    String::new()
                } else {
                    base.host
                },
                database_path: if campo == "database_path" {
                    String::new()
                } else {
                    base.database_path
                },
                username: if campo == "username" {
                    String::new()
                } else {
                    base.username
                },
                charset: if campo == "charset" {
                    String::new()
                } else {
                    base.charset
                },
                ..dados_validos()
            };
            let erro = ConnectionProfile::validar(dados)
                .expect_err("campo obrigatório em branco deve ser rejeitado");
            assert_eq!(
                erro,
                ProfileError::CampoInvalido(campo.to_string()),
                "campo {campo} deveria ser recusado"
            );
        }
    }

    #[test]
    fn perfil_rejeita_host_com_espaco_em_branco() {
        let dados = DadosPerfil {
            host: "servidor local".to_string(),
            ..dados_validos()
        };
        assert!(matches!(
            ConnectionProfile::validar(dados),
            Err(ProfileError::CampoInvalido(_))
        ));
    }

    #[test]
    fn perfil_rejeita_porta_zero() {
        let dados = DadosPerfil {
            port: 0,
            ..dados_validos()
        };
        assert_eq!(
            ConnectionProfile::validar(dados),
            Err(ProfileError::CampoInvalido("port".to_string()))
        );
    }

    #[test]
    fn perfil_rejeita_timeout_zero_e_timeout_absurdo() {
        for timeout in [0, TIMEOUT_MAXIMO_MS + 1] {
            let dados = DadosPerfil {
                connect_timeout_ms: timeout,
                ..dados_validos()
            };
            assert_eq!(
                ConnectionProfile::validar(dados),
                Err(ProfileError::CampoInvalido(
                    "connect_timeout_ms".to_string()
                )),
                "timeout {timeout} deveria ser recusado"
            );
        }
    }

    #[test]
    fn perfil_aceita_timeout_no_limite() {
        let dados = DadosPerfil {
            connect_timeout_ms: TIMEOUT_MAXIMO_MS,
            ..dados_validos()
        };
        assert!(ConnectionProfile::validar(dados).is_ok());
    }

    #[test]
    fn perfil_rejeita_opcionais_vazios() {
        for (role, lib) in [(Some("   ".to_string()), None), (None, Some(String::new()))] {
            let dados = DadosPerfil {
                role_name: role,
                client_library_path: lib,
                ..dados_validos()
            };
            assert!(
                matches!(
                    ConnectionProfile::validar(dados),
                    Err(ProfileError::CampoInvalido(_))
                ),
                "opcional vazio deveria ser recusado"
            );
        }
    }

    #[test]
    fn perfil_preserva_unicode_e_identificadores_com_aspas() {
        let dados = DadosPerfil {
            id: "perfil-ç-1".to_string(),
            name: "Čatã do \"Norte\" — O'Brien".to_string(),
            database_path: "/bases/ação.fdb".to_string(),
            username: "USUÁRIO".to_string(),
            role_name: Some("Papel \"R1\"".to_string()),
            charset: "WIN1252".to_string(),
            ..dados_validos()
        };
        let p = ConnectionProfile::validar(dados).expect("perfil unicode válido");
        assert_eq!(p.id(), "perfil-ç-1");
        assert_eq!(p.name(), "Čatã do \"Norte\" — O'Brien");
        assert_eq!(p.role_name(), Some("Papel \"R1\""));
    }

    #[test]
    fn dialeto_interpreta_1_e_3_e_rejeita_outros() {
        assert_eq!(SqlDialect::from_i64(None), Ok(None));
        assert_eq!(SqlDialect::from_i64(Some(1)), Ok(Some(SqlDialect::V1)));
        assert_eq!(SqlDialect::from_i64(Some(3)), Ok(Some(SqlDialect::V3)));
        assert_eq!(
            SqlDialect::from_i64(Some(2)),
            Err(ProfileError::DialetoDesconhecido(2))
        );
        assert_eq!(
            SqlDialect::from_i64(Some(-1)),
            Err(ProfileError::DialetoDesconhecido(-1))
        );
    }

    #[test]
    fn dialeto_serializa_para_o_registro() {
        assert_eq!(SqlDialect::V1.as_i64(), 1);
        assert_eq!(SqlDialect::V3.as_i64(), 3);
        assert_eq!(SqlDialect::V1.to_string(), "v1");
        assert_eq!(SqlDialect::V3.to_string(), "v3");
    }

    #[test]
    fn status_serializa_em_minusculas() {
        assert_eq!(ConnectionStatus::Disconnected.to_string(), "disconnected");
        assert_eq!(ConnectionStatus::Connecting.to_string(), "connecting");
        assert_eq!(ConnectionStatus::Connected.to_string(), "connected");
        assert_eq!(ConnectionStatus::Reconnecting.to_string(), "reconnecting");
        assert_eq!(
            ConnectionStatus::Error("falhou".to_string()).to_string(),
            "error"
        );
    }

    #[test]
    fn status_expoe_mensagem_somente_no_estado_de_erro() {
        assert_eq!(
            ConnectionStatus::Error("servidor fora do ar".to_string()).mensagem(),
            Some("servidor fora do ar")
        );
        assert_eq!(ConnectionStatus::Connected.mensagem(), None);
        assert_eq!(ConnectionStatus::Disconnected.mensagem(), None);
    }

    #[test]
    fn apenas_connected_esta_conectado() {
        assert!(ConnectionStatus::Connected.is_connected());
        assert!(!ConnectionStatus::Reconnecting.is_connected());
        assert!(!ConnectionStatus::Connecting.is_connected());
        assert!(!ConnectionStatus::Disconnected.is_connected());
        assert!(!ConnectionStatus::Error("x".into()).is_connected());
    }

    #[test]
    fn conversao_para_persistido_preserva_campos_e_tipos_do_sqlite() {
        let p = ConnectionProfile::validar(dados_validos()).expect("perfil válido");
        let persistido = PersistedProfile::from(&p);
        assert_eq!(persistido.port, 3050);
        assert_eq!(persistido.sql_dialect, Some(3));
        assert_eq!(persistido.connect_timeout_ms, 5_000);
        assert_eq!(persistido.name, "Produção");
    }

    #[test]
    fn conversao_de_persistido_nao_exige_dialeto() {
        let persistido = PersistedProfile {
            id: "p1".to_string(),
            name: "Alfa".to_string(),
            host: "localhost".to_string(),
            port: 3050,
            database_path: "/dados/app.fdb".to_string(),
            username: "SYSDBA".to_string(),
            role_name: None,
            charset: "UTF8".to_string(),
            sql_dialect: None,
            client_library_path: None,
            connect_timeout_ms: TIMEOUT_PADRAO_MS as i64,
        };
        let p = ConnectionProfile::try_from(persistido).expect("perfil do registro");
        assert_eq!(p.sql_dialect(), None);
        assert_eq!(p.connect_timeout_ms(), TIMEOUT_PADRAO_MS);
    }

    #[test]
    fn conversao_de_persistido_rejeita_dialeto_desconhecido() {
        let persistido = PersistedProfile {
            id: "p1".to_string(),
            name: "Alfa".to_string(),
            host: "localhost".to_string(),
            port: 3050,
            database_path: "/dados/app.fdb".to_string(),
            username: "SYSDBA".to_string(),
            role_name: None,
            charset: "UTF8".to_string(),
            sql_dialect: Some(2),
            client_library_path: None,
            connect_timeout_ms: 5_000,
        };
        assert_eq!(
            ConnectionProfile::try_from(persistido),
            Err(ProfileError::DialetoDesconhecido(2))
        );
    }

    #[test]
    fn round_trip_entre_perfil_e_persistido_e_estavel() {
        let p = ConnectionProfile::validar(DadosPerfil {
            role_name: Some("RDB$ADMIN".to_string()),
            client_library_path: Some("/usr/lib/libfbclient.so.3".to_string()),
            ..dados_validos()
        })
        .expect("perfil válido");
        let persistido = PersistedProfile::from(&p);
        let volta = ConnectionProfile::try_from(persistido).expect("round trip");
        assert_eq!(p, volta);
    }

    #[test]
    fn perfil_nao_carrega_segredo_em_nenhum_campo() {
        let p = ConnectionProfile::validar(dados_validos()).expect("perfil válido");
        let texto = format!("{p:?}");
        for proibido in ["password", "senha", "secret", "passwd"] {
            assert!(
                !texto.to_lowercase().contains(proibido),
                "perfil não pode carregar {proibido}: {texto}"
            );
        }
    }
}
