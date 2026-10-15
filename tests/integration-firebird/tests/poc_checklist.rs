//! Checklist de integração por versão do Firebird (Fase 0 — PoC, Task 4).
//!
//! Um único teste executa o checklist completo contra **um** ambiente real,
//! descrito pelas env vars `FB_POC_HOST`, `FB_POC_PORT`, `FB_POC_DATABASE`,
//! `FB_POC_USER`, `FB_POC_PASSWORD` e `FB_POC_LIB_PATH` (ambiente de
//! `scripts/firebird-env.sh up <tag>` ou servidor local). Sem elas o teste é
//! **ignorado** com aviso claro e nunca falha o `cargo test` local.
//!
//! O checklist (por ambiente, binding do controller):
//!  1. connect + `server_version`;
//!  2. `relations()` contém a tabela sintética CLIENTES e a view V_CLIENTES;
//!  3. `relation_fields("CLIENTES")` com tipos/nullable/domain corretos;
//!  4. SELECT parametrizado com `?` devolve a linha certa;
//!  5. Unicode, decimal e NULL corretos no round-trip;
//!  6. SQL inválido → [`DomainError`] sem senha na mensagem.
//!
//! Cada item acumula falha própria (com texto real do erro) e o teste falha ao
//! final listando **todas** — nada é maquiado (Spec 12, §2). O schema sintético
//! é criado no início e derrubado no final (melhor esforço: drop que falha não
//! derruba o teste). Nenhum segredo real em fixtures (FR-016): a única
//! credencial é a do container descartável de teste.

use std::path::PathBuf;

use firebird_adapter::{ConnParams, FirebirdAdapter, MetadataRepository, RsFbClientAdapter};
use firebird_domain::{DomainError, RelationKind, ServerVersion};

/// Env vars obrigatórias para rodar o checklist (todas devem estar presentes
/// e não vazias; caso contrário, o teste é ignorado).
const ENV_OBRIGATORIAS: [&str; 6] = [
    "FB_POC_HOST",
    "FB_POC_PORT",
    "FB_POC_DATABASE",
    "FB_POC_USER",
    "FB_POC_PASSWORD",
    "FB_POC_LIB_PATH",
];

/// Ambiente de teste real descrito pelas env vars `FB_POC_*`.
struct Ambiente {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
    lib_path: String,
}

impl Ambiente {
    /// Parâmetros de conexão (charset UTF8 — banco criado com UTF8 pelo
    /// harness; a senha nunca sai deste struct para log).
    fn conn_params(&self) -> ConnParams {
        ConnParams {
            host: self.host.clone(),
            port: self.port,
            database: self.database.clone(),
            user: self.user.clone(),
            password: self.password.clone(),
            charset: "UTF8".to_string(),
            client_library_path: PathBuf::from(&self.lib_path),
        }
    }

    /// Identificação curta para mensagens (sem credenciais).
    fn rotulo(&self) -> String {
        format!(
            "{}:{}/{} (lib {})",
            self.host, self.port, self.database, self.lib_path
        )
    }
}

/// Lê o ambiente das env vars. Se qualquer uma faltar (ou a porta for
/// inválida), avisa claro e devolve [`None`] — o teste é ignorado, nunca
/// falha, sem env (regra do harness).
fn ambiente_de_env() -> Option<Ambiente> {
    let mut valores = Vec::with_capacity(ENV_OBRIGATORIAS.len());
    for nome in ENV_OBRIGATORIAS {
        match std::env::var(nome) {
            Ok(valor) if !valor.trim().is_empty() => valores.push(valor),
            _ => {
                eprintln!(
                    "=== CHECKLIST DE INTEGRAÇÃO IGNORADO (sem env): {nome} ausente/vazia. ==="
                );
                eprintln!("=== Defina FB_POC_HOST, FB_POC_PORT, FB_POC_DATABASE, FB_POC_USER, ===");
                eprintln!(
                    "=== FB_POC_PASSWORD e FB_POC_LIB_PATH para rodar contra um servidor ==="
                );
                eprintln!(
                    "=== real (ex.: ./scripts/firebird-env.sh up 3.0 e use o export exibido). ==="
                );
                return None;
            }
        }
    }
    let mut it = valores.into_iter();
    let host = it.next().expect("6 env vars lidas");
    let port_raw = it.next().expect("6 env vars lidas");
    let database = it.next().expect("6 env vars lidas");
    let user = it.next().expect("6 env vars lidas");
    let password = it.next().expect("6 env vars lidas");
    let lib_path = it.next().expect("6 env vars lidas");

    let Ok(port) = port_raw.trim().parse::<u16>() else {
        eprintln!("=== CHECKLIST DE INTEGRAÇÃO IGNORADO (sem env): FB_POC_PORT='{port_raw}' não é porta válida. ===");
        return None;
    };

    Some(Ambiente {
        host,
        port,
        database,
        user,
        password,
        lib_path,
    })
}

#[test]
fn checklist_poc_contra_servidor_real() {
    let Some(amb) = ambiente_de_env() else {
        return;
    };
    eprintln!("=== checklist de integração contra {} ===", amb.rotulo());

    let falhas = executar_checklist(&amb);

    assert!(
        falhas.is_empty(),
        "{} item(ns) do checklist falharam contra {}:\n{}",
        falhas.len(),
        amb.rotulo(),
        falhas.join("\n")
    );
}

/// Executa o checklist completo, acumulando falhas por item (texto real do
/// erro, sem maquiagem). Cada item imprime PASS/FAIL via `eprintln!`.
fn executar_checklist(amb: &Ambiente) -> Vec<String> {
    let mut falhas: Vec<String> = Vec::new();
    let params = amb.conn_params();

    // ---------- Item 1: connect + server_version ----------
    let mut adapter = match RsFbClientAdapter::connect(&params) {
        Ok(adapter) => adapter,
        Err(erro) => {
            falhas.push(format!("item 1: conexão falhou: {erro:?}"));
            return falhas;
        }
    };
    eprintln!("item 1: conectado em {}", amb.rotulo());

    let versao = match adapter.server_version() {
        Ok(versao) => {
            eprintln!(
                "item 1 [PASS]: server_version = {}",
                formatar_versao(&versao)
            );
            Some(versao)
        }
        Err(erro) => {
            falhas.push(format!("item 1: server_version falhou: {erro:?}"));
            None
        }
    };

    // ---------- Setup: schema sintético (pré-checagem antes de qualquer DDL) ----------
    if !criar_schema_sintetico(&mut adapter, versao.as_ref(), &mut falhas) {
        return falhas;
    }

    // ---------- Item 2: relations() contém CLIENTES e V_CLIENTES ----------
    match versao {
        Some(versao) => {
            let repo = MetadataRepository::new();
            match repo.relations(&mut adapter, &versao) {
                Ok(relacoes) => {
                    let tem_tabela = relacoes
                        .iter()
                        .any(|r| r.name == "CLIENTES" && r.kind == RelationKind::Table);
                    let tem_view = relacoes
                        .iter()
                        .any(|r| r.name == "V_CLIENTES" && r.kind == RelationKind::View);
                    if tem_tabela && tem_view {
                        eprintln!(
                            "item 2 [PASS]: relations() lista CLIENTES (Table) e V_CLIENTES (View) entre {} relações",
                            relacoes.len()
                        );
                    } else {
                        falhas.push(format!(
                            "item 2: relations() não classificou como esperado (tem_tabela={tem_tabela}, tem_view={tem_view}); relações: {:?}",
                            relacoes.iter().map(|r| format!("{}:{:?}", r.name, r.kind)).collect::<Vec<_>>()
                        ));
                    }
                }
                Err(erro) => falhas.push(format!("item 2: relations() falhou: {erro:?}")),
            }
        }
        None => falhas.push("item 2: pulado — server_version indisponível (item 1)".to_string()),
    }

    // ---------- Item 3: relation_fields("CLIENTES") ----------
    {
        let repo = MetadataRepository::new();
        match repo.relation_fields(&mut adapter, "CLIENTES") {
            Ok(campos) => validar_campos_clientes(&campos, &mut falhas),
            Err(erro) => falhas.push(format!("item 3: relation_fields falhou: {erro:?}")),
        }
    }

    // ---------- Item 4: SELECT parametrizado com ? ----------
    match adapter.query_rows_params("SELECT NOME FROM CLIENTES WHERE ID = ?", &[Some("2")]) {
        Ok(linhas) if linhas == vec![vec![Some("O'BRIEN".to_string())]] => {
            eprintln!("item 4 [PASS]: SELECT com ? devolveu a linha certa (O'BRIEN)");
        }
        Ok(linhas) => falhas.push(format!(
            "item 4: SELECT com ? devolveu linha errada: {linhas:?}"
        )),
        Err(erro) => falhas.push(format!("item 4: SELECT parametrizado falhou: {erro:?}")),
    }

    // ---------- Item 5: Unicode / decimal / NULL ----------
    match adapter.query_rows("SELECT NOME, VALOR, OBS, FLAG FROM CLIENTES ORDER BY ID") {
        Ok(linhas) => validar_dados_roundtrip(&linhas, &mut falhas),
        Err(erro) => falhas.push(format!("item 5: SELECT de dados falhou: {erro:?}")),
    }

    // ---------- Item 6: SQL inválido → erro sem senha ----------
    match adapter.query_rows("SELECT COLUNA_INEXISTENTE_POOC FROM TABELA_INEXISTENTE_POOC") {
        Ok(linhas) => falhas.push(format!(
            "item 6: SQL inválido não gerou erro (devolveu {linhas:?})"
        )),
        Err(erro) => {
            let eh_driver = matches!(erro, DomainError::Driver { .. });
            let debug = format!("{erro:?}"); // Debug já sai redigido (FR-016)
            let redigido = erro.redacted(); // consome o erro — por isso o variant checado antes
            if debug.contains(&amb.password) || redigido.contains(&amb.password) {
                falhas.push(
                    "item 6: a senha do ambiente apareceu na mensagem de erro (viola FR-016)"
                        .to_string(),
                );
            } else if eh_driver {
                eprintln!(
                    "item 6 [PASS]: SQL inválido virou DomainError::Driver sem senha: {debug}"
                );
            } else {
                falhas.push(format!(
                    "item 6: erro veio de variante inesperada (esperado Driver): {debug}"
                ));
            }
        }
    }

    // ---------- Teardown: melhor esforço, em conexão nova ----------
    // A conexão do teste é derrubada ANTES: no Firebird 2.5 (SuperClassic) os
    // statements em cache que referenciam CLIENTES seguram lock de metadata e
    // o DROP de outra conexão fica em espera infinita (evidência da Task 4).
    drop(adapter);
    derrubar_schema_melhor_esforco(amb);

    falhas
}

/// Cria o schema sintético (tabela CLIENTES + view V_CLIENTES) e insere as
/// duas linhas de dados. DDL/DML usam `execute` (sem fetch) — caminho
/// obrigatório enquanto o bug de classificação do rsfbclient 0.29 nativo
/// existir (ver [`firebird_adapter::FirebirdAdapter::execute`]). Retorna
/// `false` quando o setup falhou (checklist não pode continuar sem a tabela).
///
/// **Pré-checagem obrigatória:** se `CLIENTES`/`V_CLIENTES` já existirem no
/// banco de destino, o teste **recusa** criar ou derrubar — este harness pode
/// ser apontado para um banco que não é descartável (o plano da Task 4 previa
/// uso do servidor local), e um `DROP` cego destruiria dados alheios. A regra
/// mora em [`integration_firebird::pre_checagem`], coberta por testes unitários
/// (sem servidor) — aqui só consumimos o veredito.
fn criar_schema_sintetico(
    adapter: &mut RsFbClientAdapter,
    versao: Option<&ServerVersion>,
    falhas: &mut Vec<String>,
) -> bool {
    match integration_firebird::pre_checagem(adapter, versao) {
        Ok(Some(recusa)) => {
            falhas.push(format!("setup: {recusa}"));
            return false;
        }
        Ok(None) => {}
        Err(erro) => {
            falhas.push(format!("setup: pré-checagem de schema falhou: {erro:?}"));
            return false;
        }
    }

    let ddl_tabela = "CREATE TABLE CLIENTES (\
        ID INTEGER NOT NULL PRIMARY KEY, \
        NOME VARCHAR(60) NOT NULL, \
        VALOR NUMERIC(15,2), \
        OBS BLOB SUB_TYPE TEXT, \
        FLAG INTEGER)";
    if let Err(erro) = adapter.execute(ddl_tabela, &[]) {
        falhas.push(format!("setup: CREATE TABLE CLIENTES falhou: {erro:?}"));
        return false;
    }
    if let Err(erro) = adapter.execute(
        "CREATE VIEW V_CLIENTES AS SELECT ID, NOME FROM CLIENTES",
        &[],
    ) {
        falhas.push(format!("setup: CREATE VIEW V_CLIENTES falhou: {erro:?}"));
        return false;
    }

    let insert = "INSERT INTO CLIENTES (ID, NOME, VALOR, OBS, FLAG) VALUES (?, ?, ?, ?, ?)";
    let linhas: [(&str, [Option<&str>; 5]); 2] = [
        (
            "1",
            [
                Some("1"),
                Some("João da Conceição"), // Unicode com acento
                Some("1234.56"),           // decimal positivo
                Some("obs sintética"),     // BLOB texto
                Some("1"),
            ],
        ),
        (
            "2",
            [
                Some("2"),
                Some("O'BRIEN"), // apóstrofo no dado
                Some("-987.65"), // decimal negativo
                None,            // BLOB NULL (≠ vazio)
                None,            // INTEGER NULL (≠ vazio)
            ],
        ),
    ];
    for (id, params) in linhas {
        if let Err(erro) = adapter.execute(insert, &params) {
            falhas.push(format!("setup: INSERT do registro {id} falhou: {erro:?}"));
            return false;
        }
    }
    eprintln!("setup [PASS]: schema sintético CLIENTES/V_CLIENTES criado com 2 linhas");
    true
}

/// Valida as colunas de CLIENTES (item 3): tipos, nullable e domain.
fn validar_campos_clientes(campos: &[firebird_domain::FieldInfo], falhas: &mut Vec<String>) {
    let antes = falhas.len();
    let campo = |nome: &str| campos.iter().find(|c| c.name == nome);

    let esperados: [(&str, &str, bool); 5] = [
        ("ID", "INTEGER", false),
        ("NOME", "VARCHAR", false),
        ("VALOR", "NUMERIC", true),
        ("OBS", "BLOB SUB_TYPE TEXT", true),
        ("FLAG", "INTEGER", true),
    ];
    for (nome, tipo, nullable) in esperados {
        match campo(nome) {
            None => falhas.push(format!("item 3: coluna {nome} ausente em relation_fields")),
            Some(c) => {
                if c.type_name != tipo {
                    falhas.push(format!(
                        "item 3: {nome} com type_name '{}' (esperado '{tipo}')",
                        c.type_name
                    ));
                }
                if c.nullable != nullable {
                    falhas.push(format!(
                        "item 3: {nome} com nullable={} (esperado {nullable})",
                        c.nullable
                    ));
                }
                if c.domain.is_none() {
                    falhas.push(format!(
                        "item 3: {nome} sem domain no catálogo (Firebird gera domínio interno R$ para coluna inline)"
                    ));
                }
            }
        }
    }
    if falhas.len() == antes {
        eprintln!("item 3 [PASS]: relation_fields('CLIENTES') com tipos/nullable/domain corretos");
    }
}

/// Valida o round-trip de Unicode, decimal e NULL (item 5).
fn validar_dados_roundtrip(linhas: &[Vec<Option<String>>], falhas: &mut Vec<String>) {
    if linhas.len() != 2 {
        falhas.push(format!(
            "item 5: esperadas 2 linhas, veio {}: {linhas:?}",
            linhas.len()
        ));
        return;
    }
    let antes = falhas.len();
    let linha1 = &linhas[0];
    let linha2 = &linhas[1];

    // Linha 1: Unicode, decimal positivo, BLOB texto e flag 1.
    comparar(
        linha1.first(),
        &Some("João da Conceição".to_string()),
        "linha1 NOME (Unicode)",
        falhas,
    );
    comparar(
        linha1.get(1),
        &Some("1234.56".to_string()),
        "linha1 VALOR (decimal)",
        falhas,
    );
    comparar(
        linha1.get(2),
        &Some("obs sintética".to_string()),
        "linha1 OBS (BLOB texto)",
        falhas,
    );
    comparar(linha1.get(3), &Some("1".to_string()), "linha1 FLAG", falhas);

    // Linha 2: apóstrofo, decimal negativo e NULLs (NULL ≠ vazio: precisa ser
    // None, nunca Some(")).
    comparar(
        linha2.first(),
        &Some("O'BRIEN".to_string()),
        "linha2 NOME (apóstrofo)",
        falhas,
    );
    comparar(
        linha2.get(1),
        &Some("-987.65".to_string()),
        "linha2 VALOR (decimal negativo)",
        falhas,
    );
    comparar(linha2.get(2), &None, "linha2 OBS (NULL ≠ vazio)", falhas);
    comparar(linha2.get(3), &None, "linha2 FLAG (NULL ≠ vazio)", falhas);

    if falhas.len() == antes {
        eprintln!("item 5 [PASS]: Unicode/decimal/NULL corretos no round-trip");
    }
}

/// Compara um campo obtido com o esperado, registrando falha com o valor real.
fn comparar(
    obtido: Option<&Option<String>>,
    esperado: &Option<String>,
    rotulo: &str,
    falhas: &mut Vec<String>,
) {
    match obtido {
        None => falhas.push(format!("item 5: {rotulo}: linha sem a coluna esperada")),
        Some(valor) if valor == esperado => {}
        Some(valor) => falhas.push(format!(
            "item 5: {rotulo}: obtido {valor:?}, esperado {esperado:?}"
        )),
    }
}

/// Teardown melhor esforço: derruba view e tabela; erro só é avisado, nunca
/// derruba o teste. Usa conexão **nova**: o cache de statements do rsfbclient
/// mantém statements alocados na conexão do teste e o Firebird 2.5/3.0 recusa
/// o DROP com "object ... is in use" (evidência da Task 4).
fn derrubar_schema_melhor_esforco(amb: &Ambiente) {
    let mut adapter = match RsFbClientAdapter::connect(&amb.conn_params()) {
        Ok(adapter) => adapter,
        Err(erro) => {
            eprintln!("teardown: reconexão falhou (melhor esforço): {erro:?}");
            return;
        }
    };
    if let Err(erro) = adapter.execute("DROP VIEW V_CLIENTES", &[]) {
        eprintln!("teardown: DROP VIEW falhou (melhor esforço): {erro:?}");
    }
    if let Err(erro) = adapter.execute("DROP TABLE CLIENTES", &[]) {
        eprintln!("teardown: DROP TABLE falhou (melhor esforço): {erro:?}");
    }
}

/// Formata ServerVersion para log (sem segredos).
fn formatar_versao(v: &ServerVersion) -> String {
    match v.patch {
        Some(patch) => format!("{}.{}.{}", v.major, v.minor, patch),
        None => format!("{}.{}", v.major, v.minor),
    }
}
