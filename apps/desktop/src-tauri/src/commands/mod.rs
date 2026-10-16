//! Fronteira de comandos Tauri (Spec 10 §3).
//!
//! Cada módulo aqui é **casca fina**: valida o que a UI mandou, chama um
//! serviço de [`crate::application`] e traduz o resultado em DTO ou em
//! [`error::CommandError`] (`{ code, message }`, já redigido).

pub mod catalogo;
pub mod connections;
pub mod dto;
pub mod error;
pub mod ia;
pub mod sql;

pub use catalogo::catalog_get_node;
pub use connections::{
    connection_close, connection_get_status, connection_list_profiles, connection_open,
    connection_save_profile, connection_test_profile,
};
pub use error::{CommandError, CommandErrorCode};
pub use ia::{
    ia_context_preview, ia_generate, ia_interaction_accept_sql, ia_interactions_list,
    ia_provider_delete, ia_provider_list, ia_provider_save, ia_provider_test, ia_template_list,
    sql_risk_assessment, IaContextPreviewDto, IaGenerateDto, IaInteractionDto, IaTemplateDto,
    ProvedorIaDto, SqlRiskDto,
};
pub use sql::{
    history_clear, history_list, query_cancel, query_run, script_delete, script_get, script_list,
    script_save, table_columns, table_keys, transaction_begin, transaction_commit,
    transaction_rollback, ColunaTabelaDto, TransactionStatusDto,
};
