//! Erros tipados do cofre de segredos.

use crate::redact::redact_secrets;

/// Erros do cofre de segredos, independentes de plataforma.
///
/// O `Debug` é manual e já sai redigido, para que `unwrap()`, `panic!` e
/// `log!("{err:?}")` nunca exponham a senha que causou a falha (FR-016).
/// O `Display` literal deve chegar à tela já sanitizado pelo ponto de
/// construção do erro.
#[derive(PartialEq, Eq, thiserror::Error)]
pub enum SecurityError {
    /// O cofre do sistema operacional não pôde ser usado (sem sessão D-Bus,
    /// bloqueio, permissão negada, `libsecret` ausente).
    #[error("cofre indisponível: {0}")]
    Unavailable(String),

    /// A entrada solicitada não existe no cofre.
    #[error("segredo ausente")]
    NotFound,
}

impl std::fmt::Debug for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variante = match self {
            SecurityError::Unavailable(_) => "Unavailable",
            SecurityError::NotFound => "NotFound",
        };
        write!(f, "{variante}({})", redact_secrets(&self.to_string(), &[]))
    }
}

#[cfg(test)]
mod tests {
    use super::SecurityError;

    #[test]
    fn display_de_unavailable_mostra_a_mensagem_acionavel() {
        let err = SecurityError::Unavailable(
            "nenhum serviço de segredo disponível nesta sessão".to_string(),
        );
        assert!(err.to_string().contains("nenhum serviço de segredo"));
    }

    #[test]
    fn not_found_nao_expoe_detalhes_internos() {
        assert_eq!(SecurityError::NotFound.to_string(), "segredo ausente");
    }

    #[test]
    fn debug_nao_exibe_segredo_presente_na_mensagem() {
        let err = SecurityError::Unavailable("falhou com password=S3nh4! no cofre".to_string());
        let texto = format!("{err:?}");
        assert!(
            !texto.contains("S3nh4!"),
            "a senha não pode aparecer no Debug: {texto}"
        );
        assert!(texto.contains("***"), "o Debug deve redigir: {texto}");
    }

    #[test]
    fn debug_conserva_o_nome_da_variante() {
        assert!(format!("{:?}", SecurityError::NotFound).contains("NotFound"));
    }
}
