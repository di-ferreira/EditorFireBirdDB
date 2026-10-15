# Fase 0 — Prova de Conceito Técnica (Firebird Studio Linux) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended)
> or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`)
> syntax for tracking.

**Goal:** Provar viabilidade técnica de conexão/leitura de metadados/execução de SQL
contra múltiplas versões de Firebird (2.0–5.0) com `libfbclient.so` carregada de forma
controlada, gerando o relatório PoC e a decisão da ADR-002.

**Architecture:** Workspace Cargo com crates puros (`firebird-domain` sem dependências de
infra; `firebird-adapter` com trait `FirebirdAdapter` + backend `rsfbclient-native` em
modo dynamic loading apontando para `lib_path` explícito), harness de integração por
versão via Docker + servidor local (porta 3050 já ativa), binário `firebird-spike` que
executa o checklist da Fase 0 e emite o relatório. Scaffold mínimo Tauri 2 + React é
criado em paralelo.

**Tech Stack:** Rust 1.99 (estável), rsfbclient 0.29 / rsfbclient-native 0.29 (feature
`dynamic_loading`), Docker (imagens Firebird por versão), Node 24 + pnpm 11 + Vite +
React, Tauri 2 (webkit2gtk-4.1 presente).

**Specs:**
- `docs/specs/01-compatibilidade-firebird.md` (libfbclient controlada, matriz)
- `docs/specs/14-plano-entrega.md` (Fase 0: checklist obrigatório)
- `docs/specs/12-testes-tdd.md` (TDD, ambientes isolados por versão)
- `docs/adr/ADR-002-driver-firebird.md` (decisão pendente que este plano resolve)

## Global Constraints

- TDD obrigatório em `domain/` e `application/` (teste antes da implementação).
- Nenhuma senha/token em log, erro ou DTO (FR-016); senha lida apenas de env var.
- Nunca substituir bibliotecas do sistema nem criar symlinks globais (Spec 01 §3).
- Normalização `RDB$`: trim de preenchimento `CHAR`; campos ausentes = `None`, nunca
  inventados (Spec 01 §4).
- Nenhum segredo, dump real ou banco de cliente em fixtures — apenas dados sintéticos.
- Commits Conventional Commits, um por mudança lógica (ao final, via skill git).
- Ambiente: Rust 1.99.0, Node v24.13.0, pnpm 11.5.1, Docker 29.1.5, webkit2gtk-4.1 OK.
  Servidor Firebird local escutando em `0.0.0.0:3050` (versão a identificar).
  Bibliotecas disponíveis: `/usr/lib/libfbclient.so.2.5.9` e `/usr/lib/libfbclient.so.3.0.11`.

## Estrutura de arquivos (visão geral)

```text
crates/firebird-domain/       Task 1 — domínio puro (ServerVersion, erros, modelos)
crates/firebird-adapter/      Tasks 2–3 — trait, DynLoad, MetadataRepository
crates/firebird-spike/        Task 5 — binário do checklist PoC
tests/integration-firebird/   Task 4 — testes por versão (env-gated)
scripts/firebird-env.sh       Task 4 — sobe/desce servidores Docker por versão
apps/desktop/                 Task 6 — scaffold React/Vite mínimo
apps/desktop/src-tauri/       Task 6 — app Tauri mínimo com 1 comando
docs/compatibility/POC-FASE0.md  Task 5 — relatório final
docs/adr/ADR-002-driver-firebird.md  Task 5 — atualização com evidências
```

---

### Task 1: Workspace Cargo + crate `firebird-domain` (TDD)

**Files:**
- Modify: `Cargo.toml` (raiz — `members`)
- Create: `crates/firebird-domain/Cargo.toml`, `crates/firebird-domain/src/{lib.rs,version.rs,error.rs,metadata.rs}`

**Interfaces (Produz — usadas pelas Tasks 2–6):**
- `ServerVersion { major: u8, minor: u8, patch: Option<u8> }` com
  `ServerVersion::parse(raw: &str) -> Option<ServerVersion>` e
  `ServerVersion::supports_relation_type(&self) -> bool` (true para >= 2.0 com
  `RDB$RELATION_TYPE`; usar gate `>= (2,1)` se 2.0 falhar nos testes reais — decidir na Task 4).
- `DomainError` (thiserror) com variantes `ParseVersion`, `Driver`, `Catalog`, e método
  `pub fn redacted(self) -> String` que **nunca** inclui senha (teste obrigatório:
  mensagem contendo "password=abc123" sai redigida como "password=***").
- Modelos: `RelationKind { Table, View, Other }`,
  `RelationSummary { name: String, kind: RelationKind }`,
  `FieldInfo { name: String, type_name: String, domain: Option<String>, nullable: bool,
  default_value: Option<String>, position: i32 }`.

- [ ] **Step 1: Escrever testes falhando** (`#[cfg(test)]` em cada módulo):
  - parse de `"WI-V2.5.9.27139 Firebird 2.5 WI-V"` → (2,5,9); `"TDSV5.0.2.16063 Firebird 5.0"` → (5,0,2); `"2.5"` → (2,5,None); `""` → None.
  - redação: `DomainError::Driver { message: "password=abc123 denied" }.redacted()` não contém `abc123`.
- [ ] **Step 2:** `cargo test -p firebird-domain` → FAIL (crate inexistente).
- [ ] **Step 3:** Implementar `version.rs`/`error.rs`/`metadata.rs` com regex-free parsing
  (split em não-dígitos; extrair primeira sequência `d+.d+(.d+)?` após remover prefixos `WI-V`/`TDSV`/`OI-V`).
- [ ] **Step 4:** `cargo test -p firebird-domain` → PASS; `cargo clippy -p firebird-domain` limpo.

### Task 2: Crate `firebird-adapter` — descoberta de biblioteca + trait + conexão (TDD)

**Files:**
- Create: `crates/firebird-adapter/Cargo.toml` (deps: `firebird-domain`, `rsfbclient`, `rsfbclient-native` com features `dynamic_loading`+`chrono_support`, `thiserror`), `src/{lib.rs,library.rs,adapter.rs}`

**Interfaces:**
- `ClientLibrary { path: PathBuf, version: Option<ServerVersion> }`:
  - `discover() -> Vec<ClientLibrary>` — varre `/usr/lib`, `/usr/lib64`, `/usr/lib/x86_64-linux-gnu`, `/opt/firebird/lib`, `/usr/local/firebird/lib` por `libfbclient.so*` (não seguir symlink para duplicar), extrai versão do nome (`libfbclient.so.2.5.9` → 2.5.9).
  - `validate_arch(&self) -> Result<(), DomainError>` — lê os 20 primeiros bytes ELF
    (magic `\x7fELF`, `e_machine` offset 18: 62=x86_64) e compara com o processo atual.
- `trait FirebirdAdapter { fn server_version(&mut self) -> Result<ServerVersion, DomainError>;
  fn query_rows(&mut self, sql: &str) -> Result<Vec<Vec<Option<String>>>, DomainError>; }`
  (versão spike simplificada; será estendida na Fase 1)
- `struct RsFbClientAdapter { conn: Connection<rsfbclient_native::DynLoad, ManualTc> }`
  com `connect(params: &ConnParams) -> Result<Self, DomainError>` usando
  `DynLoad { charset, lib_path: params.client_library_path }`.
- `ConnParams { host, port, database, user, password, charset, client_library_path }` —
  `Debug`/`Display` manual **redigindo password**.

- [ ] **Step 1: Testes falhando** — parsing de nome de arquivo (casos: com/sem versão, `.so.3.0.11`, symlink); ELF: bytes fixture `\x7fELF\x02\x01\x01...` e_machine=62 → OK, e_machine=40 (AArch64) → Err; `format!("{:?}", ConnParams{password:"segredo"})` não contém "segredo".
- [ ] **Step 2:** `cargo test -p firebird-adapter` → FAIL.
- [ ] **Step 3:** Implementar `library.rs` (discover + ELF check manual, sem crates extras) e `adapter.rs`.
- [ ] **Step 4:** `cargo test -p firebird-adapter` → PASS.

### Task 3: `MetadataRepository` com variantes de catálogo por versão (TDD com fake)

**Files:**
- Create: `crates/firebird-adapter/src/metadata.rs`, `crates/firebird-adapter/src/fake.rs` (`#[cfg(test)]` ou feature `fake`)

**Interfaces:**
- `MetadataRepository`:
  - `relations(&self, adapter: &mut dyn FirebirdAdapter, version: &ServerVersion) -> Result<Vec<RelationSummary>, DomainError>` — SQL usa `RDB$RELATION_TYPE` quando suportado; fallback para `RDB$VIEW_TYPE`/ausência de system flag. Remove `RDB$SYSTEM_FLAG = 1` (objetos de sistema).
  - `relation_fields(&self, adapter: &mut dyn FirebirdAdapter, relation: &str) -> Result<Vec<FieldInfo>, DomainError>` — trim de `RDB$FIELD_NAME`/`RDB$RELATION_NAME` (CHAR padding), escape de identificador entre aspas.
- `FakeAdapter` (testes): devolve linhas com espaçamento `CHAR` (`"CLIENTES            "`) e registra SQL executado para asserção de variante.

- [ ] **Step 1: Testes falhando** — fakeAdapter com linhas espaçadas → `RelationSummary.name == "CLIENTES"`; versão (2,5) → SQL contém `RDB$RELATION_TYPE`; versão (2,0,se suportado) → variante fallback; `relation` com aspas duplas escapadas corretamente.
- [ ] **Step 2:** FAIL → **Step 3:** implementar → **Step 4:** PASS + clippy.

### Task 4: Harness de integração por versão (Docker + servidor local)

**Files:**
- Create: `scripts/firebird-env.sh` (up|down|status|urls)
- Create: `tests/integration-firebird/Cargo.toml` (package independente no workspace), `tests/integration-firebird/tests/poc_checklist.rs`
- Modify: `Cargo.toml` (raiz — adicionar `tests/integration-firebird` aos members)

**Comportamento:**
- `scripts/firebird-env.sh up 2.5` → `docker run -d --name fb-studio-2.5 -p 3051:3050 -e FIREBIRD_DATABASE=poc.fdb jacobalberty/firebird:2.5` (tags 2.5, 3.0, 4.0, 5.0; se 2.0/2.1 não tiverem imagem viável, registrar "sem ambiente reproduzível" no relatório). `down` para e remove; `status` mostra portas.
- Testes leem env vars `FB_POC_HOST/PORT/DATABASE/USER/PASSWORD/LIB_PATH` e rodam o checklist:
  1. connect + `server_version`;
  2. `SELECT rdb$get_context('SYSTEM','ENGINE VERSION') FROM rdb$database` (fallback quando aplicável);
  3. `relations()` contém tabela sintética criada pelo harness (`CREATE TABLE CLIENTES...` com Unicode: nome `João da Conceição`, coluna `DECIMAL(15,2)`, `BLOB`, `NULL`);
  4. `relation_fields("CLIENTES")` com tipos corretos;
  5. `SELECT` parametrizado com `?` (se suportado pelo backend) e checagem de Unicode/decimal/NULL;
  6. erro de SQL inválido → mensagem mapeada, **sem senha** na string.
- Sem env vars → teste é ignorado com aviso claro (`eprintln!` + return), nunca falha o `cargo test` local.

- [ ] **Step 1:** Escrever `scripts/firebird-env.sh` e os testes (skipped sem env).
- [ ] **Step 2:** `cargo test -p integration-firebird` → compila e reporta "ignorado (sem env)".
- [ ] **Step 3:** `./scripts/firebird-env.sh up 3.0` + env vars → `cargo test -p integration-firebird -- --nocapture` → PASS (ou registro honesto de falha).
- [ ] **Step 4:** Repetir para 2.5; tentar 4.0/5.0 (pull da imagem); registrar resultado por versão.

### Task 5: Binário `firebird-spike` + relatório PoC + atualização da ADR-002

**Files:**
- Create: `crates/firebird-spike/Cargo.toml`, `crates/firebird-spike/src/main.rs`
- Create: `docs/compatibility/POC-FASE0.md`
- Modify: `docs/adr/ADR-002-driver-firebird.md`, `CHANGELOG.md`

**Comportamento do spike (CLI):**
`firebird-spike --host H --port P --database D --user U --lib L` (senha exclusivamente
via env `FB_PASSWORD`, nunca via flag/argv). Executa o checklist da Fase 0 e imprime
bloco de relatório por combinação: versão do servidor, biblioteca carregada (caminho +
versão), leitura de `RDB$RELATIONS`/`RDB$RELATION_FIELDS`, SELECT com parâmetro,
Unicode/decimal/NULL, mensagem de erro, classificação: ✅/⚠️/❌.

**Execução:**
1. Servidor local :3050 (identificar versão; usuário=SYSDBA; senha via env do operador).
2. Docker 2.5, 3.0, 4.0, 5.0 conforme disponibilidade de imagem.
3. 2.0/2.1: sem ambiente reproduzível → estado "não testado — pendente de ambiente"
   (nunca "suportado").

- [ ] **Step 1:** Implementar CLI com checklist.
- [ ] **Step 2:** Rodar contra cada ambiente disponível; salvar saída.
- [ ] **Step 3:** Escrever `docs/compatibility/POC-FASE0.md` com matriz honesta
  (combinação, resultado, limitações, incl. falhas).
- [ ] **Step 4:** Atualizar ADR-002 → "Decidida (parcialmente)": driver
  `rsfbclient-native` com `DynLoad.lib_path` como estratégia primária; evidências e
  lacunas apontadas para a Fase 1. Atualizar CHANGELOG.

### Task 6: Scaffold mínimo Tauri 2 + React

**Files:**
- Create: `apps/desktop/package.json`, `apps/desktop/vite.config.ts`, `apps/desktop/tsconfig.json`, `apps/desktop/index.html`, `apps/desktop/src/main.tsx`, `apps/desktop/src/App.tsx`
- Create: `apps/desktop/src-tauri/Cargo.toml`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/build.rs`
- Modify: `Cargo.toml` (raiz — adicionar `apps/desktop/src-tauri` aos members)
- Modify: `pnpm-workspace.yaml` (já cobre `apps/*`)

**Comportamento:** app abre janela com título "Firebird Studio Linux" e UI que chama o
comando `app_health` → `{ status: "ok", domain_version: <ServerVersion do crate domain> }`.
Permissões mínimas (nenhuma extra além das padrão do core). Sem credenciais embutidas.

- [ ] **Step 1:** Scaffold React/Vite (`pnpm dev` responde; `pnpm build` passa).
- [ ] **Step 2:** `src-tauri` com comando `app_health`; `cargo build -p firebird-studio-desktop` (ou o nome definido) compila.
- [ ] **Step 3:** Verificação final: `cargo test --workspace` verde (integração ignorados sem env), `pnpm build` verde.

---

## Self-review (feito)

- Cobertura Fase 0: app mínimo Tauri (Task 6) ✔; libfbclient controlada (Tasks 2/4) ✔;
  conexão a versões múltiplas (Tasks 4/5) ✔; versão + RDB$RELATIONS + RDB$RELATION_FIELDS
  + SELECT (Tasks 3/4/5) ✔; Unicode/dialeto/parâmetros/tipos/erros (Task 4) ✔;
  ADR do driver (Task 5) ✔. Saída obrigatória = PoC report (Task 5) ✔.
- Placeholders: nenhum "TBD/TODO"; caminhos e assinaturas exatos definidos acima.
- Consistência de tipos: `ServerVersion`, `DomainError`, `FirebirdAdapter`,
  `MetadataRepository`, `RelationSummary`, `FieldInfo` usados igualmente em todas as tasks.
