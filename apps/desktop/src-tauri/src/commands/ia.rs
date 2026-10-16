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
