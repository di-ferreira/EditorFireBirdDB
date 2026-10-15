//! Backend desktop do Firebird Studio Linux (Tauri 2).
//!
//! Camada de interface com o frontend: expõe comandos tipados via
//! `invoke_handler`. O acesso a Firebird, arquivos e segredos passa apenas
//! pelos módulos internos (`firebird`, `storage`, `security`) — nunca
//! diretamente pela UI. As permissões da webview ficam em
//! `capabilities/default.json` (apenas `core:default`).

use firebird_domain::ServerVersion;

/// Resposta do comando [`app_health`], serializada para a UI.
#[derive(Debug, serde::Serialize)]
pub struct AppHealth {
    /// Sempre `"ok"` enquanto o processo do app estiver vivo.
    pub status: String,
    /// Versão do domínio (`firebird-domain`) no formato `major.minor.patch`.
    pub domain_version: String,
}

/// Saúde do aplicativo: prova de ponta a ponta UI → Tauri → domínio.
///
/// A versão é derivada de `ServerVersion::parse(env!("CARGO_PKG_VERSION"))`.
/// **Escolha documentada:** `env!` é avaliado neste crate
/// (`firebird-studio-desktop`), cujo `version` no `Cargo.toml` do workspace é
/// mantido em sincronia com `firebird-domain` (ambos `0.1.0`) — a Fase 0 não
/// altera crates de domínio para expor uma constante de versão, por aderência
/// ao escopo de arquivos da tarefa. O parse via `ServerVersion` (e não o texto
/// cru) garante que só uma versão semântica válida chegue à UI.
///
/// A função é privada de propósito: `#[tauri::command]` em um `pub fn`
/// declarado na **raiz** do crate emite `#[macro_export]` para `__cmd__…`,
/// que colidiria com o `macro_rules!` local (E0255). Como o
/// `generate_handler!` está no mesmo módulo, a visibilidade privada basta.
#[tauri::command]
fn app_health() -> AppHealth {
    let versao = ServerVersion::parse(env!("CARGO_PKG_VERSION"))
        .expect("a versão do crate deve ser semântica (major.minor[.patch])");

    let domain_version = match versao.patch {
        Some(patch) => format!("{}.{}.{}", versao.major, versao.minor, patch),
        None => format!("{}.{}", versao.major, versao.minor),
    };

    AppHealth {
        status: "ok".to_string(),
        domain_version,
    }
}

/// Inicializa o Tauri: janela, capabilities e comandos registrados.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_health])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o Firebird Studio Linux");
}
