//! Comando de catálogo do DB Explorer (`catalog_get_node`).

use crate::application::catalogo::{CatalogNodeDto, CatalogNodeRequestDto, CatalogoService};
use crate::application::connection_manager::ConnectionManager;
use crate::application::estado::AppState;
use crate::commands::error::CommandError;

/// Serviço de catálogo compartilhado por todas as requisições.
static SERVICO: std::sync::LazyLock<CatalogoService> =
    std::sync::LazyLock::new(CatalogoService::novo);

/// Comando Tauri: expande um nó de catálogo e devolve **um nível** de filhos.
#[tauri::command]
pub fn catalog_get_node(
    profile_id: String,
    node: CatalogNodeRequestDto,
    filter: Option<String>,
    include_system: bool,
    force_refresh: bool,
    deps: tauri::State<'_, AppState>,
) -> Result<CatalogNodeDto, CommandError> {
    if !deps.armazenamento_disponivel() {
        return Err(CommandError::indisponivel(
            deps.avisos()
                .first()
                .cloned()
                .unwrap_or_else(|| "armazenamento indisponível".to_string()),
        ));
    }
    if !deps.cofre_disponivel() {
        // catálogo não exige cofre, mas conexão ativa sim; degradação orienta
        // o usuário. Não bloqueia — usa manager.
    }
    let manager: &ConnectionManager = deps.manager();
    SERVICO
        .get_node(
            manager,
            profile_id,
            node,
            filter,
            include_system,
            force_refresh,
        )
        .map_err(|e| CommandError::conexao(e.to_string()))
}
