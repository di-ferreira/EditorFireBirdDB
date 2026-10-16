//! Fronteira de comandos do assistente de IA — Passo 1 da Fase 3 (Spec 07/10).
//!
//! Nenhum DTO aqui carrega chave de API: o campo de escrita é usado só para
//! gravar no cofre e some antes da resposta.

use crate::application::estado::AppState;
use crate::application::ia::TransporteReqwest;
use crate::application::ia_service::{IaService, ProvedorEntrada};
use crate::commands::error::CommandError;

/// Perfil de provedor devolvido à UI (sem segredo).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvedorIaDto {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub model_name: String,
    /// `true` quando existe chave guardada no cofre (nunca a chave em si).
    pub has_api_key: bool,
    pub timeout_ms: i64,
    pub remote_context_confirmation: bool,
}

fn para_dto(p: firebird_storage::AiProviderProfile) -> ProvedorIaDto {
    ProvedorIaDto {
        id: p.id,
        name: p.name,
        provider_type: p.provider_type,
        base_url: p.base_url,
        model_name: p.model_name,
        has_api_key: p.secret_ref.is_some(),
        timeout_ms: p.timeout_ms,
        remote_context_confirmation: p.remote_context_confirmation,
    }
}

/// Monta o serviço com o transporte HTTP real (falha só se o cliente não subir).
fn servico(deps: &AppState) -> Result<IaService, CommandError> {
    let transporte =
        std::sync::Arc::new(TransporteReqwest::new().map_err(CommandError::desconhecido)?);
    Ok(IaService::new(
        deps.repos().clone(),
        deps.secrets().clone(),
        transporte,
    ))
}

/// Lista os provedores de IA configurados.
#[tauri::command]
pub fn ia_provider_list(
    deps: tauri::State<'_, AppState>,
) -> Result<Vec<ProvedorIaDto>, CommandError> {
    deps.exigir_armazenamento()?;
    let lista = servico(&deps)?
        .listar()
        .map_err(|e| CommandError::provedor(e.to_string()))?;
    Ok(lista.into_iter().map(para_dto).collect())
}

/// Cria ou atualiza um provedor. `api_key`, quando informada, vai para o cofre.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn ia_provider_save(
    id: Option<String>,
    name: String,
    provider_type: String,
    base_url: String,
    model_name: String,
    api_key: Option<String>,
    timeout_ms: Option<i64>,
    remote_context_confirmation: Option<bool>,
    deps: tauri::State<'_, AppState>,
) -> Result<ProvedorIaDto, CommandError> {
    deps.exigir_armazenamento()?;
    deps.exigir_cofre()?;
    let perfil = servico(&deps)?
        .salvar(ProvedorEntrada {
            id,
            nome: name,
            tipo: provider_type,
            base_url,
            modelo: model_name,
            api_key,
            timeout_ms: timeout_ms.unwrap_or(30_000),
            consentir_contexto_remoto: remote_context_confirmation.unwrap_or(false),
        })
        .map_err(|e| CommandError::provedor(e.to_string()))?;
    Ok(para_dto(perfil))
}

/// Testa a configuração do provedor (sem enviar contexto do banco).
#[tauri::command]
pub fn ia_provider_test(
    id: String,
    deps: tauri::State<'_, AppState>,
) -> Result<String, CommandError> {
    deps.exigir_armazenamento()?;
    servico(&deps)?
        .testar(&id)
        .map_err(|e| CommandError::provedor(e.to_string()))
}

/// Remove o provedor e a chave correspondente do cofre.
#[tauri::command]
pub fn ia_provider_delete(
    id: String,
    deps: tauri::State<'_, AppState>,
) -> Result<bool, CommandError> {
    deps.exigir_armazenamento()?;
    servico(&deps)?
        .apagar(&id)
        .map_err(|e| CommandError::provedor(e.to_string()))
}

/// Pré-visualização do contexto que **seria** enviado ao provedor.
///
/// Nada sai da máquina neste comando: ele só mostra o texto, para o usuário
/// decidir (consentimento do Passo 4).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IaContextPreviewDto {
    pub text: String,
    pub relacoes: usize,
    pub relacoes_omitidas: usize,
    pub truncado: bool,
}

/// Monta o contexto do schema de um perfil conectado.
#[tauri::command]
pub fn ia_context_preview(
    profile_id: String,
    deps: tauri::State<'_, AppState>,
) -> Result<IaContextPreviewDto, CommandError> {
    let manager: &crate::application::connection_manager::ConnectionManager = deps.manager();
    let contexto = manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| {
            use crate::application::ia_context::ContextoBuilder;
            let versao = adapter.server_version().map_err(|e| {
                crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
            })?;
            ContextoBuilder::default()
                .construir(&mut **adapter, &versao)
                .map_err(|e| {
                    crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
                })
        })
        .map_err(|e| CommandError::provedor(e.to_string()))?;

    Ok(IaContextPreviewDto {
        text: contexto.texto,
        relacoes: contexto.relacoes,
        relacoes_omitidas: contexto.relacoes_omitidas,
        truncado: contexto.truncado,
    })
}

/// Um template de prompt disponível, com a versão vigente.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IaTemplateDto {
    pub tarefa: String,
    pub rotulo: String,
    pub versao: u32,
    pub instrucao: String,
}

/// Lista os templates versionados de prompt (geração, explicação, correção,
/// revisão) — nenhum envia contexto, apenas descreve o prompt.
#[tauri::command]
pub fn ia_template_list() -> Result<Vec<IaTemplateDto>, CommandError> {
    use crate::application::ia_context::templates;
    Ok(templates()
        .iter()
        .map(|t| IaTemplateDto {
            tarefa: t.tarefa.as_str().to_string(),
            rotulo: match t.tarefa {
                firebird_domain::TarefaIa::GerarSql => "Gerar SQL",
                firebird_domain::TarefaIa::ExplicarSql => "Explicar SQL",
                firebird_domain::TarefaIa::CorrigirSql => "Corrigir SQL",
                firebird_domain::TarefaIa::RevisarSql => "Revisar SQL",
            }
            .to_string(),
            versao: t.versao,
            instrucao: t.instrucao.to_string(),
        })
        .collect())
}

/// Resposta do assistente, pronta para a UI.
///
/// `sql` é **sugestão**: a UI só pode inseri-lo no editor, nunca executá-lo
/// sozinha (Spec 07 §revisão).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IaGenerateDto {
    pub texto: String,
    pub sql: Option<String>,
    pub modelo: String,
    pub interaction_id: String,
    pub contexto_relacoes: usize,
    pub contexto_truncado: bool,
    /// `true` quando o contexto foi enviado a um provedor remoto autorizado.
    pub contexto_remoto: bool,
}

/// Executa uma tarefa de IA com o contexto do schema do perfil.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn ia_generate(
    profile_id: String,
    provider_profile_id: String,
    task: String,
    prompt: String,
    deps: tauri::State<'_, AppState>,
) -> Result<IaGenerateDto, CommandError> {
    deps.exigir_armazenamento()?;
    let tarefa = match task.as_str() {
        "gerar_sql" => firebird_domain::TarefaIa::GerarSql,
        "explicar_sql" => firebird_domain::TarefaIa::ExplicarSql,
        "corrigir_sql" => firebird_domain::TarefaIa::CorrigirSql,
        "revisar_sql" => firebird_domain::TarefaIa::RevisarSql,
        outra => {
            return Err(CommandError::consulta(format!(
                "tarefa de IA desconhecida: {outra}"
            )))
        }
    };

    let manager: &crate::application::connection_manager::ConnectionManager = deps.manager();
    let contexto = manager
        .executar_no_adaptador_conectado(&profile_id, |adapter| {
            use crate::application::ia_context::ContextoBuilder;
            let versao = adapter.server_version().map_err(|e| {
                crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
            })?;
            ContextoBuilder::default()
                .construir(&mut **adapter, &versao)
                .map_err(|e| {
                    crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
                })
        })
        .map_err(|e| CommandError::provedor(e.to_string()))?;

    // Consentimento remoto é conferido antes de qualquer envio.
    let remoto = servico(&deps)?
        .listar()
        .map_err(|e| CommandError::provedor(e.to_string()))?
        .into_iter()
        .find(|p| p.id == provider_profile_id)
        .map(|p| p.remote_context_confirmation)
        .unwrap_or(false);

    let resultado = servico(&deps)?
        .gerar(&provider_profile_id, tarefa, &prompt, &contexto)
        .map_err(|e| CommandError::provedor(e.to_string()))?;

    Ok(IaGenerateDto {
        texto: resultado.resposta.texto,
        sql: resultado.resposta.sql,
        modelo: resultado.resposta.modelo,
        interaction_id: resultado.interacao_id,
        contexto_relacoes: contexto.relacoes,
        contexto_truncado: contexto.truncado,
        contexto_remoto: remoto,
    })
}

/// Uma interação do histórico (texto já redigido).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IaInteractionDto {
    pub id: String,
    pub provider_profile_id: Option<String>,
    pub task_type: String,
    pub prompt_text: String,
    pub response_text: String,
    pub accepted_sql: Option<String>,
    pub created_at: String,
}

/// Lista o histórico de interações com a IA.
#[tauri::command]
pub fn ia_interactions_list(
    limit: Option<i64>,
    deps: tauri::State<'_, AppState>,
) -> Result<Vec<IaInteractionDto>, CommandError> {
    deps.exigir_armazenamento()?;
    let limite = limit.unwrap_or(20).clamp(1, 200) as usize;
    Ok(servico(&deps)?
        .historico(limite)
        .map_err(|e| CommandError::provedor(e.to_string()))?
        .into_iter()
        .map(|i| IaInteractionDto {
            id: i.id,
            provider_profile_id: i.provider_profile_id,
            task_type: i.task_type,
            prompt_text: i.prompt_text,
            response_text: i.response_text,
            accepted_sql: i.accepted_sql,
            created_at: i.created_at,
        })
        .collect())
}

/// Marca o SQL sugerido como aceito pelo usuário (auditoria).
#[tauri::command]
pub fn ia_interaction_accept_sql(
    interaction_id: String,
    sql: String,
    deps: tauri::State<'_, AppState>,
) -> Result<bool, CommandError> {
    deps.exigir_armazenamento()?;
    servico(&deps)?
        .marcar_sql_aceito(&interaction_id, &sql)
        .map_err(|e| CommandError::provedor(e.to_string()))?;
    Ok(true)
}
