//! Erros da fronteira de comandos (Spec 10 §3).
//!
//! Todo comando devolve `Result<T, CommandError>`: o Tauri serializa o
//! `Err` como **rejeição** do `invoke`, e a UI lê `{ code, message }` —
//! nunca um panic e nunca um stack trace.
//!
//! O mapeamento é feito na fronteira: `ErroGerente`, `StorageError`,
//! `SecurityError`, `ProfileError` e `DomainError` viram um dos seis códigos
//! estáveis, e a `message` já sai redigida (FR-016).

use firebird_domain::{redigir_segredo, DomainError, ProfileError};
use firebird_security::SecurityError;
use firebird_storage::StorageError;

use crate::application::connection_manager::ErroGerente;

/// Código de erro exposto à UI (Spec 10 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CommandErrorCode {
    /// Falha ao conectar ao servidor ou de dados da conexão.
    ConnectionError,
    /// O servidor não suporta o recurso pedido.
    UnsupportedServerFeature,
    /// Falha na execução de uma consulta.
    QueryError,
    /// Operação negada (cofre bloqueado, permissão do servidor).
    PermissionDenied,
    /// Falha no provedor de IA.
    ProviderError,
    /// Falha sem classificação possível.
    Unknown,
}

/// Erro serializado como `{ code, message }` na rejeição do `invoke`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandError {
    /// Código estável, consumido pela UI para rótulo e tratamento.
    pub code: CommandErrorCode,
    /// Mensagem legível, **já redigida** e sem stack trace.
    pub message: String,
}

impl CommandError {
    /// Erro na execução de uma consulta SQL.
    pub fn consulta(mensagem: impl std::fmt::Display) -> Self {
        Self {
            code: CommandErrorCode::QueryError,
            message: redigir_segredo(&mensagem.to_string()),
        }
    }

    /// Erro de conexão (perfil inválido, ausente, cofre, falha do driver).
    pub fn conexao(mensagem: impl std::fmt::Display) -> Self {
        Self {
            code: CommandErrorCode::ConnectionError,
            message: redigir_segredo(&mensagem.to_string()),
        }
    }

    /// Erro sem classificação possível.
    pub fn desconhecido(mensagem: impl std::fmt::Display) -> Self {
        Self {
            code: CommandErrorCode::Unknown,
            message: redigir_segredo(&mensagem.to_string()),
        }
    }

    /// Recurso não suportado pelo servidor conectado.
    pub fn nao_suportado(mensagem: impl std::fmt::Display) -> Self {
        Self {
            code: CommandErrorCode::UnsupportedServerFeature,
            message: redigir_segredo(&mensagem.to_string()),
        }
    }

    /// Operação negada.
    pub fn negado(mensagem: impl std::fmt::Display) -> Self {
        Self {
            code: CommandErrorCode::PermissionDenied,
            message: redigir_segredo(&mensagem.to_string()),
        }
    }

    /// Falha de armazenamento local ou de infraestrutura do app — a UI deve
    /// orientar o usuário, e não sugerir retry cego.
    pub fn indisponivel(mensagem: impl std::fmt::Display) -> Self {
        Self {
            code: CommandErrorCode::ConnectionError,
            message: redigir_segredo(&mensagem.to_string()),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl From<ErroGerente> for CommandError {
    fn from(erro: ErroGerente) -> Self {
        match erro {
            // Já vem redigido na construção de `ErroGerente`; a redação aqui
            // é belt-and-suspenders para mensagens que carregam id de perfil.
            ErroGerente::PerfilNaoEncontrado(id) => {
                CommandError::conexao(format!("perfil de conexão não encontrado: {id}"))
            }
            ErroGerente::PerfilInvalido(motivo) => {
                CommandError::conexao(format!("perfil de conexão inválido: {motivo}"))
            }
            ErroGerente::SegredoAusente(id) => CommandError::conexao(format!(
                "a senha do perfil '{id}' não está guardada no cofre: informe-a ao conectar"
            )),
            ErroGerente::CofreIndisponivel(motivo) => CommandError::negado(motivo),
            ErroGerente::Conexao(mensagem) => CommandError::conexao(mensagem),
            ErroGerente::Indisponivel(mensagem) => CommandError::indisponivel(mensagem),
        }
    }
}

impl From<ProfileError> for CommandError {
    fn from(erro: ProfileError) -> Self {
        CommandError::conexao(format!("perfil de conexão inválido: {erro}"))
    }
}

impl From<SecurityError> for CommandError {
    fn from(erro: SecurityError) -> Self {
        match erro {
            // Cofre bloqueado/ausente é negativa do ambiente, não do servidor:
            // `PermissionDenied` diz à UI que não adianta repetir a tentativa.
            SecurityError::Unavailable(motivo) => CommandError::negado(motivo),
            SecurityError::NotFound => {
                CommandError::conexao("a senha não está guardada no cofre: informe-a ao conectar")
            }
        }
    }
}

impl From<StorageError> for CommandError {
    fn from(erro: StorageError) -> Self {
        match erro {
            StorageError::NotFound(id) => {
                CommandError::conexao(format!("perfil de conexão não encontrado: {id}"))
            }
            StorageError::Invalid(motivo) => {
                CommandError::conexao(format!("perfil de conexão inválido: {motivo}"))
            }
            // Falha de SQLite/migração é infraestrutura: `Unknown`, com a
            // mensagem já redigida pelo próprio storage.
            outra => CommandError::desconhecido(outra),
        }
    }
}

impl From<DomainError> for CommandError {
    fn from(erro: DomainError) -> Self {
        match erro {
            DomainError::Driver { message } => CommandError::conexao(message),
            DomainError::Catalog { message } => CommandError::conexao(message),
            DomainError::ParseVersion { raw } => CommandError::desconhecido(format!(
                "não foi possível interpretar a versão do servidor informada ({raw})"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erro_serializa_com_codigo_e_mensagem() {
        let erro = CommandError::conexao("perfil não encontrado: p1");
        let json = serde_json::to_value(&erro).expect("serializando");
        assert_eq!(json["code"], "ConnectionError");
        assert_eq!(json["message"], "perfil não encontrado: p1");
    }

    #[test]
    fn codigos_serializam_com_os_nomes_da_spec() {
        let casos = [
            (CommandError::nao_suportado("x"), "UnsupportedServerFeature"),
            (CommandError::conexao("x"), "ConnectionError"),
            (
                CommandError {
                    code: CommandErrorCode::QueryError,
                    message: "x".into(),
                },
                "QueryError",
            ),
            (CommandError::negado("x"), "PermissionDenied"),
            (
                CommandError {
                    code: CommandErrorCode::ProviderError,
                    message: "x".into(),
                },
                "ProviderError",
            ),
            (CommandError::desconhecido("x"), "Unknown"),
        ];
        for (erro, esperado) in casos {
            assert_eq!(
                serde_json::to_value(&erro).expect("serializando")["code"],
                esperado
            );
        }
    }

    #[test]
    fn codigos_sao_deserializaveis_de_volta() {
        let erro: CommandError =
            serde_json::from_str(r#"{"code":"ConnectionError","message":"x"}"#).expect("lendo");
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert_eq!(erro.message, "x");
    }

    #[test]
    fn mensagem_de_erro_redige_segredo() {
        let erro = CommandError::conexao("falhou com password=S3nh4! e token=abc");
        assert!(!erro.message.contains("S3nh4!"), "vazou: {}", erro.message);
        assert!(!erro.message.contains("abc"), "vazou: {}", erro.message);
        assert!(erro.message.contains("password=***"), "{}", erro.message);
    }

    #[test]
    fn erro_do_gerente_mapeia_para_connection_error() {
        let erro = CommandError::from(ErroGerente::PerfilNaoEncontrado("p1".into()));
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(erro.message.contains("p1"));
    }

    #[test]
    fn cofre_indisponivel_mapeia_para_permission_denied() {
        let erro = CommandError::from(ErroGerente::CofreIndisponivel(
            "nenhum serviço de segredo disponível".into(),
        ));
        assert_eq!(erro.code, CommandErrorCode::PermissionDenied);
        assert!(erro.message.contains("nenhum serviço de segredo"));
    }

    #[test]
    fn erro_do_driver_mapeia_para_connection_error_com_mensagem_redigida() {
        let erro = CommandError::from(DomainError::Driver {
            message: "connection refused (password=S3nh4!)".into(),
        });
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(!erro.message.contains("S3nh4!"), "vazou: {}", erro.message);
    }

    #[test]
    fn erro_de_storage_mapeia_not_found_invalid_e_desconhecido() {
        assert_eq!(
            CommandError::from(StorageError::NotFound("p1".into())).code,
            CommandErrorCode::ConnectionError
        );
        assert_eq!(
            CommandError::from(StorageError::Invalid("nome".into())).code,
            CommandErrorCode::ConnectionError
        );
        assert_eq!(
            CommandError::from(StorageError::Migration("aplicar 0001".into())).code,
            CommandErrorCode::Unknown
        );
    }

    #[test]
    fn erro_de_seguranca_sem_cofre_mapeia_para_permission_denied_e_secredo_ausente_para_conexao() {
        assert_eq!(
            CommandError::from(SecurityError::Unavailable("sem serviço".into())).code,
            CommandErrorCode::PermissionDenied
        );
        assert_eq!(
            CommandError::from(SecurityError::NotFound).code,
            CommandErrorCode::ConnectionError
        );
    }

    #[test]
    fn erro_de_perfil_invalido_mapeia_para_connection_error() {
        let erro = CommandError::from(ProfileError::CampoInvalido("port".into()));
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(erro.message.contains("port"));
    }

    #[test]
    fn mensagem_nao_traz_stack_trace() {
        let erro = CommandError::desconhecido("falhou");
        assert!(!erro.message.contains('\n'), "{}", erro.message);
        assert!(!erro.message.contains("src/"), "{}", erro.message);
        assert!(!erro.message.contains("panic"), "{}", erro.message);
    }

    #[test]
    fn display_nao_vaza_segredo_redigido() {
        let erro = CommandError::conexao("password=Abc@123 recusada");
        let texto = erro.to_string();
        assert!(!texto.contains("Abc@123"), "vazou: {texto}");
    }
}
