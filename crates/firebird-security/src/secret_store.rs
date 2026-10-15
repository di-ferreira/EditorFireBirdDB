//! Contrato do cofre de segredos e chave determinística por perfil.

use crate::error::SecurityError;

/// Nome do serviço gravado no cofre do sistema operacional.
pub const SERVICE_NAME: &str = "firebird-studio";

/// Prefixo comum a toda referência de segredo deste aplicativo.
pub const SERVICE_PREFIX: &str = "firebird-studio/";

/// Armazenamento de segredos legível por máquina (senhas, API keys).
///
/// Implementações nunca registram o segredo em log e nunca o incluem na
/// mensagem de erro (FR-016). `delete` de uma referência ausente é sucesso
/// idempotente: apagar duas vezes não é erro.
pub trait SecretStore: Send + Sync {
    /// Grava (ou substitui) o segredo da referência.
    fn set(&self, ref_: &str, secret: &str) -> Result<(), SecurityError>;

    /// Lê o segredo da referência, ou `None` se não houver entrada.
    fn get(&self, ref_: &str) -> Result<Option<String>, SecurityError>;

    /// Remove a entrada da referência. Idempotente.
    fn delete(&self, ref_: &str) -> Result<(), SecurityError>;
}

/// Chave do cofre para um perfil de conexão.
///
/// Determinística (`"firebird-studio/profile/<id>"`) e sem PII: carrega apenas
/// o id do perfil, nunca host, usuário ou nome. É o valor persistido na coluna
/// `secret_ref` do SQLite local (Spec 09) — a senha em si fica só no cofre do SO.
pub fn secret_ref(profile_id: &str) -> String {
    format!("{SERVICE_PREFIX}profile/{profile_id}")
}

/// Lê um segredo que é obrigatório para a operação (abrir uma conexão,
/// por exemplo): referência sem entrada vira [`SecurityError::NotFound`] em
/// vez de `Ok(None)`, para o chamador não precisar tratar dois casos.
pub fn obter_obrigatorio(loja: &dyn SecretStore, ref_: &str) -> Result<String, SecurityError> {
    loja.get(ref_)?.ok_or(SecurityError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemorySecretStore;

    #[test]
    fn obter_obrigatorio_devolve_o_segredo_gravado() {
        let store = InMemorySecretStore::new();
        store
            .set("firebird-studio/profile/p1", "Abc@123")
            .expect("set falhou");
        let segredo =
            obter_obrigatorio(&store, "firebird-studio/profile/p1").expect("esperava o segredo");
        assert_eq!(segredo, "Abc@123");
    }

    #[test]
    fn obter_obrigatorio_sem_entrada_devolve_not_found() {
        let store = InMemorySecretStore::new();
        let err =
            obter_obrigatorio(&store, "firebird-studio/profile/p1").expect_err("esperava NotFound");
        assert_eq!(err, SecurityError::NotFound);
    }

    #[test]
    fn secret_ref_usa_o_prefixo_do_servico_e_o_id_do_perfil() {
        assert_eq!(secret_ref("perfil-1"), "firebird-studio/profile/perfil-1");
    }

    #[test]
    fn secret_ref_e_deterministica_para_o_mesmo_perfil() {
        assert_eq!(secret_ref("abc-123"), secret_ref("abc-123"));
    }

    #[test]
    fn secret_ref_diferencia_perfis_diferentes() {
        assert_ne!(secret_ref("perfil-1"), secret_ref("perfil-2"));
    }

    #[test]
    fn secret_ref_comeca_com_o_nome_do_servico_gravado_no_cofre() {
        assert!(secret_ref("p1").starts_with(SERVICE_NAME));
        assert!(secret_ref("p1").starts_with(SERVICE_PREFIX));
    }

    #[test]
    fn secret_ref_carrega_somente_o_id_do_perfil() {
        let referencia = secret_ref("perfil-xyz");
        let sufixo = referencia
            .strip_prefix(SERVICE_PREFIX)
            .expect("deveria começar com o prefixo do serviço");
        assert_eq!(sufixo, "profile/perfil-xyz");
        assert!(
            !referencia.contains(' '),
            "a chave do cofre não deve conter espaços: {referencia}"
        );
    }
}
