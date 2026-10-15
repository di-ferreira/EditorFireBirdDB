//! Backend desktop do Firebird Studio Linux (Tauri 2).
//!
//! Camada de interface com o frontend: expõe comandos tipados via
//! `invoke_handler`. O acesso a Firebird, arquivos e segredos passa apenas
//! pelos módulos internos (`firebird`, `storage`, `security`) — nunca
//! diretamente pela UI. As permissões da webview ficam em
//! `capabilities/default.json` (apenas `core:default`).

pub mod application;
pub mod commands;
pub mod security;
pub mod storage;

use application::estado::AppState;
use commands::{
    catalog_get_node, connection_close, connection_get_status, connection_list_profiles,
    connection_open, connection_save_profile, connection_test_profile,
};
use firebird_domain::VERSION as DOMAIN_VERSION;
use tauri::Manager;

/// Nomes dos comandos de conexão registrados no `invoke_handler`.
///
/// **Atualize esta lista junto com o [`tauri::generate_handler!`]**: ela
/// existe para que o teste de contrato com a UI
/// (`apps/desktop/src/features/connections/api.ts`) detecte comando
/// registrado com nome divergente do que o frontend invoca.
pub const NOMES_DOS_COMANDOS_DE_CONEXAO: [&str; 6] = [
    "connection_list_profiles",
    "connection_save_profile",
    "connection_test_profile",
    "connection_open",
    "connection_close",
    "connection_get_status",
];

/// Nomes dos comandos de catálogo registrados no `invoke_handler`.
pub const NOMES_DOS_COMANDOS_DE_CATALOGO: [&str; 1] = ["catalog_get_node"];

/// Resposta do comando [`app_health`], serializada para a UI.
#[derive(Debug, serde::Serialize)]
pub struct AppHealth {
    /// Sempre `"ok"` enquanto o processo do app estiver vivo.
    pub status: String,
    /// Versão real do crate de domínio (`firebird_domain::VERSION`).
    pub domain_version: String,
}

/// Saúde do aplicativo: prova de ponta a ponta UI → Tauri → domínio.
///
/// A versão exibida vem de [`firebird_domain::VERSION`] — a constante do
/// próprio crate de domínio —, e **não** de `env!("CARGO_PKG_VERSION")`, que
/// leria a versão do crate desktop e poderia divergir silenciosamente.
/// [`firebird_domain`] garante que a constante é semântica (teste em
/// `firebird_domain`).
///
/// A função é privada: o comando só precisa existir no módulo onde o
/// `generate_handler!` é montado.
#[tauri::command]
fn app_health() -> AppHealth {
    AppHealth {
        status: "ok".to_string(),
        domain_version: DOMAIN_VERSION.to_string(),
    }
}

/// Inicializa o Tauri: janela, capabilities, estado e comandos registrados.
///
/// O estado (armazenamento local + cofre + gerenciador de sessões) é montado
/// no `setup` e degrada com mensagem acionável quando algum deles não abre —
/// sem `panic!` no startup (Spec 03 §2).
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let caminho = app.path().app_data_dir().ok();
            let estado = AppState::montar(caminho.as_deref());
            for aviso in estado.avisos() {
                eprintln!("[firebird-studio] {aviso}");
            }
            app.manage(estado);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            catalog_get_node,
            connection_list_profiles,
            connection_save_profile,
            connection_test_profile,
            connection_open,
            connection_close,
            connection_get_status
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o Firebird Studio Linux");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_health_reporta_ok_e_a_versao_real_do_crate_de_dominio() {
        let health = app_health();

        assert_eq!(health.status, "ok");
        assert_eq!(
            health.domain_version, DOMAIN_VERSION,
            "domain_version deve ser a versão do crate firebird-domain"
        );
    }

    #[test]
    fn nenhuma_capability_concede_permissao_alem_de_core_default() {
        // Trava de segurança sobre **todos** os arquivos de capabilities: um
        // `capabilities/extra.json` com outras permissões também reprovaria.
        // O JSON é *parseado* (não comparado por substring), então uma
        // reformatação legítima do arquivo não quebra o teste e nenhuma
        // permissão extra passa despercebida. (Regra: Spec 11 §1.)
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
        let arquivos: Vec<_> = std::fs::read_dir(&dir)
            .expect("diretório capabilities legível")
            .map(|entrada| entrada.expect("entrada legível em capabilities").path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();

        assert!(
            !arquivos.is_empty(),
            "esperado ao menos um arquivo de capability em {}",
            dir.display()
        );

        for arquivo in arquivos {
            let nome = arquivo.display().to_string();
            let texto = std::fs::read_to_string(&arquivo)
                .unwrap_or_else(|e| panic!("falha ao ler {nome}: {e}"));
            let valor: serde_json::Value = serde_json::from_str(&texto)
                .unwrap_or_else(|e| panic!("{nome}: capability precisa ser JSON válido: {e}"));

            let permisos = valor["permissions"]
                .as_array()
                .unwrap_or_else(|| panic!("{nome}: campo `permissions` ausente ou não é lista"));

            for permissao in permisos {
                assert_eq!(
                    permissao.as_str(),
                    Some("core:default"),
                    "{nome}: permissão não permitida {permissao} (Spec 11 §1)"
                );
            }
        }
    }
}
