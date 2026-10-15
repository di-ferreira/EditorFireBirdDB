//! Redação de segredos em mensagens de erro (FR-016).

/// Valor que substitui todo segredo encontrado no texto.
const MARCA: &str = "***";

/// Chaves cujo valor deve ser redigido, sem diferenciar maiúsculas de
/// minúsculas.
const CHAVES_SENSAVEIS: [&str; 4] = ["password=", "passwd=", "pwd=", "token="];

/// Redige `input` antes de ele entrar em erro, log ou DTO.
///
/// Substitui por `***` (a) cada valor de [`secrets`] que apareça no texto e
/// (b) o valor de chaves sensíveis conhecidas (`password=`, `passwd=`,
/// `pwd=`, `token=`), sem diferenciar maiúsculas de minúsculas.
///
/// Segredos vazios são ignorados: sem isso um segredo vazio apagaria a
/// mensagem inteira. A troca é feita por busca de substring (não por regex),
/// portanto nunca quebra uma sequência multibyte.
pub(crate) fn redact_secrets(input: &str, secrets: &[&str]) -> String {
    let mut texto = redigir_chaves(input);
    for segredo in secrets.iter().copied().filter(|s| !s.is_empty()) {
        texto = texto.replace(segredo, MARCA);
    }
    texto
}

/// Substitui por `***` o valor de cada chave sensível em `input`.
///
/// Percorre os bytes: toda correspondência começa em byte ASCII (logo, em
/// limite de caractere UTF-8) e o valor vai até o próximo espaço em branco ou
/// o fim da string.
fn redigir_chaves(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut saida = String::with_capacity(input.len());
    let mut i = 0;
    let mut copiar_de = 0;
    while i < bytes.len() {
        match tamanho_da_chave_em(bytes, i) {
            Some(tamanho) => {
                saida.push_str(&input[copiar_de..i]);
                saida.push_str(&input[i..i + tamanho]);
                saida.push_str(MARCA);
                i += tamanho;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                copiar_de = i;
            }
            None => i += 1,
        }
    }
    saida.push_str(&input[copiar_de..]);
    saida
}

/// Tamanho da chave sensível que começa em `i`, ou `None` se nenhuma casa.
fn tamanho_da_chave_em(bytes: &[u8], i: usize) -> Option<usize> {
    CHAVES_SENSAVEIS
        .iter()
        .map(|chave| chave.as_bytes())
        .find(|chave| {
            i + chave.len() <= bytes.len() && bytes[i..i + chave.len()].eq_ignore_ascii_case(chave)
        })
        .map(|chave| chave.len())
}

#[cfg(test)]
mod tests {
    use super::redact_secrets;

    #[test]
    fn redige_segredo_conhecido_em_qualquer_posicao() {
        let texto = redact_secrets("falhou ao gravar a senha Abc@123 no cofre", &["Abc@123"]);
        assert!(!texto.contains("Abc@123"), "segredo vazou: {texto}");
        assert!(texto.contains("***"), "esperava redação: {texto}");
    }

    #[test]
    fn redige_todas_as_ocorrencias_do_segredo() {
        let texto = redact_secrets("Abc@123 e Abc@123 e Abc@123", &["Abc@123"]);
        assert_eq!(texto, "*** e *** e ***");
    }

    #[test]
    fn redige_valor_de_chave_sensivel_mesmo_sem_o_segredo_conhecido() {
        for chave in ["password=", "passwd=", "pwd=", "token="] {
            let entrada = format!("{chave}segredo-123 negado");
            let texto = redact_secrets(&entrada, &[]);
            assert!(!texto.contains("segredo-123"), "vazou em {chave}: {texto}");
            assert!(texto.contains(&format!("{chave}***")), "texto: {texto}");
        }
    }

    #[test]
    fn redige_chave_sensivel_em_maiusculas() {
        let texto = redact_secrets("PASSWORD=segredo-123 negado", &[]);
        assert!(!texto.contains("segredo-123"), "vazou: {texto}");
        assert!(texto.contains("PASSWORD=***"), "texto: {texto}");
    }

    #[test]
    fn ignora_segredo_vazio_para_nao_apagar_a_mensagem_inteira() {
        let texto = redact_secrets("erro qualquer", &[""]);
        assert_eq!(texto, "erro qualquer");
    }

    #[test]
    fn preserva_texto_sem_segredo() {
        let texto = redact_secrets("chave firebird-studio não encontrada", &["outro"]);
        assert_eq!(texto, "chave firebird-studio não encontrada");
    }

    #[test]
    fn nao_quebra_caracteres_unicode() {
        let texto = redact_secrets("usuário maçã password=segredo-123 ç", &[]);
        assert!(!texto.contains("segredo-123"), "vazou: {texto}");
        assert!(texto.contains("usuário maçã"), "texto: {texto}");
        assert!(texto.contains("ç"), "texto: {texto}");
    }
}
