//! Adaptador Firebird do Firebird Studio Linux (Fase 0 — prova de conceito).
//!
//! Este crate isola o driver escolhido atrás do trait [`FirebirdAdapter`]
//! (ADR-002), com descoberta e validação de arquitetura da `libfbclient.so`
//! ([`ClientLibrary`]) e parâmetros de conexão cujo `Debug`/`Display` nunca
//! vazam a senha ([`ConnParams`], FR-016).

pub mod adapter;
pub mod library;

pub use adapter::{ConnParams, FirebirdAdapter, RsFbClientAdapter};
pub use library::ClientLibrary;

use firebird_domain::DomainError;

/// Monta um [`DomainError::Driver`] com a mensagem redigida (FR-016).
///
/// Reaproveita a redação do próprio domínio: constrói o erro e o submete a
/// [`DomainError::redacted`], removendo o prefixo literal do `Display` para
/// devolver apenas a mensagem. Nenhuma credencial em texto cru sobrevive ao
/// mapeamento de erro do driver.
pub(crate) fn erro_driver(message: String) -> DomainError {
    let vermelho = DomainError::Driver { message }.redacted();
    const PREFIXO: &str = "erro de driver: ";
    let mensagem = vermelho.strip_prefix(PREFIXO).unwrap_or(&vermelho);
    DomainError::Driver {
        message: mensagem.to_string(),
    }
}
