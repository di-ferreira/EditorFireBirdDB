//! Fronteira de comandos para o editor SQL (Spec 05, 06, 09 §histórico/scripts).

use std::time::Instant;

use crate::application::connection_manager::{ConnectionManager, ErroGerente};
use crate::application::estado::AppState;
use crate::commands::error::CommandError;
use firebird_storage::{agora_iso8601, QueryHistoryEntry, SavedScript};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResultDto {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    pub elapsed_ms: u128,
    pub kind: String,
    pub affected: Option<usize>,
    pub message: Option<String>,
    /// `true` quando existe ao menos uma linha depois da janela devolvida.
    pub has_more: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedScriptDto {
    pub id: String,
    pub name: String,
    pub content: String,
    pub connection_profile_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryDto {
    pub id: String,
    pub connection_profile_id: Option<String>,
    pub query_text: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub rows_returned: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub executed_at: String,
    pub execution_mode: String,
}

impl From<firebird_storage::SavedScript> for SavedScriptDto {
    fn from(s: firebird_storage::SavedScript) -> Self {
        SavedScriptDto {
            id: s.id,
            name: s.name,
            content: s.content,
            connection_profile_id: s.connection_profile_id,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

impl From<firebird_storage::QueryHistoryEntry> for HistoryEntryDto {
    fn from(e: firebird_storage::QueryHistoryEntry) -> Self {
        HistoryEntryDto {
            id: e.id,
            connection_profile_id: e.connection_profile_id,
            query_text: e.query_text,
            status: e.status,
            duration_ms: e.duration_ms,
            rows_returned: e.rows_returned,
            error_code: e.error_code,
            error_message: e.error_message,
            executed_at: e.executed_at,
            execution_mode: e.execution_mode,
        }
    }
}

fn erro_storage(e: firebird_storage::StorageError) -> CommandError {
    CommandError::indisponivel(e.to_string())
}

#[tauri::command]
pub fn query_run(
    profile_id: String,
    sql: String,
    query_id: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
    deps: tauri::State<'_, AppState>,
) -> Result<QueryResultDto, CommandError> {
    let manager: &ConnectionManager = deps.manager();
    let trimmed = sql.trim().to_string();
    let lower = trimmed.to_lowercase();
    let is_query = lower.starts_with("select") || lower.starts_with("with");

    let inicio = Instant::now();
    let mut error_code: Option<String> = None;
    let mut error_message: Option<String> = None;
    let mut rows_returned: Option<i64> = None;

    // Caminho paginado (SELECT/WITH): busca só a janela pedida no servidor e
    // permite cancelamento cooperativo por linha.
    if let Some(limit) = limit {
        if !is_query {
            return Err(CommandError::consulta(
                "paginação só se aplica a consultas (SELECT/WITH); use Executar para DML/DDL",
            ));
        }
        let limit = limit.clamp(1, 5_000) as usize;
        let offset = offset.unwrap_or(0).max(0) as usize;
        let query_id =
            query_id.unwrap_or_else(|| crate::application::connection_manager::novo_id("q"));
        let bandeira = deps.registrar_cancelamento(&query_id);
        let bruto = trimmed.clone();
        let resultado = manager.executar_no_adaptador_conectado(&profile_id, |adapter| {
            adapter
                .query_page_with_columns(&bruto, &[], offset, limit, Some(bandeira.as_ref()))
                .map_err(|e| {
                    crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
                })
        });
        deps.encerrar_cancelamento(&query_id);

        return match resultado {
            Ok((cols, rows, tem_mais)) => {
                let total = offset + rows.len() + usize::from(tem_mais);
                if deps.armazenamento_disponivel() && offset == 0 {
                    let entry = QueryHistoryEntry {
                        id: crate::application::connection_manager::novo_id("hist"),
                        connection_profile_id: Some(profile_id.clone()),
                        query_text: trimmed.clone(),
                        status: "ok".to_string(),
                        duration_ms: Some(inicio.elapsed().as_millis() as i64),
                        rows_returned: Some(rows.len() as i64),
                        error_code: None,
                        error_message: None,
                        executed_at: agora_iso8601(),
                        execution_mode: "manual".to_string(),
                    };
                    let _ = deps.repos().add_history(&entry);
                }
                Ok(QueryResultDto {
                    columns: if cols.is_empty() {
                        (0..rows.first().map(|r| r.len()).unwrap_or(0))
                            .map(|i| format!("COL{i}"))
                            .collect()
                    } else {
                        cols
                    },
                    rows,
                    row_count: total,
                    elapsed_ms: inicio.elapsed().as_millis(),
                    kind: "rows".to_string(),
                    affected: None,
                    message: None,
                    has_more: Some(tem_mais),
                })
            }
            Err(e) if e.to_string().contains("cancelada") => Err(CommandError::cancelada()),
            Err(e) => Err(CommandError::consulta(e.to_string())),
        };
    }

    let result: Result<QueryResultDto, CommandError> = if is_query {
        match manager.executar_no_adaptador_conectado(&profile_id, |adapter| {
            adapter.query_rows_with_columns(&trimmed, &[]).map_err(|e| {
                crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
            })
        }) {
            Ok((cols, rows)) => {
                let rc = rows.len();
                rows_returned = Some(rc as i64);
                Ok(QueryResultDto {
                    columns: if cols.is_empty() {
                        (0..rows.first().map(|r| r.len()).unwrap_or(0))
                            .map(|i| format!("COL{}", i))
                            .collect()
                    } else {
                        cols
                    },
                    rows,
                    row_count: rc,
                    elapsed_ms: inicio.elapsed().as_millis(),
                    kind: "rows".to_string(),
                    affected: None,
                    message: None,
                    has_more: None,
                })
            }
            Err(e) => {
                error_code = Some("QueryError".to_string());
                error_message = Some(e.to_string());
                Err(CommandError::consulta(e.to_string()))
            }
        }
    } else {
        match manager.executar_no_adaptador_conectado(&profile_id, |adapter| {
            adapter.execute(&trimmed, &[]).map_err(|e| {
                crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
            })
        }) {
            Ok(n) => Ok(QueryResultDto {
                columns: vec![],
                rows: vec![],
                row_count: 0,
                elapsed_ms: inicio.elapsed().as_millis(),
                kind: "affected".to_string(),
                affected: Some(n),
                message: None,
                has_more: Some(false),
            }),
            Err(e) => {
                error_code = Some("QueryError".to_string());
                error_message = Some(e.to_string());
                Err(CommandError::consulta(e.to_string()))
            }
        }
    };

    if deps.armazenamento_disponivel() {
        let entry = QueryHistoryEntry {
            id: crate::application::connection_manager::novo_id("hist"),
            connection_profile_id: Some(profile_id.clone()),
            query_text: trimmed.clone(),
            status: if result.is_ok() {
                "ok".to_string()
            } else {
                "erro".to_string()
            },
            duration_ms: Some(inicio.elapsed().as_millis() as i64),
            rows_returned,
            error_code,
            error_message,
            executed_at: agora_iso8601(),
            execution_mode: "manual".to_string(),
        };
        let _ = deps.repos().add_history(&entry);
    }

    result
}

#[tauri::command]
pub fn script_save(
    id: Option<String>,
    name: String,
    content: String,
    profile_id: Option<String>,
    deps: tauri::State<'_, AppState>,
) -> Result<SavedScriptDto, CommandError> {
    deps.exigir_armazenamento()?;
    let agora = agora_iso8601();
    let existente = match &id {
        Some(i) if !i.trim().is_empty() => deps.repos().get_script(i).map_err(erro_storage)?,
        _ => None,
    };
    let script_id = match id.clone() {
        Some(i) if !i.trim().is_empty() => i,
        _ => crate::application::connection_manager::novo_id("script"),
    };
    let created_at = existente
        .map(|s| s.created_at)
        .unwrap_or_else(|| agora.clone());
    let script = SavedScript {
        id: script_id,
        name,
        content,
        connection_profile_id: profile_id,
        created_at,
        updated_at: agora,
    };
    deps.repos().save_script(&script).map_err(erro_storage)?;
    Ok(SavedScriptDto::from(script))
}

#[tauri::command]
pub fn script_list(deps: tauri::State<'_, AppState>) -> Result<Vec<SavedScriptDto>, CommandError> {
    deps.exigir_armazenamento()?;
    let scripts = deps.repos().scripts().map_err(erro_storage)?;
    Ok(scripts.into_iter().map(SavedScriptDto::from).collect())
}

#[tauri::command]
pub fn script_get(
    id: String,
    deps: tauri::State<'_, AppState>,
) -> Result<Option<SavedScriptDto>, CommandError> {
    deps.exigir_armazenamento()?;
    let script = deps.repos().get_script(&id).map_err(erro_storage)?;
    Ok(script.map(SavedScriptDto::from))
}

#[tauri::command]
pub fn script_delete(id: String, deps: tauri::State<'_, AppState>) -> Result<bool, CommandError> {
    deps.exigir_armazenamento()?;
    deps.repos().delete_script(&id).map_err(erro_storage)
}

#[tauri::command]
pub fn history_list(
    limit: Option<i64>,
    deps: tauri::State<'_, AppState>,
) -> Result<Vec<HistoryEntryDto>, CommandError> {
    deps.exigir_armazenamento()?;
    let limite = limit.unwrap_or(100).clamp(1, 1000) as usize;
    let entradas = deps.repos().history(limite).map_err(erro_storage)?;
    Ok(entradas.into_iter().map(HistoryEntryDto::from).collect())
}

#[tauri::command]
pub fn history_clear(deps: tauri::State<'_, AppState>) -> Result<usize, CommandError> {
    deps.exigir_armazenamento()?;
    deps.repos().clear_history().map_err(erro_storage)
}

/// Pede o cancelamento de uma consulta em execução.
///
/// `true` quando havia uma consulta viva com esse `queryId`; `false` quando ela
/// já terminou (a UI trata como "nada a cancelar", não como erro).
#[tauri::command]
pub fn query_cancel(
    query_id: String,
    deps: tauri::State<'_, AppState>,
) -> Result<bool, CommandError> {
    Ok(deps.cancelar(&query_id))
}

/// Estado da transação da sessão (`true` quando há alterações pendentes).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionStatusDto {
    pub in_transaction: bool,
    pub message: String,
}

/// Inicia uma transação manual: os próximos DML ficam pendentes até
/// `transaction_commit` ou `transaction_rollback`.
#[tauri::command]
pub fn transaction_begin(
    profile_id: String,
    deps: tauri::State<'_, AppState>,
) -> Result<TransactionStatusDto, CommandError> {
    let manager: &ConnectionManager = deps.manager();
    manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| {
            adapter
                .begin_transaction()
                .map_err(|e| ErroGerente::Conexao(e.to_string()))
        })
        .map_err(CommandError::consulta)?;
    Ok(TransactionStatusDto {
        in_transaction: true,
        message: "transação iniciada: as alterações ficam pendentes até Commit ou Rollback"
            .to_string(),
    })
}

/// Confirma a transação pendente.
#[tauri::command]
pub fn transaction_commit(
    profile_id: String,
    deps: tauri::State<'_, AppState>,
) -> Result<TransactionStatusDto, CommandError> {
    let manager: &ConnectionManager = deps.manager();
    let estava_ativa = manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| Ok(adapter.in_transaction()))
        .map_err(CommandError::consulta)?;
    manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| {
            adapter
                .commit()
                .map_err(|e| ErroGerente::Conexao(e.to_string()))
        })
        .map_err(CommandError::consulta)?;
    Ok(TransactionStatusDto {
        in_transaction: false,
        message: if estava_ativa {
            "transação confirmada (Commit)".to_string()
        } else {
            "não havia transação pendente".to_string()
        },
    })
}

/// Desfaz a transação pendente.
#[tauri::command]
pub fn transaction_rollback(
    profile_id: String,
    deps: tauri::State<'_, AppState>,
) -> Result<TransactionStatusDto, CommandError> {
    let manager: &ConnectionManager = deps.manager();
    let estava_ativa = manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| Ok(adapter.in_transaction()))
        .map_err(CommandError::consulta)?;
    manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| {
            adapter
                .rollback()
                .map_err(|e| ErroGerente::Conexao(e.to_string()))
        })
        .map_err(CommandError::consulta)?;
    Ok(TransactionStatusDto {
        in_transaction: false,
        message: if estava_ativa {
            "transação desfeita (Rollback)".to_string()
        } else {
            "não havia transação pendente".to_string()
        },
    })
}

/// Chave primária de uma relação (colunas na ordem do índice).
///
/// Vazio quando a tabela não tem PK — a UI usa isso para decidir se a grade
/// pode ser editada com segurança.
#[tauri::command]
pub fn table_keys(
    profile_id: String,
    relation: String,
    deps: tauri::State<'_, AppState>,
) -> Result<Vec<String>, CommandError> {
    let manager: &ConnectionManager = deps.manager();
    manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| {
            use firebird_adapter::MetadataRepository;
            let repo = MetadataRepository::new();
            repo.chave_primaria(&mut **adapter, &relation)
                .map_err(|e| ErroGerente::Conexao(e.to_string()))
        })
        .map_err(|e| CommandError::consulta(e.to_string()))
}

/// Colunas da relação com tamanho declarado, marcando as que estouram o
/// limite de buffer do Firebird (erro `-204`).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColunaTabelaDto {
    pub name: String,
    pub exceeds_buffer: bool,
}

/// Lista as colunas de uma relação e sinaliza quais não devem entrar num
/// `SELECT` quando o erro `-204` (block size) aparecer.
#[tauri::command]
pub fn table_columns(
    profile_id: String,
    relation: String,
    deps: tauri::State<'_, AppState>,
) -> Result<Vec<ColunaTabelaDto>, CommandError> {
    let manager: &ConnectionManager = deps.manager();
    manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| {
            use firebird_adapter::MetadataRepository;
            let repo = MetadataRepository::new();
            repo.colunas(&mut **adapter, &relation)
                .map_err(|e| ErroGerente::Conexao(e.to_string()))
        })
        .map(|colunas| {
            colunas
                .into_iter()
                .map(|c| ColunaTabelaDto {
                    exceeds_buffer: c.excede_buffer_seguro(),
                    name: c.name,
                })
                .collect()
        })
        .map_err(CommandError::consulta)
}
