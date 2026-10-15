//! Cofre do sistema operacional (Linux: Secret Service/libsecret).
//!
//! O acesso é feito pelo crate `keyring`, que resolve o cofre nativo da
//! plataforma: Secret Service no Linux, Keychain no macOS e Gerenciador de
//! Credenciais no Windows. O serviço gravado é sempre
//! [`SERVICE_NAME`](crate::SERVICE_NAME) ("firebird-studio"), e o usuário da
//! entrada é a referência sem o prefixo do serviço — assim a chave aparece
//! limpa no `gnome-keyring`/`kwallet`.
//!
//! Ambiente sem cofre (CI headless, sessão sem D-Bus, `libsecret` ausente)
//! resulta em [`SecurityError::Unavailable`] com mensagem acionável, nunca em
//! `panic!` (FR-016).

use crate::error::SecurityError;
use crate::redact::redact_secrets;
use crate::secret_store::{SecretStore, SERVICE_NAME, SERVICE_PREFIX};

/// Usuário da entrada usada apenas para provar que o cofre do SO respondeu.
///
/// Nunca recebe segredo: serve de sonda de disponibilidade na construção.
const SONDA_USUARIO: &str = "disponibilidade";

/// Cofre de segredos do sistema operacional.
pub struct KeyringSecretStore {
    /// Sonda de disponibilidade: [`KeyringSecretStore::new`] só existe se a
    /// entrada foi construída, ou seja, se o cofre do SO inicializou.
    sonda: keyring::Entry,
}

impl KeyringSecretStore {
    /// Cria o cofre, verificando que o serviço de segredos do sistema está
    /// disponível.
    ///
    /// Em CI/headless (sem sessão de Secret Service) retorna
    /// [`SecurityError::Unavailable`] — a aplicação decide se segue sem cofre
    /// ou orienta o usuário; nunca entra em pânico.
    pub fn new() -> Result<Self, SecurityError> {
        let sonda = keyring::Entry::new(SERVICE_NAME, SONDA_USUARIO)
            .map_err(|err| mapear_escrita(err, None))?;
        Ok(Self { sonda })
    }

    /// `true` quando o cofre do sistema realmente respondeu à sonda.
    ///
    /// A entrada de sonda não tem senha gravada, então a resposta
    /// [`keyring::Error::NoEntry`] prova a ida e volta ao serviço de
    /// segredos sem queimar segredo algum.
    pub fn is_available(&self) -> bool {
        matches!(self.sonda.get_password(), Err(keyring::Error::NoEntry))
    }

    /// Constrói a entrada do cofre para uma referência.
    fn entry(ref_: &str) -> Result<keyring::Entry, SecurityError> {
        keyring::Entry::new(SERVICE_NAME, usuario_da_referencia(ref_))
            .map_err(|err| mapear_escrita(err, None))
    }
}

impl SecretStore for KeyringSecretStore {
    fn set(&self, ref_: &str, secret: &str) -> Result<(), SecurityError> {
        Self::entry(ref_)?
            .set_password(secret)
            // O segredo pode reaparecer na mensagem do cofre: redige antes.
            .map_err(|err| mapear_escrita(err, Some(secret)))
    }

    fn get(&self, ref_: &str) -> Result<Option<String>, SecurityError> {
        match Self::entry(ref_)?.get_password() {
            Ok(segredo) => Ok(Some(segredo)),
            Err(err) => mapear_leitura(err),
        }
    }

    fn delete(&self, ref_: &str) -> Result<(), SecurityError> {
        match Self::entry(ref_)?.delete_credential() {
            Ok(()) => Ok(()),
            // Apagar duas vezes não é erro.
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(mapear_escrita(err, None)),
        }
    }
}

impl std::fmt::Debug for KeyringSecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyringSecretStore")
            .field("servico", &SERVICE_NAME)
            .finish_non_exhaustive()
    }
}

/// Usuário da entrada do cofre para uma referência de segredo.
///
/// Remove o prefixo do serviço, se presente: `"firebird-studio/profile/abc"`
/// vira `"profile/abc"`, já que o serviço gravado é `firebird-studio`.
fn usuario_da_referencia(ref_: &str) -> &str {
    ref_.strip_prefix(SERVICE_PREFIX).unwrap_or(ref_)
}

/// Traduz erro do `keyring` em erro de cofre, redactando o segredo conhecido.
fn mapear_escrita(err: keyring::Error, segredo: Option<&str>) -> SecurityError {
    let connu = segredo.into_iter().collect::<Vec<_>>();
    let detalhe = redact_secrets(&err.to_string(), &connu);
    SecurityError::Unavailable(format!(
        "{detalhe}; desbloqueie o cofre do sistema ou rode o aplicativo em uma \
         sessão com o serviço de segredos ativo (verifique se '{SERVICE_NAME}' \
         está disponível no login do sistema)"
    ))
}

/// Traduz erro de leitura do `keyring`; entrada ausente não é erro (`None`).
fn mapear_leitura(err: keyring::Error) -> Result<Option<String>, SecurityError> {
    match err {
        keyring::Error::NoEntry => Ok(None),
        outra => Err(mapear_escrita(outra, None)),
    }
}

/// Erro sintético de plataforma, sem passar pelo cofre real, para testar a
/// redação da mensagem.
#[cfg(test)]
fn erro_de_plataforma(mensagem: &str) -> keyring::Error {
    keyring::Error::PlatformFailure(Box::new(std::io::Error::other(mensagem)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_store::secret_ref;

    #[test]
    fn usuario_da_referencia_remove_o_prefixo_do_servico() {
        assert_eq!(
            usuario_da_referencia("firebird-studio/profile/perfil-1"),
            "profile/perfil-1"
        );
    }

    #[test]
    fn usuario_da_referencia_aceita_chave_sem_prefixo() {
        assert_eq!(
            usuario_da_referencia("profile/perfil-1"),
            "profile/perfil-1"
        );
    }

    #[test]
    fn gravacao_redige_o_segredo_da_mensagem_de_erro() {
        let err = mapear_escrita(erro_de_plataforma("negado para Abc@123"), Some("Abc@123"));
        let texto = err.to_string();
        assert!(!texto.contains("Abc@123"), "segredo vazou: {texto}");
        assert!(texto.contains("***"), "esperava redação: {texto}");
    }

    #[test]
    fn gravacao_relata_cofre_indisponivel_com_mensagem_acionavel() {
        let err = mapear_escrita(erro_de_plataforma("org.freedesktop.secrets negado"), None);
        assert!(
            matches!(err, SecurityError::Unavailable(_)),
            "esperava Unavailable, veio {err:?}"
        );
        let texto = err.to_string();
        assert!(texto.contains("cofre indisponível"), "texto: {texto}");
    }

    #[test]
    fn leitura_de_entrada_inexistente_devolve_none_e_nao_erro() {
        assert_eq!(mapear_leitura(keyring::Error::NoEntry), Ok(None));
    }

    #[test]
    fn leitura_redige_segredo_de_mensagem_de_plataforma() {
        let err = mapear_leitura(erro_de_plataforma("blob continha pwd=Abc@123"))
            .expect_err("esperava erro");
        assert!(!err.to_string().contains("Abc@123"), "vazou: {err}");
    }

    #[test]
    fn leitura_redige_chave_sensivel_independente_do_segredo_conhecido() {
        let err = mapear_leitura(erro_de_plataforma("token=Abc@123 expirado"))
            .expect_err("esperava erro");
        assert!(!err.to_string().contains("Abc@123"), "vazou: {err}");
    }

    #[test]
    fn cofre_sem_sessao_nao_entra_em_panico() {
        // CI/headless: pode ser `true` ou `false` aqui, mas nunca um panic.
        match KeyringSecretStore::new() {
            Ok(store) => {
                let _ = store.is_available();
            }
            Err(err) => assert!(
                matches!(err, SecurityError::Unavailable(_)),
                "esperava Unavailable, veio {err:?}"
            ),
        }
    }

    #[test]
    fn debug_do_cofre_nao_expoe_nada_alem_do_servico() {
        match KeyringSecretStore::new() {
            Ok(store) => {
                let texto = format!("{store:?}");
                assert!(texto.contains(SERVICE_NAME), "texto: {texto}");
                assert!(!texto.contains("password"), "texto: {texto}");
            }
            Err(err) => {
                assert!(
                    matches!(err, SecurityError::Unavailable(_)),
                    "esperava Unavailable, veio {err:?}"
                );
                let texto = err.to_string();
                assert!(texto.contains("cofre indisponível"), "texto: {texto}");
            }
        }
    }

    #[test]
    fn referencia_do_perfil_vira_chave_do_cofre() {
        assert_eq!(
            usuario_da_referencia(&secret_ref("perfil-7")),
            "profile/perfil-7"
        );
        assert!(secret_ref("perfil-7").starts_with(SERVICE_PREFIX));
    }

    /// Teste opt-in do cofre REAL do sistema (exige sessão gráfica com
    /// Secret Service). Rodar com:
    ///
    /// ```text
    /// cargo test -p firebird-security -- --ignored cofre_real_do_sistema
    /// ```
    #[test]
    #[ignore = "exige o cofre do sistema operacional (Secret Service) disponível"]
    fn cofre_real_do_sistema_guarda_le_e_apaga() {
        let store = KeyringSecretStore::new().expect("cofre indisponível neste ambiente");
        let id = format!("teste-{}", std::process::id());
        let ref_ = secret_ref(&id);
        let segredo = "Abc@123-cofre-real";

        store.set(&ref_, segredo).expect("set falhou");
        assert_eq!(store.get(&ref_).expect("get falhou"), Some(segredo.into()));
        store.delete(&ref_).expect("delete falhou");
        assert_eq!(store.get(&ref_).expect("get falhou"), None);
    }

    /// O mesmo fluxo, contra o cofre real, provando que a referência não
    /// precisa ser criada antes de `delete`.
    #[test]
    #[ignore = "exige o cofre do sistema operacional (Secret Service) disponível"]
    fn cofre_real_do_sistema_apagar_ausente_e_idempotente() {
        let store = KeyringSecretStore::new().expect("cofre indisponível neste ambiente");
        let ref_ = secret_ref(&format!("inexistente-{}", std::process::id()));
        store.delete(&ref_).expect("primeiro delete falhou");
        store.delete(&ref_).expect("segundo delete falhou");
    }
}
