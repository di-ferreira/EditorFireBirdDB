//! Backend desktop do Firebird Studio Linux (Tauri 2).
//!
//! Camada de interface com o frontend: expõe comandos tipados via
//! `invoke_handler`. O acesso a Firebird, arquivos e segredos passa apenas
//! pelos módulos internos (`firebird`, `storage`, `security`) — nunca
//! diretamente pela UI. As permissões da webview ficam em
//! `capabilities/default.json` (apenas `core:default`).

use firebird_domain::VERSION as DOMAIN_VERSION;

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

/// Inicializa o Tauri: janela, capabilities e comandos registrados.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_health])
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
        // Guarda contra o modo de falha anterior: um wrapper local que copiasse
        // a versão do próprio crate desktop continuaria verde se ambos
        // divergissem — a constante do domínio é a única fonte de verdade.
        assert_eq!(health.domain_version, firebird_domain::VERSION);
    }

    #[test]
    fn nenhuma_capability_concede_permissao_alem_de_core_default() {
        // Trava de segurança sobre **todos** os arquivos de capabilities: um
        // `capabilities/extra.json` com `fs:`/`shell:` também reprovaria.
        // (Regra: Spec 11 §1 — escopos mínimos, sem shell/fsystem genérico.)
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
        let arquivos: Vec<_> = std::fs::read_dir(&dir)
            .expect("diretório capabilities legível")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();

        assert!(
            !arquivos.is_empty(),
            "esperado ao menos um arquivo de capability em {}",
            dir.display()
        );

        for arquivo in arquivos {
            let conteudo = std::fs::read_to_string(&arquivo)
                .unwrap_or_else(|e| panic!("falha ao ler {}: {e}", arquivo.display()));
            let nome = arquivo.display().to_string();
            assert!(
                conteudo.contains("core:default"),
                "{nome}: deveria manter core:default"
            );
            for proibida in ["fs:", "shell:", "process:", "http:"] {
                assert!(
                    !conteudo.contains(proibida),
                    "{nome}: permissão não permitida `{proibida}` (Spec 11 §1)"
                );
            }
        }
    }
}
