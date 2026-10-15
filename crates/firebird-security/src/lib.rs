//! Cofre de segredos do sistema operacional (FR-016).
//!
//! Este crate isola o acesso a senhas e API keys no cofre do SO (Linux:
//! Secret Service/libsecret, via `keyring`). Nenhuma senha é persistida pelo
//! SQLite local: o perfil guarda apenas a [`secret_ref`] (Spec 09).
//!
//! Regras que valem para todo o crate:
//!
//! * segredo nunca aparece em `Debug`, `Display`, log, panic ou erro (FR-016);
//! * cofre indisponível (sessão sem D-Bus, CI, headless) é
//!   [`SecurityError::Unavailable`] com mensagem acionável — nunca `panic!`;
//! * testes usam [`InMemorySecretStore`] e não tocam o cofre real.

pub mod error;
#[cfg(any(test, feature = "in-memory"))]
pub mod in_memory;
pub mod keyring_store;
pub mod redact;
pub mod secret_store;

#[cfg(any(test, feature = "in-memory"))]
pub use in_memory::InMemorySecretStore;
pub use keyring_store::KeyringSecretStore;
pub use secret_store::{obter_obrigatorio, secret_ref, SecretStore, SERVICE_NAME, SERVICE_PREFIX};

pub use error::SecurityError;

/// Versão do crate de segurança (`Cargo.toml`), exposta para a UI do app.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versao_do_cofre_e_semantica() {
        assert!(
            VERSION.split('.').count() >= 2,
            "VERSION ({VERSION}) deveria ser semântica"
        );
    }

    #[test]
    fn versao_e_reexportada_para_a_ui_do_app() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
