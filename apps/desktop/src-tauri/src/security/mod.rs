//! Cofre de segredos do backend desktop.
//!
//! O app usa o cofre do sistema operacional
//! ([`KeyringSecretStore`]); quando ele não está disponível (sessão sem
//! D-Bus, CI headless, `libsecret` ausente) o estado do app **degrada** para
//! [`CofreIndisponivel`], que responde a toda operação com
//! [`SecurityError::Unavailable`] e uma mensagem acionável — nunca `panic!` e
//! nunca um cofre em RAM que fingisse ser o do sistema (FR-016).

use firebird_domain::redigir_segredo;
use firebird_security::{SecretStore, SecurityError};

/// Placeholder usado quando o cofre do sistema operacional não respondeu.
///
/// Guardar a senha em memória seria o caminho errado: o app não saberia dizer
/// ao usuário que a sessão atual não sobrevive ao fechamento. Com este
/// cofre, `connection_open` falha com instrução para desbloquear o cofre.
pub struct CofreIndisponivel {
    /// Motivo já redigido, mostrado ao usuário.
    mensagem: String,
}

impl CofreIndisponivel {
    /// Cria o cofre degradado com o motivo informado.
    pub fn novo(motivo: impl Into<String>) -> Self {
        Self {
            mensagem: motivo.into(),
        }
    }

    /// Mensagem acionável exibida ao usuário.
    pub fn mensagem(&self) -> &str {
        &self.mensagem
    }
}

impl SecretStore for CofreIndisponivel {
    fn set(&self, _ref_: &str, _secret: &str) -> Result<(), SecurityError> {
        Err(self.indisponivel())
    }

    fn get(&self, _ref_: &str) -> Result<Option<String>, SecurityError> {
        Err(self.indisponivel())
    }

    fn delete(&self, _ref_: &str) -> Result<(), SecurityError> {
        // Apagar um segredo que nunca entrou no cofre é sucesso: assim
        // `connection_close` funciona mesmo em sessão degradada.
        Ok(())
    }
}

impl CofreIndisponivel {
    /// Erro de cofre com a mensagem acionável do desvio.
    fn indisponivel(&self) -> SecurityError {
        SecurityError::Unavailable(self.mensagem.clone())
    }
}

impl std::fmt::Debug for CofreIndisponivel {
    /// A mensagem vem do cofre do sistema e pode conter segredo na origem:
    /// sai redigida (FR-016).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CofreIndisponivel")
            .field("mensagem", &redigir_segredo(&self.mensagem))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_e_get_reportam_cofre_indisponivel_com_mensagem_acionavel() {
        let cofre = CofreIndisponivel::novo("nenhum serviço de segredo disponível nesta sessão");
        let erro = cofre.set("ref", "Abc@123").expect_err("set deve falhar");
        assert!(matches!(erro, SecurityError::Unavailable(_)));
        assert!(erro.to_string().contains("nenhum serviço de segredo"));

        let erro = cofre.get("ref").expect_err("get deve falhar");
        assert!(matches!(erro, SecurityError::Unavailable(_)));
    }

    #[test]
    fn delete_de_referencia_ausente_e_idempotente() {
        let cofre = CofreIndisponivel::novo("sem cofre");
        cofre.delete("ref").expect("primeiro delete");
        cofre.delete("ref").expect("segundo delete");
    }

    #[test]
    fn mensagem_nao_expoe_segredo() {
        let cofre = CofreIndisponivel::novo("falhou com password=Abc@123");
        let texto = format!("{cofre:?}");
        assert!(!texto.contains("Abc@123"), "vazou: {texto}");
        assert!(texto.contains("password=***"), "esperava redação: {texto}");
        assert_eq!(cofre.mensagem(), "falhou com password=Abc@123");
    }
}
