//! DTOs da fronteira de conexões (Spec 10 §1).
//!
//! O contrato é **consumido pela UI** (`apps/desktop/src/features/connections/types.ts`):
//! nomes em `camelCase`, `status` em **minúsculas** e **nenhum campo de
//! senha** em nenhum DTO — nem de entrada. A senha chega pelo argumento
//! `password` de `connection_open`, fora de qualquer DTO, embrulhada em
//! [`crate::commands::connections::SegredoOpcional`] (que redige o `Debug`).

use firebird_domain::{ConnectionProfile, ConnectionStatus, DadosPerfil, ProfileError, SqlDialect};

use crate::application::connection_manager::{InformacoesStatus, SessaoAberta, TesteConexao};

/// Situação da sessão no formato da UI: minúsculas, sem payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusDto {
    /// Sem sessão ativa.
    Disconnected,
    /// Abertura em curso.
    Connecting,
    /// Sessão ativa.
    Connected,
    /// Reconexão em curso.
    Reconnecting,
    /// Falha de conexão ou reconexão.
    Error,
}

impl From<&ConnectionStatus> for StatusDto {
    fn from(status: &ConnectionStatus) -> Self {
        match status {
            ConnectionStatus::Disconnected => StatusDto::Disconnected,
            ConnectionStatus::Connecting => StatusDto::Connecting,
            ConnectionStatus::Connected => StatusDto::Connected,
            ConnectionStatus::Reconnecting => StatusDto::Reconnecting,
            ConnectionStatus::Error(_) => StatusDto::Error,
        }
    }
}

/// Perfil de conexão salvo, como a UI o consome.
///
/// Não tem — e não pode ter — campo de senha: o segredo vive no cofre do
/// sistema e é referenciado pela `secret_ref` do registro (Spec 03 §1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfileDto {
    /// Identificador do perfil.
    pub id: String,
    /// Nome amigável.
    pub name: String,
    /// Host do servidor.
    pub host: String,
    /// Porta TCP.
    pub port: u16,
    /// Caminho ou alias do banco.
    pub database_path: String,
    /// Usuário de autenticação.
    pub username: String,
    /// Papel usado na conexão.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    /// Charset da conexão.
    pub charset: String,
    /// Dialeto SQL (`1` ou `3`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql_dialect: Option<u16>,
    /// Caminho da biblioteca cliente escolhida.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_library_path: Option<String>,
    /// Tempo limite de conexão em milissegundos.
    pub connect_timeout_ms: u64,
}

impl From<&ConnectionProfile> for ConnectionProfileDto {
    fn from(perfil: &ConnectionProfile) -> Self {
        ConnectionProfileDto {
            id: perfil.id().to_string(),
            name: perfil.name().to_string(),
            host: perfil.host().to_string(),
            port: perfil.port(),
            database_path: perfil.database_path().to_string(),
            username: perfil.username().to_string(),
            role_name: perfil.role_name().map(str::to_string),
            charset: perfil.charset().to_string(),
            sql_dialect: perfil.sql_dialect().map(|d| d.as_i64() as u16),
            client_library_path: perfil.client_library_path().map(str::to_string),
            connect_timeout_ms: perfil.connect_timeout_ms(),
        }
    }
}

/// Dados de criação/edição vindos da UI.
///
/// `id` ausente = criação; presente = edição. Campos opcionais usam `default`
/// para que um payload mínimo da UI não falhe na desserialização.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfileInputDto {
    /// Perfil existente, quando for edição.
    #[serde(default)]
    pub id: Option<String>,
    /// Nome amigável.
    pub name: String,
    /// Host do servidor.
    pub host: String,
    /// Porta TCP; validada como `1..=65535` (erro tipado, não erro de serde).
    pub port: i64,
    /// Caminho ou alias do banco.
    pub database_path: String,
    /// Usuário de autenticação.
    pub username: String,
    /// Papel usado na conexão.
    #[serde(default)]
    pub role_name: Option<String>,
    /// Charset da conexão.
    pub charset: String,
    /// Dialeto SQL (`1` ou `3`).
    #[serde(default)]
    pub sql_dialect: Option<i64>,
    /// Caminho da biblioteca cliente escolhida.
    #[serde(default)]
    pub client_library_path: Option<String>,
    /// Tempo limite; ausente usa [`firebird_domain::TIMEOUT_PADRAO_MS`].
    #[serde(default)]
    pub connect_timeout_ms: Option<u64>,
}

impl ConnectionProfileInputDto {
    /// Converte a entrada da UI em dados de domínio (ainda sem validar).
    ///
    /// Erros de dialeto e de faixa de porta/timeout são
    /// [`ProfileError`], traduzidos depois em erro de comando tipado.
    pub fn para_dados(&self) -> Result<DadosPerfil, ProfileError> {
        let port = u16::try_from(self.port)
            .ok()
            .filter(|p| *p != 0)
            .ok_or_else(|| ProfileError::CampoInvalido("port".to_string()))?;
        let sql_dialect = SqlDialect::from_i64(self.sql_dialect)?;
        Ok(DadosPerfil {
            id: self.id.clone().unwrap_or_default(),
            name: self.name.clone(),
            host: self.host.clone(),
            port,
            database_path: self.database_path.clone(),
            username: self.username.clone(),
            role_name: self.role_name.clone(),
            charset: self.charset.clone(),
            sql_dialect,
            client_library_path: self.client_library_path.clone(),
            // `0` é aceito pela validação da UI e significa "use o padrão":
            // traduzir aqui evita um erro de comando para um valor que a tela
            // considera válido. Ausente tem o mesmo efeito.
            connect_timeout_ms: match self.connect_timeout_ms {
                None | Some(0) => firebird_domain::TIMEOUT_PADRAO_MS,
                Some(timeout) => timeout,
            },
        })
    }
}

/// Situação de um perfil, devolvida por `connection_get_status`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatusInfo {
    /// Perfil consultado.
    pub profile_id: String,
    /// Situação em minúsculas.
    pub status: StatusDto,
    /// Sessão ativa, quando houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Biblioteca cliente efetivamente carregada (Spec 03 §2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_library_path: Option<String>,
    /// Detalhe do erro quando `status === "error"`; nunca contém segredo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl From<&InformacoesStatus> for ConnectionStatusInfo {
    fn from(info: &InformacoesStatus) -> Self {
        ConnectionStatusInfo {
            profile_id: info.profile_id.clone(),
            status: StatusDto::from(&info.status),
            session_id: info.session_id.clone(),
            client_library_path: info.client_library_path.clone(),
            message: info.mensagem().map(str::to_string),
        }
    }
}

/// Retorno de `connection_test_profile` (o teste não registra o perfil).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    /// `true` quando a conexão de teste foi estabelecida.
    pub ok: bool,
    /// Biblioteca cliente efetivamente carregada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_library_path: Option<String>,
    /// Versão do servidor informada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    /// Detalhe legível da falha, sempre redigido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl From<&TesteConexao> for TestConnectionResult {
    fn from(teste: &TesteConexao) -> Self {
        TestConnectionResult {
            ok: teste.ok,
            client_library_path: teste.client_library_path.clone(),
            server_version: teste.server_version.clone(),
            message: teste.message.clone(),
        }
    }
}

/// Retorno de `connection_open`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenConnectionResult {
    /// Perfil conectado.
    pub profile_id: String,
    /// Sessão criada.
    pub session_id: String,
    /// Situação resultante.
    pub status: StatusDto,
    /// Biblioteca cliente efetivamente carregada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_library_path: Option<String>,
}

impl From<&SessaoAberta> for OpenConnectionResult {
    fn from(sessao: &SessaoAberta) -> Self {
        OpenConnectionResult {
            profile_id: sessao.profile_id.clone(),
            session_id: sessao.session_id.clone(),
            status: StatusDto::from(&sessao.status),
            client_library_path: sessao.client_library_path.clone(),
        }
    }
}

/// Retorno de `connection_close`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseConnectionResult {
    /// Perfil encerrado.
    pub profile_id: String,
    /// Situação resultante (desconectado).
    pub status: StatusDto,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    fn perfil() -> ConnectionProfile {
        ConnectionProfile::validar(DadosPerfil {
            id: "p1".to_string(),
            name: "Produção".to_string(),
            host: "servidor.local".to_string(),
            port: 3050,
            database_path: "/dados/app.fdb".to_string(),
            username: "SYSDBA".to_string(),
            role_name: Some("RDB$ADMIN".to_string()),
            charset: "UTF8".to_string(),
            sql_dialect: Some(SqlDialect::V3),
            client_library_path: Some("/usr/lib/libfbclient.so.3".to_string()),
            connect_timeout_ms: 5_000,
        })
        .expect("perfil válido")
    }

    fn entrada_json() -> serde_json::Value {
        serde_json::json!({
            "name": "Produção",
            "host": "servidor.local",
            "port": 3050,
            "databasePath": "/dados/app.fdb",
            "username": "SYSDBA",
            "charset": "UTF8",
            "sqlDialect": 3
        })
    }

    #[test]
    fn perfil_serializa_em_camel_case_sem_campo_de_senha() {
        let dto = ConnectionProfileDto::from(&perfil());
        let json = serde_json::to_value(&dto).expect("serializando");

        assert_eq!(json["databasePath"], "/dados/app.fdb");
        assert_eq!(json["roleName"], "RDB$ADMIN");
        assert_eq!(json["sqlDialect"], 3);
        assert_eq!(json["clientLibraryPath"], "/usr/lib/libfbclient.so.3");
        assert_eq!(json["connectTimeoutMs"], 5000);
        assert_eq!(json["port"], 3050);

        let objeto = json.as_object().expect("objeto");
        assert!(objeto.contains_key("id"));
        assert!(objeto.contains_key("name"));
        assert!(objeto.contains_key("host"));
        assert!(objeto.contains_key("username"));
        assert!(objeto.contains_key("charset"));
        for proibido in ["password", "senha", "secret", "secretRef", "passwd"] {
            assert!(
                !objeto.contains_key(proibido),
                "campo proibido {proibido} no DTO de perfil"
            );
        }
    }

    #[test]
    fn perfil_omite_opcionais_ausentes_em_vez_de_emitir_null() {
        let dto = ConnectionProfileDto::from(
            &ConnectionProfile::validar(DadosPerfil {
                role_name: None,
                sql_dialect: None,
                client_library_path: None,
                ..perfil().into_dados()
            })
            .expect("perfil válido"),
        );
        let json = serde_json::to_value(&dto).expect("serializando");
        assert!(!json.as_object().expect("objeto").contains_key("roleName"));
        assert!(!json.as_object().expect("objeto").contains_key("sqlDialect"));
        assert!(!json
            .as_object()
            .expect("objeto")
            .contains_key("clientLibraryPath"));
    }

    /// Auxiliar de teste: reconstrói dados de entrada a partir do perfil.
    trait DadosDoPerfilTeste {
        fn into_dados(self) -> DadosPerfil;
    }

    impl DadosDoPerfilTeste for ConnectionProfile {
        fn into_dados(self) -> DadosPerfil {
            DadosPerfil {
                id: self.id().to_string(),
                name: self.name().to_string(),
                host: self.host().to_string(),
                port: self.port(),
                database_path: self.database_path().to_string(),
                username: self.username().to_string(),
                role_name: self.role_name().map(str::to_string),
                charset: self.charset().to_string(),
                sql_dialect: self.sql_dialect(),
                client_library_path: self.client_library_path().map(str::to_string),
                connect_timeout_ms: self.connect_timeout_ms(),
            }
        }
    }

    #[test]
    fn entrada_desserializa_o_payload_da_ui_em_camel_case() {
        let dto: ConnectionProfileInputDto =
            serde_json::from_value(entrada_json()).expect("lendo entrada da UI");

        assert_eq!(dto.id, None, "sem id = criação");
        assert_eq!(dto.port, 3050);
        assert_eq!(dto.sql_dialect, Some(3));
        assert_eq!(dto.connect_timeout_ms, None);

        let dados = dto.para_dados().expect("convertendo");
        assert_eq!(dados.port, 3050);
        assert_eq!(dados.sql_dialect, Some(SqlDialect::V3));
        assert_eq!(
            dados.connect_timeout_ms,
            firebird_domain::TIMEOUT_PADRAO_MS,
            "timeout ausente usa o padrão do domínio"
        );
        assert_eq!(dados.id, "");
    }

    #[test]
    fn entrada_preserva_id_e_unicode_na_conversao() {
        let mut json = entrada_json();
        json["id"] = serde_json::json!("perfil-ç-1");
        json["name"] = serde_json::json!("Čatã do \"Norte\" — O'Brien");
        json["databasePath"] = serde_json::json!("/bases/ação.fdb");
        json["roleName"] = serde_json::json!("Papel \"R1\"");
        let dto: ConnectionProfileInputDto = serde_json::from_value(json).expect("lendo");

        let dados = dto.para_dados().expect("convertendo");
        assert_eq!(dados.id, "perfil-ç-1");
        assert_eq!(dados.name, "Čatã do \"Norte\" — O'Brien");
        assert_eq!(dados.database_path, "/bases/ação.fdb");
        assert_eq!(dados.role_name.as_deref(), Some("Papel \"R1\""));
    }

    #[test]
    fn entrada_com_porta_fora_da_faixa_vira_erro_de_perfil() {
        for porta in [0, 70_000, -1] {
            let mut json = entrada_json();
            json["port"] = serde_json::json!(porta);
            let dto: ConnectionProfileInputDto = serde_json::from_value(json).expect("lendo");
            assert_eq!(
                dto.para_dados(),
                Err(ProfileError::CampoInvalido("port".to_string())),
                "porta {porta} deveria ser recusada"
            );
        }
    }

    #[test]
    fn entrada_com_timeout_zero_ou_ausente_usa_o_padrao_do_dominio() {
        for timeout in [None, Some(0)] {
            let mut json = entrada_json();
            match timeout {
                Some(valor) => json["connectTimeoutMs"] = serde_json::json!(valor),
                None => {
                    json.as_object_mut()
                        .expect("objeto")
                        .remove("connectTimeoutMs");
                }
            }
            let dto: ConnectionProfileInputDto = serde_json::from_value(json).expect("lendo");
            assert_eq!(
                dto.para_dados().expect("convertendo").connect_timeout_ms,
                firebird_domain::TIMEOUT_PADRAO_MS,
                "timeout {timeout:?} deveria virar o padrão"
            );
        }
    }

    #[test]
    fn entrada_com_dialeto_desconhecido_vira_erro_de_perfil() {
        let mut json = entrada_json();
        json["sqlDialect"] = serde_json::json!(2);
        let dto: ConnectionProfileInputDto = serde_json::from_value(json).expect("lendo");
        assert_eq!(dto.para_dados(), Err(ProfileError::DialetoDesconhecido(2)));
    }

    #[test]
    fn status_serializa_em_minusculas() {
        let casos = [
            (ConnectionStatus::Disconnected, "disconnected"),
            (ConnectionStatus::Connecting, "connecting"),
            (ConnectionStatus::Connected, "connected"),
            (ConnectionStatus::Reconnecting, "reconnecting"),
            (ConnectionStatus::Error("x".into()), "error"),
        ];
        for (status, esperado) in casos {
            let dto = StatusDto::from(&status);
            assert_eq!(
                serde_json::to_value(dto).expect("serializando"),
                serde_json::json!(esperado)
            );
        }
    }

    #[test]
    fn status_info_serializa_campos_opcionais_e_mensagem_redigida() {
        let info = InformacoesStatus {
            profile_id: "p1".to_string(),
            status: ConnectionStatus::Error("password=*** recusada".to_string()),
            session_id: None,
            client_library_path: Some(
                PathBuf::from("/usr/lib/libfbclient.so.3")
                    .display()
                    .to_string(),
            ),
        };
        let json = serde_json::to_value(ConnectionStatusInfo::from(&info)).expect("serializando");

        assert_eq!(json["profileId"], "p1");
        assert_eq!(json["status"], "error");
        assert_eq!(json["clientLibraryPath"], "/usr/lib/libfbclient.so.3");
        assert!(json["message"].is_string(), "erro traz mensagem");
        assert!(!json.as_object().expect("objeto").contains_key("sessionId"));
    }

    #[test]
    fn open_e_close_serializam_o_contrato_da_ui() {
        let sessao = SessaoAberta {
            profile_id: "p1".to_string(),
            session_id: "sessao-1".to_string(),
            status: ConnectionStatus::Connected,
            client_library_path: Some("/usr/lib/libfbclient.so.3".to_string()),
            server_version: Some("4.0.1".to_string()),
        };
        let json = serde_json::to_value(OpenConnectionResult::from(&sessao)).expect("serializando");
        assert_eq!(json["profileId"], "p1");
        assert_eq!(json["sessionId"], "sessao-1");
        assert_eq!(json["status"], "connected");
        assert_eq!(json["clientLibraryPath"], "/usr/lib/libfbclient.so.3");

        let fechado = CloseConnectionResult {
            profile_id: "p1".to_string(),
            status: StatusDto::Disconnected,
        };
        let json = serde_json::to_value(fechado).expect("serializando");
        assert_eq!(json["profileId"], "p1");
        assert_eq!(json["status"], "disconnected");
    }

    #[test]
    fn teste_serializa_ok_com_biblioteca_e_versao() {
        let teste = TesteConexao {
            ok: true,
            client_library_path: Some("/usr/lib/libfbclient.so.3".to_string()),
            server_version: Some("4.0.1".to_string()),
            message: None,
        };
        let json = serde_json::to_value(TestConnectionResult::from(&teste)).expect("serializando");
        assert_eq!(json["ok"], true);
        assert_eq!(json["serverVersion"], "4.0.1");
        assert_eq!(json["clientLibraryPath"], "/usr/lib/libfbclient.so.3");
        assert!(!json.as_object().expect("objeto").contains_key("message"));
    }

    #[test]
    fn teste_com_falha_serializa_mensagem_sem_segredo() {
        let teste = TesteConexao {
            ok: false,
            client_library_path: None,
            server_version: None,
            message: Some("login failed (password=***)".to_string()),
        };
        let json = serde_json::to_value(TestConnectionResult::from(&teste)).expect("serializando");
        assert_eq!(json["ok"], false);
        assert!(json["message"].as_str().expect("mensagem").contains("***"));
    }
}
