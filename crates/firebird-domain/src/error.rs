//! Erros tipados do domínio Firebird Studio Linux.
//!
//! As mensagens podem conter dados vindos do driver ou do catálogo; para
//! exibição em log, erro ou DTO, use sempre [`DomainError::redacted`] (FR-016).

/// Erros de domínio, independentes de driver e de Tauri.
///
/// O [`Debug`](std::fmt::Debug) é manual e já sai redigido, para que panics,
/// `main() -> Result<_, DomainError>` e `log!("{err:?}")` nunca vaziem
/// credenciais (FR-016); o `Display` é literal e deve ser exibido apenas
/// via [`DomainError::redacted`].
#[derive(thiserror::Error)]
pub enum DomainError {
    /// Falha ao interpretar a versão do servidor.
    #[error("não foi possível interpretar a versão do servidor: {raw}")]
    ParseVersion { raw: String },

    /// Erro reportado pelo driver Firebird.
    #[error("erro de driver: {message}")]
    Driver { message: String },

    /// Erro na consulta de catálogo (`RDB$...`).
    #[error("erro de catálogo: {message}")]
    Catalog { message: String },

    /// Consulta interrompida por cancelamento explícito do usuário.
    #[error("consulta cancelada pelo usuário")]
    Cancelled,
}

impl std::fmt::Debug for DomainError {
    /// Imprime o nome da variante e a mensagem com credenciais redigidas.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variante = match self {
            DomainError::ParseVersion { .. } => "ParseVersion",
            DomainError::Driver { .. } => "Driver",
            DomainError::Catalog { .. } => "Catalog",
            DomainError::Cancelled => "Cancelled",
        };
        write!(f, "{variante}({})", redact_secrets(&self.to_string()))
    }
}

impl DomainError {
    /// Renderiza o erro com credenciais redigidas: valores após `password=`,
    /// `passwd=`, `pwd=` e `token=` (case-insensitive) viram `***`.
    ///
    /// Nunca devolva o [`std::fmt::Display`] cru para log, erro ou DTO.
    pub fn redacted(self) -> String {
        redact_secrets(&self.to_string())
    }
}

/// Redige segredos de um **texto livre** que não vem de um [`DomainError`].
///
/// Reaproveitada pela camada de aplicação ao montar mensagens de erro que
/// combinam texto do driver com contexto (host, id do perfil): o valor de
/// `password=`, `passwd=`, `pwd=` e `token=` vira `***` (FR-016).
pub fn redigir_segredo(texto: &str) -> String {
    redact_secrets(texto)
}

/// Chaves cujo valor deve ser redigido (FR-016).
const SECRET_KEYS: [&str; 4] = ["password=", "passwd=", "pwd=", "token="];

/// Substitui, em `input`, o valor de cada chave sensível por `***`.
///
/// Percorre os bytes sem regex: toda correspondência começa em byte ASCII
/// (logo, em limite de caractere UTF-8) e o valor vai até o próximo espaço
/// em branco ou o fim da string, sem quebrar sequências multibyte.
fn redact_secrets(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut copiar_de = 0;
    while i < bytes.len() {
        if let Some(tam) = secret_key_len_at(bytes, i) {
            out.push_str(&input[copiar_de..i]);
            out.push_str(&input[i..i + tam]);
            out.push_str("***");
            i += tam;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            copiar_de = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&input[copiar_de..]);
    out
}

/// Retorna o tamanho da chave sensível que começa em `i`, sem diferenciar
/// maiúsculas de minúsculas, ou `None` se nenhuma chave casa ali.
fn secret_key_len_at(bytes: &[u8], i: usize) -> Option<usize> {
    SECRET_KEYS
        .iter()
        .map(|k| k.as_bytes())
        .find(|k| i + k.len() <= bytes.len() && bytes[i..i + k.len()].eq_ignore_ascii_case(k))
        .map(|k| k.len())
}

#[cfg(test)]
mod tests {
    use super::{redigir_segredo, DomainError};

    #[test]
    fn redigir_segredo_trata_texto_livre_da_aplicacao() {
        let texto = redigir_segredo("conexão com host=h password=S3nh4! falhou");
        assert!(!texto.contains("S3nh4!"), "vazou: {texto}");
        assert!(texto.contains("password=***"), "{texto}");
        assert!(
            texto.contains("host=h"),
            "contexto não sensível deve sobreviver: {texto}"
        );
    }

    #[test]
    fn redigir_segredo_preserva_unicode_e_aspas() {
        let texto = redigir_segredo("usuário 'Ação' pwd=Abc@123 — falhou");
        assert!(texto.contains("Ação"), "{texto}");
        assert!(!texto.contains("Abc@123"), "{texto}");
    }

    #[test]
    fn redigir_segredo_de_texto_sem_segredo_devolve_o_texto() {
        assert_eq!(redigir_segredo("nada a redigir"), "nada a redigir");
    }

    #[test]
    fn redacted_remove_senha_de_mensagem_do_driver() {
        let err = DomainError::Driver {
            message: "password=abc123 denied".to_string(),
        };
        let texto = err.redacted();
        assert!(
            !texto.contains("abc123"),
            "a senha não pode aparecer na saída: {texto}"
        );
        assert!(
            texto.contains("password=***"),
            "a senha deve sair redigida como password=***: {texto}"
        );
    }

    #[test]
    fn redacted_remove_token_de_mensagem_de_catalogo() {
        let err = DomainError::Catalog {
            message: "token=segredo123 ao ler RDB$RELATIONS".to_string(),
        };
        let texto = err.redacted();
        assert!(!texto.contains("segredo123"), "token vazou: {texto}");
        assert!(
            texto.contains("token=***"),
            "token deve sair redigido: {texto}"
        );
    }

    #[test]
    fn redacted_preserva_mensagem_sem_segredos() {
        let err = DomainError::ParseVersion {
            raw: "versão inválida".to_string(),
        };
        assert!(err.redacted().contains("versão inválida"));
    }

    #[test]
    fn debug_nao_exibe_senha() {
        let err = DomainError::Driver {
            message: "password=abc123 denied".to_string(),
        };
        let texto = format!("{err:?}");
        assert!(
            !texto.contains("abc123"),
            "a senha não pode aparecer no Debug: {texto}"
        );
        assert!(
            texto.contains("password=***"),
            "o Debug deve exibir a senha redigida: {texto}"
        );
        assert!(
            texto.contains("Driver"),
            "o Debug deve manter o nome da variante: {texto}"
        );
    }
}
