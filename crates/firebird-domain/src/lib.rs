//! Domínio puro do Firebird Studio Linux.
//!
//! Este crate não depende de Tauri, drivers nem infraestrutura: apenas
//! `thiserror` para erros tipados. Ver [`ServerVersion`], [`DomainError`] e
//! os modelos de metadados em [`metadata`].

pub mod ai;
pub mod catalog;
pub mod connection;
pub mod error;
pub mod metadata;
pub mod version;

/// Versão do crate de domínio (`Cargo.toml`), exposta para a UI do app.
///
/// Permite que a fronteira Tauri reporte a versão **real** do domínio sem
/// reimplementar `env!("CARGO_PKG_VERSION")` no crate desktop (que leria a
/// versão de si mesmo).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use ai::{ErroIa, PedidoIa, ProvedorIaConfig, RespostaIa, TarefaIa, TipoProvedorIa};
pub use catalog::{
    CategoriaCatalogo, ConstraintInfo, ConstraintTipo, DominioInfo, GeneratorInfo, IndiceInfo,
    InfoBanco, ParametroInfo, ProceduraInfo, TriggerInfo,
};
pub use connection::{
    ConnectionProfile, ConnectionStatus, DadosPerfil, PersistedProfile, ProfileError, SqlDialect,
    TIMEOUT_MAXIMO_MS, TIMEOUT_PADRAO_MS,
};
pub use error::{redigir_segredo, DomainError};
pub use metadata::{FieldInfo, RelationKind, RelationSummary};
pub use version::ServerVersion;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versao_e_semantica_e_pode_ser_lida_pelo_analisador_do_dominio() {
        // `ServerVersion` valida o formato `major.minor[.patch]`; se um dia a
        // versão do crate deixar de ser semântica, este teste avisa antes de
        // a UI exibir algo inesperado.
        assert!(
            ServerVersion::parse(VERSION).is_some(),
            "VERSION ({VERSION}) deveria ser parseável por ServerVersion"
        );
    }
}
