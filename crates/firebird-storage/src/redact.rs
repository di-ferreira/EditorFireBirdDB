//! Redação de segredos em mensagens (FR-016).
//!
//! Nenhuma mensagem de erro exibida em log, erro ou DTO pode conter senha,
//! token ou chave de API. O algoritmo percorre bytes sem regex e é o mesmo
//! padrão de `firebird_domain::error`: toda correspondência começa em byte
//! ASCII (logo, em limite de caractere UTF-8) e o valor vai até o próximo
//! espaço em branco ou o fim da string, sem quebrar sequências multibyte.

/// Chaves cujo valor deve ser redigido (FR-016).
///
/// A lista fica em ordem de leitura; [`secret_key_len_at`] escolhe a chave mais
/// longa entre as que casam no mesmo ponto.
const SECRET_KEYS: [&str; 8] = [
    "password=",
    "passwd=",
    "pwd=",
    "token=",
    "secret=",
    "secret_key=",
    "api_key=",
    "apikey=",
];

/// Redige em `input` o valor de cada chave sensível, trocando por `***`.
pub(crate) fn redact_secrets(input: &str) -> String {
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
///
/// Entre as chaves que casam vale a mais longa, para que `secret_key=` não
/// seja reconhecida como `secret=` deixando `_key=valor` exposto.
fn secret_key_len_at(bytes: &[u8], i: usize) -> Option<usize> {
    SECRET_KEYS
        .iter()
        .filter(|k| {
            i + k.len() <= bytes.len() && bytes[i..i + k.len()].eq_ignore_ascii_case(k.as_bytes())
        })
        .map(|k| k.len())
        .max()
}

#[cfg(test)]
mod tests {
    use super::redact_secrets;

    #[test]
    fn redige_valor_de_password_em_mensagem() {
        let saida = redact_secrets("falha ao abrir: password=abc123 host=localhost");
        assert!(
            !saida.contains("abc123"),
            "a senha não pode aparecer na saída: {saida}"
        );
        assert!(
            saida.contains("password=***"),
            "a senha deve sair redigida: {saida}"
        );
        assert!(
            saida.contains("host=localhost"),
            "trecho não sensível deve ser preservado: {saida}"
        );
    }

    #[test]
    fn redige_token_e_api_key_case_insensitive() {
        let saida = redact_secrets("Token=segredo123; API_KEY=chave-abc; PWD=xyz");
        assert!(!saida.contains("segredo123"), "token vazou: {saida}");
        assert!(!saida.contains("chave-abc"), "api key vazou: {saida}");
        assert!(!saida.contains("xyz"), "pwd vazou: {saida}");
        assert!(saida.contains("Token=***"), "{saida}");
        assert!(saida.contains("API_KEY=***"), "{saida}");
    }

    #[test]
    fn redige_secret_key_como_chave_unica() {
        let saida = redact_secrets("secret_key=abc-def e secret=ghi");
        assert!(!saida.contains("abc-def"), "secret_key vazou: {saida}");
        assert!(!saida.contains("ghi"), "secret vazou: {saida}");
        assert!(saida.contains("secret_key=***"), "{saida}");
        assert!(saida.contains("secret=***"), "{saida}");
    }

    #[test]
    fn redige_valor_ate_o_proximo_espanco_em_dsn() {
        let saida = redact_secrets("dsn=host:/db.fdb;user=SYSDBA;password=segredo;");
        assert!(!saida.contains("segredo"), "senha vazou: {saida}");
        assert_eq!(
            saida, "dsn=host:/db.fdb;user=SYSDBA;password=***",
            "o valor redigido vai até o espaço em branco"
        );
    }

    #[test]
    fn preserva_mensagem_sem_segredos_e_unicode() {
        let saida = redact_secrets("tabela inexistente: ČATÃ —ação");
        assert_eq!(saida, "tabela inexistente: ČATÃ —ação");
    }

    #[test]
    fn nao_quebra_sequencia_multibyte_ao_redigir() {
        let saida = redact_secrets("ação com password=abc€ ok");
        assert_eq!(saida, "ação com password=*** ok");
    }
}
