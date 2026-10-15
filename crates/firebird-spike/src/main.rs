//! Binário `firebird-spike` — checklist da Fase 0 (prova de conceito) contra
//! um servidor Firebird real, com `libfbclient.so` carregada de forma
//! controlada (Spec 01, §3; ADR-002).
//!
//! A senha vem **exclusivamente** da variável de ambiente [`ENV_SENHA`]:
//! não existe flag de senha, o `--help` nunca mostra uma, e o [`Debug`] de
//! [`Args`] sai redigido (FR-016). A saída é um bloco de relatório por
//! combinação (servidor × biblioteca) com os 6 itens do checklist
//! classificados ✅ (passou), ⚠️ (não executado) ou ❌ (falhou).
//!
//! Este binário **não** declara suporte a nenhuma versão: ele produz evidência
//! de prova de conceito, registrada em `docs/compatibility/POC-FASE0.md`
//! (Spec 01, §1).

use std::path::{Path, PathBuf};

use firebird_adapter::{
    ClientLibrary, ConnParams, FirebirdAdapter, MetadataRepository, RsFbClientAdapter,
};
use firebird_domain::{DomainError, FieldInfo, RelationKind, ServerVersion};

/// Variável de ambiente **única** de onde a senha é lida (FR-016). Nunca
/// existe `--password`/`--senha`: a senha não entra em argv nem em log.
const ENV_SENHA: &str = "FB_PASSWORD";

/// Texto de ajuda. Não contém flag de senha nem valor de senha — apenas o
/// nome da variável de ambiente, que não é segredo.
const AJUDA: &str = "\
firebird-spike — checklist da Fase 0 (prova de conceito) contra um servidor Firebird

Uso:
  firebird-spike --host <H> --port <P> --database <D> --user <U> --lib <caminho> [--charset <C>]

Flags:
  --host <H>       host ou IP do servidor (obrigatória)
  --port <P>       porta TCP do servidor (obrigatória)
  --database <D>   nome ou caminho do banco no servidor (obrigatória)
  --user <U>       usuário de autenticação (obrigatória; ex.: SYSDBA)
  --lib <caminho>  caminho da libfbclient.so a carregar (obrigatória)
  --charset <C>    charset da conexão (opcional; padrão UTF8)
  --help, -h       mostra esta ajuda

A senha NUNCA é aceita por flag: leia-a da variável de ambiente e passe-a
assim, fora do argv:

  FB_PASSWORD=... firebird-spike --host ... --port ... --database ... --user ... --lib ...

Saída: bloco com versão do servidor, biblioteca carregada (caminho + versão),
os 6 itens do checklist classificados ✅ (passou), ⚠️ (não executado) ou ❌
(falhou) e o resultado consolidado.
Código de saída: 0 = 6/6 ✅; 1 = alguma falha ou item não executado; 2 = uso inválido.";

/// Argumentos de execução do spike.
///
/// A senha é preenchida **apenas** a partir de [`ENV_SENHA`] (o parser de argv
/// não a aceita sob nenhuma forma). O [`Debug`] é manual e redige a senha —
/// nunca vaza em log ou DTO (FR-016).
#[derive(Clone, PartialEq, Eq)]
struct Args {
    /// Host ou IP do servidor.
    host: String,
    /// Porta TCP do servidor.
    port: u16,
    /// Nome/caminho do banco no servidor.
    database: String,
    /// Usuário de autenticação (ex.: SYSDBA).
    user: String,
    /// Caminho da `libfbclient.so` a carregar.
    lib: PathBuf,
    /// Charset da conexão (padrão UTF8).
    charset: String,
    /// Senha lida de [`ENV_SENHA`] — nunca exibida em texto cru.
    password: String,
}

impl std::fmt::Debug for Args {
    /// Imprime os campos com a senha redigida como `***` (FR-016).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Args")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("lib", &self.lib)
            .field("charset", &self.charset)
            .field("password", &"***")
            .finish()
    }
}

/// Resultado do parse de argv (+ senha de [`ENV_SENHA`]).
#[derive(Debug)]
enum Cli {
    /// `--help`/`-h`: imprimir [`AJUDA`] e sair com 0.
    Ajuda,
    /// Execução normal do checklist.
    Executar(Args),
}

/// Monta o [`Cli`] a partir de argv e da senha vinda de [`ENV_SENHA`].
///
/// Erros de uso são `Err(String)` prontos para exibição (nunca contêm a
/// senha: ela não faz parte de argv). `--password`/`--senha` são rejeitados
/// com mensagem acionável apontando para a variável de ambiente.
fn montar_args(argv: &[String], senha: Option<String>) -> Result<Cli, String> {
    if argv.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(Cli::Ajuda);
    }
    if argv.is_empty() {
        return Err("nenhuma flag informada (use --help)".to_string());
    }

    let mut host = None;
    let mut port = None;
    let mut database = None;
    let mut user = None;
    let mut lib = None;
    let mut charset = "UTF8".to_string();

    let mut i = 0;
    while i < argv.len() {
        let flag = &argv[i];
        if flag == "--password" || flag == "--senha" {
            return Err(format!(
                "a flag {flag} não é suportada: a senha é lida exclusivamente da variável de ambiente {ENV_SENHA}"
            ));
        }
        let valor = argv
            .get(i + 1)
            .ok_or_else(|| format!("flag {flag} sem valor"))?;
        match flag.as_str() {
            "--host" => host = Some(valor.clone()),
            "--port" => {
                let porta: u16 = valor.parse().map_err(|_| {
                    format!("--port inválida: '{valor}' (esperado número de 1 a 65535)")
                })?;
                if porta == 0 {
                    return Err("--port inválida: 0 (esperado número de 1 a 65535)".to_string());
                }
                port = Some(porta);
            }
            "--database" => database = Some(valor.clone()),
            "--user" => user = Some(valor.clone()),
            "--lib" => lib = Some(PathBuf::from(valor)),
            "--charset" => charset = valor.clone(),
            outro => return Err(format!("flag desconhecida: {outro}")),
        }
        i += 2;
    }

    let host = host.ok_or_else(|| "flag obrigatória ausente: --host".to_string())?;
    let port = port.ok_or_else(|| "flag obrigatória ausente: --port".to_string())?;
    let database = database.ok_or_else(|| "flag obrigatória ausente: --database".to_string())?;
    let user = user.ok_or_else(|| "flag obrigatória ausente: --user".to_string())?;
    let lib = lib.ok_or_else(|| "flag obrigatória ausente: --lib".to_string())?;

    // A senha NUNCA vem de argv: só da env (FR-016). Vazia também não serve.
    let password = senha.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
        format!(
            "variável de ambiente {ENV_SENHA} ausente ou vazia (a senha nunca é passada por flag; exporte {ENV_SENHA} e execute de novo)"
        )
    })?;

    Ok(Cli::Executar(Args {
        host,
        port,
        database,
        user,
        lib,
        charset,
        password,
    }))
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let senha = std::env::var(ENV_SENHA).ok();
    let cli = match montar_args(&argv, senha) {
        Ok(cli) => cli,
        Err(erro) => {
            eprintln!("erro: {erro}");
            eprintln!();
            eprintln!("{AJUDA}");
            std::process::exit(2);
        }
    };
    match cli {
        Cli::Ajuda => println!("{AJUDA}"),
        Cli::Executar(args) => {
            let ok = executar_checklist(&args);
            std::process::exit(if ok { 0 } else { 1 });
        }
    }
}

/// Situação de uma linha do relatório.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    /// ✅ o item passou.
    Passou,
    /// ❌ o item falhou (com o texto real do erro).
    Falhou,
    /// ⚠️ o item não foi executado (com o motivo explícito).
    NaoExecutado,
}

impl Status {
    /// Símbolo exibido na linha.
    fn simbolo(self) -> &'static str {
        match self {
            Status::Passou => "✅",
            Status::Falhou => "❌",
            Status::NaoExecutado => "⚠️",
        }
    }
}

/// Uma linha do bloco de relatório: rótulo (`[1]`…`[6]`, `setup`,
/// `teardown`), situação e texto **censurado** (nunca com a senha em texto
/// cru — FR-016).
struct Linha {
    rotulo: String,
    status: Status,
    texto: String,
}

impl Linha {
    /// É um dos 6 itens numerados do checklist?
    fn e_item(&self) -> bool {
        self.rotulo.starts_with('[')
    }

    /// Imprime a linha no bloco.
    fn imprimir(&self) {
        println!(
            "{:<9} {} {}",
            self.rotulo,
            self.status.simbolo(),
            self.texto
        );
    }
}

/// Monta uma [`Linha`] censurando a senha no texto (FR-016).
fn linha(rotulo: &str, status: Status, texto: impl Into<String>, senha: &str) -> Linha {
    Linha {
        rotulo: rotulo.to_string(),
        status,
        texto: censurar(&texto.into(), senha),
    }
}

/// Substitui toda ocorrência literal da senha por `***` — garantia final de
/// que nenhum erro do driver imprime a credencial em texto cru.
fn censurar(texto: &str, senha: &str) -> String {
    if senha.is_empty() {
        return texto.to_string();
    }
    texto.replace(senha, "***")
}

/// Executa o checklist (6 itens) e imprime o bloco de relatório por
/// combinação; devolve `true` quando os 6 itens passaram (6/6 ✅).
///
/// O schema sintético `CLIENTES`/`V_CLIENTES` só é criado quando **não**
/// pré-existe (pré-checagem via `relations()`): o spike nunca derruba objeto
/// que não criou nesta execução.
fn executar_checklist(args: &Args) -> bool {
    let versao_lib = versao_da_biblioteca(&args.lib);
    let biblioteca = ClientLibrary {
        path: args.lib.clone(),
        version: versao_lib,
    };
    let elf = biblioteca.validate_arch();

    // ---------- Cabeçalho da combinação (sem segredos)
    println!("=== firebird-spike — checklist da Fase 0 (PoC) ===");
    println!("combinação : {}:{}/{}", args.host, args.port, args.database);
    println!("usuário    : {}", args.user);
    println!("charset    : {}", args.charset);
    println!(
        "biblioteca : {} (versão do nome: {})",
        args.lib.display(),
        match versao_lib {
            Some(v) => formatar_versao(&v),
            None => "não identificada no nome".to_string(),
        }
    );
    match &elf {
        Ok(()) => println!("arquitetura: ELF da biblioteca compatível com o processo"),
        Err(erro) => println!(
            "arquitetura: ❌ {}",
            censurar(&format!("{erro:?}"), &args.password)
        ),
    }
    println!("senha      : lida da variável de ambiente {ENV_SENHA} (nunca exibida)");
    println!("---");

    let mut linhas: Vec<Linha> = Vec::with_capacity(8);

    // ---------- Item 1: conexão + server_version
    let params = ConnParams {
        host: args.host.clone(),
        port: args.port,
        database: args.database.clone(),
        user: args.user.clone(),
        password: args.password.clone(),
        charset: args.charset.clone(),
        client_library_path: args.lib.clone(),
    };

    let mut adapter = if let Err(erro) = &elf {
        linhas.push(linha(
            "[1]",
            Status::Falhou,
            format!("conexão + server_version: biblioteca rejeitada na validação de arquitetura: {erro:?}"),
            &args.password,
        ));
        for n in 2..=6 {
            linhas.push(linha(
                &format!("[{n}]"),
                Status::NaoExecutado,
                "não executado: conexão não estabelecida (biblioteca rejeitada na validação de arquitetura)",
                &args.password,
            ));
        }
        return concluir(&linhas);
    } else {
        match RsFbClientAdapter::connect(&params) {
            Ok(adapter) => adapter,
            Err(erro) => {
                linhas.push(linha(
                    "[1]",
                    Status::Falhou,
                    format!("conexão + server_version: conexão falhou: {erro:?}"),
                    &args.password,
                ));
                for n in 2..=6 {
                    linhas.push(linha(
                        &format!("[{n}]"),
                        Status::NaoExecutado,
                        "não executado: conexão indisponível",
                        &args.password,
                    ));
                }
                return concluir(&linhas);
            }
        }
    };

    let versao = match adapter.server_version() {
        Ok(versao) => {
            linhas.push(linha(
                "[1]",
                Status::Passou,
                format!("conexão + server_version = {}", formatar_versao(&versao)),
                &args.password,
            ));
            Some(versao)
        }
        Err(erro) => {
            linhas.push(linha(
                "[1]",
                Status::Falhou,
                format!("conexão ok, mas server_version falhou: {erro:?}"),
                &args.password,
            ));
            None
        }
    };

    // ---------- Setup: schema sintético (pré-checagem primeiro)
    let repo = MetadataRepository::new();
    let mut setup_status = Status::NaoExecutado;
    let mut setup_criado = false;

    let setup_texto: String = if let Some(v) = versao {
        match repo.relations(&mut adapter, &v) {
            Err(erro) => {
                setup_status = Status::Falhou;
                format!("pré-checagem de schema falhou: {erro:?}")
            }
            Ok(relacoes) => {
                let pre_existe = relacoes
                    .iter()
                    .any(|r| r.name == "CLIENTES" || r.name == "V_CLIENTES");
                if pre_existe {
                    "CLIENTES/V_CLIENTES já existem no banco de destino; recusa criar ou derrubar schema que não é desta execução".to_string()
                } else {
                    match criar_schema(&mut adapter) {
                        Ok(()) => {
                            setup_status = Status::Passou;
                            setup_criado = true;
                            "schema sintético CLIENTES/V_CLIENTES criado com 2 linhas (pré-checagem: não pré-existia)"
                                .to_string()
                        }
                        Err(erro) => {
                            setup_status = Status::Falhou;
                            erro
                        }
                    }
                }
            }
        }
    } else {
        "pré-checagem exige server_version (item 1 não informou a versão)".to_string()
    };
    linhas.push(linha(
        "setup",
        setup_status,
        setup_texto.clone(),
        &args.password,
    ));

    // ---------- Itens 2–5: exigem o schema criado nesta execução
    if !setup_criado {
        for (n, rotulo_item) in [
            (2, "RDB$RELATIONS"),
            (3, "RDB$RELATION_FIELDS"),
            (4, "SELECT parametrizado"),
            (5, "round-trip Unicode/decimal/NULL"),
        ] {
            linhas.push(linha(
                &format!("[{n}]"),
                Status::NaoExecutado,
                format!("não executado: {rotulo_item} — {setup_texto}"),
                &args.password,
            ));
        }
    } else {
        let v = versao.expect("setup criado implica versão conhecida");

        // ---------- Item 2: relations() com CLIENTES (Table) e V_CLIENTES (View)
        match repo.relations(&mut adapter, &v) {
            Ok(relacoes) => {
                let tem_tabela = relacoes
                    .iter()
                    .any(|r| r.name == "CLIENTES" && r.kind == RelationKind::Table);
                let tem_view = relacoes
                    .iter()
                    .any(|r| r.name == "V_CLIENTES" && r.kind == RelationKind::View);
                if tem_tabela && tem_view {
                    linhas.push(linha(
                        "[2]",
                        Status::Passou,
                        format!(
                            "RDB$RELATIONS: CLIENTES (Table) e V_CLIENTES (View) entre {} relações",
                            relacoes.len()
                        ),
                        &args.password,
                    ));
                } else {
                    let listagem = relacoes
                        .iter()
                        .map(|r| format!("{}:{:?}", r.name, r.kind))
                        .collect::<Vec<_>>()
                        .join(", ");
                    linhas.push(linha(
                        "[2]",
                        Status::Falhou,
                        format!("RDB$RELATIONS não classificou como esperado (tem_tabela={tem_tabela}, tem_view={tem_view}); relações: {listagem}"),
                        &args.password,
                    ));
                }
            }
            Err(erro) => linhas.push(linha(
                "[2]",
                Status::Falhou,
                format!("RDB$RELATIONS falhou: {erro:?}"),
                &args.password,
            )),
        }

        // ---------- Item 3: relation_fields("CLIENTES")
        match repo.relation_fields(&mut adapter, "CLIENTES") {
            Ok(campos) => {
                let problemas = validar_campos_clientes(&campos);
                if problemas.is_empty() {
                    linhas.push(linha(
                        "[3]",
                        Status::Passou,
                        "RDB$RELATION_FIELDS: 5 colunas com tipo/nullable/domain esperados"
                            .to_string(),
                        &args.password,
                    ));
                } else {
                    linhas.push(linha(
                        "[3]",
                        Status::Falhou,
                        format!(
                            "RDB$RELATION_FIELDS divergiu do esperado: {}",
                            problemas.join(" | ")
                        ),
                        &args.password,
                    ));
                }
            }
            Err(erro) => linhas.push(linha(
                "[3]",
                Status::Falhou,
                format!("RDB$RELATION_FIELDS falhou: {erro:?}"),
                &args.password,
            )),
        }

        // ---------- Item 4: SELECT parametrizado com ?
        match adapter.query_rows_params("SELECT NOME FROM CLIENTES WHERE ID = ?", &[Some("2")]) {
            Ok(linhas_busca) if linhas_busca == vec![vec![Some("O'BRIEN".to_string())]] => {
                linhas.push(linha(
                    "[4]",
                    Status::Passou,
                    "SELECT com ? devolveu a linha certa (O'BRIEN)".to_string(),
                    &args.password,
                ));
            }
            Ok(obtido) => linhas.push(linha(
                "[4]",
                Status::Falhou,
                format!("SELECT com ? devolveu linha errada: {obtido:?}"),
                &args.password,
            )),
            Err(erro) => linhas.push(linha(
                "[4]",
                Status::Falhou,
                format!("SELECT parametrizado falhou: {erro:?}"),
                &args.password,
            )),
        }

        // ---------- Item 5: Unicode / decimal / NULL
        match adapter.query_rows("SELECT NOME, VALOR, OBS, FLAG FROM CLIENTES ORDER BY ID") {
            Ok(resultado) => {
                let problemas = validar_dados_roundtrip(&resultado);
                if problemas.is_empty() {
                    linhas.push(linha(
                        "[5]",
                        Status::Passou,
                        "Unicode/decimal/NULL corretos no round-trip (NULL ≠ vazio)".to_string(),
                        &args.password,
                    ));
                } else {
                    linhas.push(linha(
                        "[5]",
                        Status::Falhou,
                        format!("round-trip divergiu do esperado: {}", problemas.join(" | ")),
                        &args.password,
                    ));
                }
            }
            Err(erro) => linhas.push(linha(
                "[5]",
                Status::Falhou,
                format!("SELECT de dados falhou: {erro:?}"),
                &args.password,
            )),
        }
    }

    // ---------- Item 6: SQL inválido → DomainError::Driver sem senha
    match adapter.query_rows("SELECT COLUNA_INEXISTENTE_POOC FROM TABELA_INEXISTENTE_POOC") {
        Ok(obtido) => linhas.push(linha(
            "[6]",
            Status::Falhou,
            format!("SQL inválido não gerou erro (devolveu {obtido:?})"),
            &args.password,
        )),
        Err(erro) => {
            let debug = format!("{erro:?}");
            if !args.password.is_empty() && debug.contains(&args.password) {
                linhas.push(linha(
                    "[6]",
                    Status::Falhou,
                    format!(
                        "SQL inválido gerou erro com a senha em texto cru (viola FR-016): {debug}"
                    ),
                    &args.password,
                ));
            } else if matches!(erro, DomainError::Driver { .. }) {
                linhas.push(linha(
                    "[6]",
                    Status::Passou,
                    format!("SQL inválido → DomainError::Driver sem senha: {debug}"),
                    &args.password,
                ));
            } else {
                linhas.push(linha(
                    "[6]",
                    Status::Falhou,
                    format!("erro veio de variante inesperada (esperado Driver): {debug}"),
                    &args.password,
                ));
            }
        }
    }

    // ---------- Teardown: melhor esforço, em conexão nova (evidência Task 4:
    // statements em cache seguram lock de metadata; drop da conexão antes)
    if setup_criado {
        drop(adapter);
        match RsFbClientAdapter::connect(&params) {
            Err(erro) => linhas.push(linha(
                "teardown",
                Status::NaoExecutado,
                format!("reconexão para derrubar o schema falhou (melhor esforço): {erro:?}"),
                &args.password,
            )),
            Ok(mut nova) => {
                let mut problemas: Vec<String> = Vec::new();
                if let Err(erro) = nova.execute("DROP VIEW V_CLIENTES", &[]) {
                    problemas.push(format!("DROP VIEW: {erro:?}"));
                }
                if let Err(erro) = nova.execute("DROP TABLE CLIENTES", &[]) {
                    problemas.push(format!("DROP TABLE: {erro:?}"));
                }
                if problemas.is_empty() {
                    linhas.push(linha(
                        "teardown",
                        Status::Passou,
                        "view e tabela derrubadas em conexão nova".to_string(),
                        &args.password,
                    ));
                } else {
                    linhas.push(linha(
                        "teardown",
                        Status::NaoExecutado,
                        format!(
                            "derrubada parcial (melhor esforço): {}",
                            problemas.join(" | ")
                        ),
                        &args.password,
                    ));
                }
            }
        }
    }

    concluir(&linhas)
}

/// Imprime as linhas do relatório e o consolidado dos 6 itens; devolve `true`
/// somente para 6/6 ✅.
fn concluir(linhas: &[Linha]) -> bool {
    println!("---");
    for l in linhas {
        l.imprimir();
    }
    let itens: Vec<&Linha> = linhas.iter().filter(|l| l.e_item()).collect();
    let ok = itens.iter().filter(|l| l.status == Status::Passou).count();
    let falhou = itens.iter().filter(|l| l.status == Status::Falhou).count();
    let pulado = itens
        .iter()
        .filter(|l| l.status == Status::NaoExecutado)
        .count();
    println!("---");
    if ok == 6 && itens.len() == 6 {
        println!(
            "Resultado: 6/6 ✅ — validado no checklist PoC nesta combinação (evidência de prova de conceito, NÃO é declaração de suporte; matriz em docs/compatibility/POC-FASE0.md)"
        );
        true
    } else {
        println!(
            "Resultado: {ok}/6 ✅, {falhou} ❌, {pulado} ⚠️ — não validado nesta combinação; motivos acima (registro honesto em docs/compatibility/POC-FASE0.md)"
        );
        false
    }
}

/// Cria o schema sintético do checklist (tabela `CLIENTES` + view
/// `V_CLIENTES` + 2 linhas) via `execute` (sem fetch — caminho seguro com o
/// bug de classificação do rsfbclient 0.29). Só é chamada após pré-checagem
/// de que nada pré-existe. Erros voltam com o texto real do driver.
fn criar_schema(adapter: &mut RsFbClientAdapter) -> Result<(), String> {
    let ddl_tabela = "CREATE TABLE CLIENTES (\
        ID INTEGER NOT NULL PRIMARY KEY, \
        NOME VARCHAR(60) NOT NULL, \
        VALOR NUMERIC(15,2), \
        OBS BLOB SUB_TYPE TEXT, \
        FLAG INTEGER)";
    adapter
        .execute(ddl_tabela, &[])
        .map_err(|e| format!("CREATE TABLE CLIENTES falhou: {e:?}"))?;
    adapter
        .execute(
            "CREATE VIEW V_CLIENTES AS SELECT ID, NOME FROM CLIENTES",
            &[],
        )
        .map_err(|e| format!("CREATE VIEW V_CLIENTES falhou: {e:?}"))?;

    let insert = "INSERT INTO CLIENTES (ID, NOME, VALOR, OBS, FLAG) VALUES (?, ?, ?, ?, ?)";
    let registros: [(&str, [Option<&str>; 5]); 2] = [
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
    for (id, params) in registros {
        adapter
            .execute(insert, &params)
            .map_err(|e| format!("INSERT do registro {id} falhou: {e:?}"))?;
    }
    Ok(())
}

/// Valida as colunas de `CLIENTES` (item 3): tipos, nullable e domain.
/// Devolve a lista de problemas (vazia = tudo como esperado).
fn validar_campos_clientes(campos: &[FieldInfo]) -> Vec<String> {
    let mut problemas = Vec::new();
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
            None => problemas.push(format!("coluna {nome} ausente em relation_fields")),
            Some(c) => {
                if c.type_name != tipo {
                    problemas.push(format!(
                        "{nome} com type_name '{}' (esperado '{tipo}')",
                        c.type_name
                    ));
                }
                if c.nullable != nullable {
                    problemas.push(format!(
                        "{nome} com nullable={} (esperado {nullable})",
                        c.nullable
                    ));
                }
                if c.domain.is_none() {
                    problemas.push(format!(
                        "{nome} sem domain no catálogo (Firebird gera domínio interno R$ para coluna inline)"
                    ));
                }
            }
        }
    }
    problemas
}

/// Valida o round-trip de Unicode, decimal e NULL (item 5). Devolve a lista
/// de problemas (vazia = tudo como esperado).
fn validar_dados_roundtrip(resultado: &[Vec<Option<String>>]) -> Vec<String> {
    let mut problemas = Vec::new();
    if resultado.len() != 2 {
        problemas.push(format!(
            "esperadas 2 linhas, veio {}: {resultado:?}",
            resultado.len()
        ));
        return problemas;
    }
    let linha1 = &resultado[0];
    let linha2 = &resultado[1];

    comparar(
        linha1.first(),
        &Some("João da Conceição".to_string()),
        "linha1 NOME (Unicode)",
        &mut problemas,
    );
    comparar(
        linha1.get(1),
        &Some("1234.56".to_string()),
        "linha1 VALOR (decimal)",
        &mut problemas,
    );
    comparar(
        linha1.get(2),
        &Some("obs sintética".to_string()),
        "linha1 OBS (BLOB texto)",
        &mut problemas,
    );
    comparar(
        linha1.get(3),
        &Some("1".to_string()),
        "linha1 FLAG",
        &mut problemas,
    );

    comparar(
        linha2.first(),
        &Some("O'BRIEN".to_string()),
        "linha2 NOME (apóstrofo)",
        &mut problemas,
    );
    comparar(
        linha2.get(1),
        &Some("-987.65".to_string()),
        "linha2 VALOR (decimal negativo)",
        &mut problemas,
    );
    comparar(
        linha2.get(2),
        &None,
        "linha2 OBS (NULL ≠ vazio)",
        &mut problemas,
    );
    comparar(
        linha2.get(3),
        &None,
        "linha2 FLAG (NULL ≠ vazio)",
        &mut problemas,
    );

    problemas
}

/// Compara um campo obtido com o esperado, registrando problema com o valor
/// real (nunca silenciando divergência).
fn comparar(
    obtido: Option<&Option<String>>,
    esperado: &Option<String>,
    rotulo: &str,
    problemas: &mut Vec<String>,
) {
    match obtido {
        None => problemas.push(format!("{rotulo}: linha sem a coluna esperada")),
        Some(valor) if valor == esperado => {}
        Some(valor) => problemas.push(format!("{rotulo}: obtido {valor:?}, esperado {esperado:?}")),
    }
}

/// Formata [`ServerVersion`] como `major.minor[.patch]` (sem segredos).
fn formatar_versao(v: &ServerVersion) -> String {
    match v.patch {
        Some(patch) => format!("{}.{}.{}", v.major, v.minor, patch),
        None => format!("{}.{}", v.major, v.minor),
    }
}

/// Deduz a versão da biblioteca pelo nome do arquivo
/// (`libfbclient.so.2.5.9` → 2.5.9; `libfbclient.so` → [`None`]).
fn versao_da_biblioteca(caminho: &Path) -> Option<ServerVersion> {
    let nome = caminho.file_name()?.to_str()?;
    let resto = nome.strip_prefix("libfbclient.so")?.strip_prefix('.')?;
    if resto.is_empty() {
        return None;
    }
    ServerVersion::parse(resto)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Senha sentinela usada nos testes — nunca dado real (FR-016).
    const SENTINELA: &str = "sentinela-nao-imprimir-918273";

    /// Flags válidas e completas para montar um [`Cli::Executar`].
    fn flags_completas() -> Vec<String> {
        [
            "--host",
            "127.0.0.1",
            "--port",
            "3051",
            "--database",
            "/firebird/data/poc.fdb",
            "--user",
            "SYSDBA",
            "--lib",
            "/bibliotecas/libfbclient.so.2.5.9",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    #[test]
    fn ajuda_via_cli_nao_exibe_flag_nem_valor_de_senha() {
        // Simula a execução `--help` com a senha disponível no ambiente: o
        // resultado (que main imprime) não pode revelá-la nem sugerir flag.
        let cli = montar_args(&["--help".to_string()], Some(SENTINELA.to_string()))
            .expect("--help deve ser aceito");
        let texto = format!("{cli:?}");
        assert!(
            !texto.contains(SENTINELA),
            "a senha vazou na ajuda: {texto}"
        );
        assert!(
            !texto.contains("--password"),
            "ajuda não pode ter flag de senha: {texto}"
        );
        assert!(
            !texto.contains("--senha"),
            "ajuda não pode ter flag de senha: {texto}"
        );
        assert!(
            AJUDA.contains(ENV_SENHA),
            "a ajuda deve documentar a variável de ambiente da senha"
        );
        assert!(matches!(cli, Cli::Ajuda), "--help deve virar Cli::Ajuda");
    }

    #[test]
    fn debug_de_args_redige_a_senha() {
        let args = Args {
            host: "localhost".to_string(),
            port: 3050,
            database: "/dados/app.fdb".to_string(),
            user: "SYSDBA".to_string(),
            lib: PathBuf::from("/libs/libfbclient.so.3.0.11"),
            charset: "UTF8".to_string(),
            password: SENTINELA.to_string(),
        };
        let texto = format!("{args:?}");
        assert!(
            !texto.contains(SENTINELA),
            "a senha vazou no Debug: {texto}"
        );
        assert!(
            texto.contains("password: \"***\""),
            "a senha deve sair redigida como ***: {texto}"
        );
        assert!(
            texto.contains("Args"),
            "o Debug deve nomear o tipo: {texto}"
        );
    }

    #[test]
    fn senha_e_rejeitada_quando_vem_por_flag() {
        let mut argv = flags_completas();
        argv.push("--password".to_string());
        argv.push("super-segreto-de-argv".to_string());
        let erro = montar_args(&argv, Some(SENTINELA.to_string()))
            .expect_err("--password não pode ser aceito");
        assert!(
            erro.contains("--password"),
            "erro deve nomear a flag: {erro}"
        );
        assert!(
            erro.contains(ENV_SENHA),
            "erro deve apontar a variável de ambiente: {erro}"
        );
        assert!(
            !erro.contains("super-segreto-de-argv"),
            "o valor da flag nunca pode aparecer no erro: {erro}"
        );
    }

    #[test]
    fn senha_ausente_na_env_e_erro_acionavel() {
        let erro = montar_args(&flags_completas(), None)
            .expect_err("sem senha de env a execução não pode começar");
        assert!(
            erro.contains(ENV_SENHA),
            "erro deve nomear a variável de ambiente: {erro}"
        );
    }

    #[test]
    fn flags_completas_montam_args_com_charset_padrao() {
        let cli = montar_args(&flags_completas(), Some(SENTINELA.to_string()))
            .expect("flags completas devem montar o Cli");
        let Cli::Executar(args) = cli else {
            panic!("esperado Cli::Executar, obtido {cli:?}");
        };
        assert_eq!(args.host, "127.0.0.1");
        assert_eq!(args.port, 3051);
        assert_eq!(args.database, "/firebird/data/poc.fdb");
        assert_eq!(args.user, "SYSDBA");
        assert_eq!(args.lib, PathBuf::from("/bibliotecas/libfbclient.so.2.5.9"));
        assert_eq!(args.charset, "UTF8");
        assert_eq!(args.password, SENTINELA);
    }

    #[test]
    fn charset_pode_ser_sobrescrito_pelo_argv() {
        let mut argv = flags_completas();
        argv.push("--charset".to_string());
        argv.push("ISO8859_1".to_string());
        let cli = montar_args(&argv, Some(SENTINELA.to_string())).expect("charset válido");
        let Cli::Executar(args) = cli else {
            panic!("esperado Cli::Executar");
        };
        assert_eq!(args.charset, "ISO8859_1");
    }

    #[test]
    fn porta_invalida_gera_erro_de_uso() {
        let mut argv = flags_completas();
        argv[3] = "abc".to_string();
        let erro = montar_args(&argv, Some(SENTINELA.to_string()))
            .expect_err("porta não numérica deve falhar");
        assert!(erro.contains("--port"), "erro deve nomear a flag: {erro}");
    }

    #[test]
    fn flag_desconhecida_gera_erro_sem_capturar_o_valor() {
        let argv = vec![
            "--banana".to_string(),
            "segredo-de-valor".to_string(),
            "--host".to_string(),
        ];
        let erro = montar_args(&argv, Some(SENTINELA.to_string())).expect_err("flag desconhecida");
        assert!(erro.contains("--banana"), "erro deve nomear a flag: {erro}");
        assert!(
            !erro.contains("segredo-de-valor"),
            "o valor após a flag não pode aparecer: {erro}"
        );
    }

    #[test]
    fn flag_sem_valor_gera_erro_de_uso() {
        let mut argv = flags_completas();
        argv.pop(); // remove o valor de --lib
        let erro = montar_args(&argv, Some(SENTINELA.to_string())).expect_err("flag sem valor");
        assert!(
            erro.contains("--lib"),
            "erro deve nomear a flag pendente: {erro}"
        );
    }

    #[test]
    fn flag_obrigatoria_ausente_gera_erro_nomeando_a_flag() {
        let argv = vec![
            "--host".to_string(),
            "h".to_string(),
            "--port".to_string(),
            "3050".to_string(),
        ];
        let erro = montar_args(&argv, Some(SENTINELA.to_string())).expect_err("faltam flags");
        assert!(
            erro.contains("--database") || erro.contains("--user") || erro.contains("--lib"),
            "erro deve nomear a flag que falta: {erro}"
        );
    }

    #[test]
    fn argv_vazio_gera_erro_de_uso() {
        let erro = montar_args(&[], Some(SENTINELA.to_string())).expect_err("sem args");
        assert!(erro.contains("--help"), "erro deve apontar --help: {erro}");
    }

    #[test]
    fn versao_da_biblioteca_interpreta_nome_versionado() {
        assert_eq!(
            versao_da_biblioteca(Path::new("/x/libfbclient.so.2.5.9"))
                .map(|v| (v.major, v.minor, v.patch)),
            Some((2, 5, Some(9)))
        );
        assert_eq!(
            versao_da_biblioteca(Path::new("libfbclient.so.3.0.11"))
                .map(|v| (v.major, v.minor, v.patch)),
            Some((3, 0, Some(11)))
        );
    }

    #[test]
    fn versao_da_biblioteca_sem_sufixo_e_none() {
        assert_eq!(versao_da_biblioteca(Path::new("/x/libfbclient.so")), None);
        assert_eq!(versao_da_biblioteca(Path::new("/x/outro.so.1.0")), None);
        assert_eq!(versao_da_biblioteca(Path::new("/x/")), None);
    }
}
