//! Domínio puro do Firebird Studio Linux.
//!
//! Este crate não depende de Tauri, drivers nem infraestrutura: apenas
//! `thiserror` para erros tipados. Ver [`ServerVersion`], [`DomainError`] e
//! os modelos de metadados em [`metadata`].

pub mod error;
pub mod metadata;
pub mod version;

pub use error::DomainError;
pub use metadata::{FieldInfo, RelationKind, RelationSummary};
pub use version::ServerVersion;
