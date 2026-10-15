//! Teste de integração das consultas de catálogo (Passo 1, Fase 2).
//!
//! Roda contra um servidor real quando as env vars `FB_POC_*` estão definidas;
//! sem elas, o teste é ignorado. Não cria nem derruba objetos (não toca em
//! `CLIENTES`/`V_CLIENTES`) — apenas exercita as 7 categorias do catálogo e
//! imprime contagens para o relatório de compatibilidade.

use firebird_adapter::{ConnParams, FirebirdAdapter, MetadataRepository, RsFbClientAdapter};
// use firebird_domain::ServerVersion;
use std::path::PathBuf;

const ENV_OBRIGATORIAS: [&str; 6] = [
    "FB_POC_HOST",
    "FB_POC_PORT",
    "FB_POC_DATABASE",
    "FB_POC_USER",
    "FB_POC_PASSWORD",
    "FB_POC_LIB_PATH",
];

fn ambiente() -> Option<(String, u16, String, String, String, String)> {
    let mut vals = Vec::new();
    for nome in ENV_OBRIGATORIAS {
        match std::env::var(nome) {
            Ok(v) if !v.trim().is_empty() => vals.push(v),
            _ => {
                eprintln!("=== CATALOGO IGNORADO (sem env): {nome} ausente ===");
                return None;
            }
        }
    }
    let mut it = vals.into_iter();
    let host = it.next().unwrap();
    let port: u16 = it.next().unwrap().trim().parse().ok()?;
    let database = it.next().unwrap();
    let user = it.next().unwrap();
    let password = it.next().unwrap();
    let lib_path = it.next().unwrap();
    Some((host, port, database, user, password, lib_path))
}

#[test]
fn catalogo_sete_categorias() {
    let Some((host, port, database, user, password, lib_path)) = ambiente() else {
        return;
    };
    let params = ConnParams {
        host,
        port,
        database,
        user,
        password,
        charset: "UTF8".to_string(),
        client_library_path: PathBuf::from(lib_path),
    };
    let mut adapter = RsFbClientAdapter::connect(&params).expect("conexao");
    let versao = adapter.server_version().expect("versao");
    eprintln!("versao: {}", versao.major);
    let repo = MetadataRepository::new();

    let info = repo
        .informacoes_do_banco(&mut adapter, &versao)
        .expect("info banco");
    eprintln!("info_banco: {:?}", info.page_size);

    let rels = repo
        .relations_com_filtro(&mut adapter, &versao, None, false)
        .expect("rels");
    eprintln!("relacoes: {}", rels.len());

    let indices = repo.indices(&mut adapter, &versao).expect("indices");
    eprintln!("indices: {}", indices.len());

    let constraints = repo
        .constraints(&mut adapter, &versao)
        .expect("constraints");
    eprintln!("constraints: {}", constraints.len());

    let dominios = repo.dominios(&mut adapter, &versao).expect("dominios");
    eprintln!("dominios: {}", dominios.len());

    let gens = repo.generators(&mut adapter, &versao).expect("generators");
    eprintln!("generators: {}", gens.len());

    let procs = repo.procedures(&mut adapter, &versao).expect("procedures");
    eprintln!("procedures: {}", procs.len());

    let triggers = repo.triggers(&mut adapter, &versao).expect("triggers");
    eprintln!("triggers: {}", triggers.len());

    // sanity: pelo menos uma relacao de usuario nao sistema esperada? apenas nao falhar
    let _ = (rels, indices, constraints, dominios, gens, procs, triggers);
}
