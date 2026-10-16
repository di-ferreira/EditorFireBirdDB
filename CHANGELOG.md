# Changelog

Todo formato notável deste projeto é documentado neste arquivo.
Baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e
[Semantic Versioning](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado

- **Fase 1 — Fundação**:
  - `crates/firebird-storage`: SQLite (`rusqlite` bundled), migração
    `0001_inicial.sql` com as 7 tabelas da Spec 09 (sem coluna de senha — apenas
    `secret_ref`), e repositórios de perfis de conexão, histórico de consultas e
    scripts salvos com SQL parametrizado.
  - `crates/firebird-security`: `SecretStore` sobre o cofre do sistema (Secret
    Service via `keyring` 4.2) + implementação em memória para testes;
    `secret_ref` determinístico e redação de segredo em `Debug`/`Display`.
  - Domínio de conexão (`crates/firebird-domain/src/connection.rs`):
    `ConnectionProfile`, `SqlDialect`, `ConnectionStatus` e o ciclo de vida de sessão.
  - `ConnectionManager` com injeção de dependência (`SecretStore` + `Connector`),
    testável sem servidor Firebird.
  - Seis comandos Tauri da Spec 03/10 — `connection_list_profiles`,
    `connection_save_profile`, `connection_test_profile`, `connection_open`,
    `connection_close`, `connection_get_status` — com DTOs `camelCase`, erros
    tipados `{ code, message }` e **nenhum campo de senha** em DTO.
  - UI de conexões (`apps/desktop/src/features/connections/`): listar, criar, editar,
    testar, conectar e desconectar, com estados de loading/vazio/erro/sucesso, em
    pt-BR e acessível por teclado; abas "Saúde" e "Conexões".
  - Pipeline CI (`.github/workflows/ci.yml`): fmt, clippy `-D warnings`,
    `cargo test --locked --workspace`, build do frontend e matriz de integração
    Firebird 2.5/3.0/4.0/5.0 com espera de healthcheck e extração da lib cliente do
    próprio container; Dependabot para cargo e npm.
  - Plano da Fase 1 em `docs/plans/2026-10-15-fase1-fundacao.md` e backlog herdado
    da Fase 0 em `docs/plans/2026-10-15-backlog-fase1-heranca-da-fase0.md`.
- **Fase 2 — Passo 1: catálogo do DB Explorer (sob demanda + refresh)**:
  - Modelos puros em `firebird_domain::catalog` (InfoBanco, IndiceInfo,
    ConstraintInfo, DominioInfo, GeneratorInfo, ProceduraInfo, ParametroInfo,
    TriggerInfo, CategoriaCatalogo x7).
  - `MetadataRepository`: consultas `RDB$` versionadas para informações do banco,
    relações filtráveis (incl. `include_system`), índices, constraints, domínios,
    generators, procedures (+parâmetros) e triggers; compatibilidade provada nas
    versões 2.5/3.0/4.0 (5.0 pendente de host com glibc ≥ 2.36).
  - `CatalogoService` (SPA) com cache por sessão (apenas sem filtro) e
    `forceRefresh`; comando Tauri `catalog_get_node` (`profileId, node, filter,
    includeSystem, forceRefresh`) devolvendo DOI `CatalogNodeDto` com **um nível**
    de filhos — cada filho carrega o próprio `request` (lazy).
  - `ConnectionManager::executar_no_adaptador_conectado` e correção real de
    `server_version` registrada na sessão.
  - UI `features/db-explorer` (árvore lazy com expandir/recolher, refresh por nó
    e da árvore, busca com debounce, marcação de objeto de sistema, detalhe,
    navegação por teclado `role=tree` com `aria-expanded`/`aria-level`, estados
    de carregamento/vazio/erro, pt-BR) integrada como aba "Explorador"; perfis
    conectados vêm dos comandos de conexão já existentes.
- **Fase 2 — Passos 2 a 4: SQL Editor, execução e histórico/scripts**:
  - `FirebirdAdapter::query_rows_with_columns` (nomes de coluna + linhas como texto),
    com implementações real (`rsfbclient`) e sintéticas.
  - Comandos Tauri `query_run`, `script_save`, `script_get`, `script_list`,
    `script_delete`, `history_list` e `history_clear`; `query_run` classifica a
    instrução (SELECT/WITH → linhas; demais → afetadas), mede duração e grava o
    histórico automaticamente (falha de storage não quebra a execução).
  - `BancoLocal` expõe `QueryHistoryRepository` e `SavedScriptRepository` (chamadas
    totalmente qualificadas por trait, sem ambiguidade de nomes).
  - UI `features/sql-editor`: CodeMirror 6 com realce SQL, abas, execução de múltiplos
    statements (parser de delimitadores que respeita literais e comentários), grade de
    resultados com `NULL` distinguível, scripts salvos e histórico.
- **Fase 2 — Ajustes de UX (hardening com banco real)**:
  - Layout full-height (`calc(100vh - 1rem)`) e painel SQL ocupando a tela inteira.
  - Lista filtrável de tabelas/views/procedures integrada ao editor; **duplo clique**
    abre o objeto em nova aba com `SELECT FIRST 500 * FROM <objeto>`.
  - Grade com scroll vertical (`45vh`) e carregamento progressivo de 100 linhas por
    scroll, em vez de renderizar o resultado inteiro.
  - Autocomplete no editor: palavras-chave SQL e nomes de objetos do banco.
  - Atalhos no estilo IBExpert: `Ctrl+Enter`/`F9` (executar), `Ctrl+Shift+Enter`,
    `Ctrl+S` (salvar script), `Ctrl+N` (nova aba), `Ctrl+W` (fechar aba).
  - Tema claro/escuro alternável, persistido em `localStorage`.
  - Correções de inicialização: SQLite em `app_data_dir/studio.db` (o caminho cru
    quebrava a abertura) e remoção de `devUrl` para o binário release carregar os
    assets locais.
- Estrutura inicial do repositório (monorepo pnpm + workspace Cargo Rust).
- Especificação técnica extraída por tema em `docs/specs/` (17 documentos, do documento
  original v0.1 preservado em `docs/original/`).
- Templates de ADR-001 a ADR-006 em `docs/adr/`.
- Agents de desenvolvimento do OpenCode em `.opencode/agent/` (spec-analyst, architect,
  rust-domain-dev, firebird-adapter-dev, frontend-dev, test-engineer, security-reviewer,
  docs-writer).
- Governança: LICENSE (Apache-2.0), README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY,
  THIRD_PARTY_NOTICES.
- Crates Rust da Fase 0: `firebird-domain` (domínio puro, erros redigidos FR-016),
  `firebird-adapter` (trait `FirebirdAdapter`, descoberta/validação ELF da
  `libfbclient.so`, `MetadataRepository` com variantes `RDB$` por versão, adapter
  `rsfbclient-native` em dynamic loading).
- Harness de integração por versão do Firebird (`scripts/firebird-env.sh` +
  `tests/integration-firebird`): checklist de 6 itens contra ambientes Docker
  2.5/3.0/4.0/5.0 e servidor local; sem as env vars `FB_POC_*` é ignorado e nunca
  falha o `cargo test`.
- Binário `crates/firebird-spike` (CLI da prova de conceito): executa o checklist de
  compatibilidade contra um servidor real com `libfbclient.so` escolhida (`--lib`);
  senha exclusivamente via env `FB_PASSWORD`, nunca em flag/argv/ajuda, com `Debug`
  redigido (FR-016); saída por combinação com classificação ✅/⚠️/❌.
- Relatório da prova de conceito da Fase 0 em `docs/compatibility/POC-FASE0.md`: em
  2026-10-15 o checklist PoC foi **validado (6/6)** nas combinações
  servidor 2.5.9 × cliente 2.5.9 (Docker), 3.0.10 × 3.0.10 (Docker),
  4.0.0 × 3.0.10 (Docker), 5.0.0 × 3.0.10 (Docker) e 2.5.9 local × 3.0.10 (local
  `:3050`); **2.0/2.1: não testados — pendente de ambiente**; lib do host 3.0.11 não
  exercitada (limitações e falhas reais registradas no relatório).

### Alterado

- ADR-002 (driver Firebird) → status **"Decidida (parcialmente)"**: estratégia primária
  `rsfbclient-native` 0.29 com `dynamic_loading` + `DynLoad.lib_path`; evidências da
  matriz PoC, riscos restantes (bug de classificação de statement/`RETURNING`, lib
  3.0.11 não testada, 2.0/2.1 sem ambiente, libs do host ilegíveis) e pendências da
  Fase 1 documentados no ADR.
- Harness de integração: o setup passou a **recusar** criar/derrubar `CLIENTES` e
  `V_CLIENTES` que já existam no banco de destino (o harness pode ser apontado para um
  banco não descartável); regra coberta por testes unitários sem servidor.
- `firebird_domain` expõe `VERSION` (versão real do crate de domínio, consumida pelo
  comando `app_health`) e um utilitário de redação de segredo reutilizável.

### Limitações conhecidas

- `connectTimeoutMs` é persistido no perfil mas ainda **não aplicado** à conexão
  (`ConnParams` não transporta timeout) — a aplicar.
- `reconectar()` existe no `ConnectionManager` mas ainda não é exposto por comando.
- `UnsupportedServerFeature`, `QueryError` e `ProviderError` estão no enum de erros da
  fronteira, sem produtor até a Fase 2/3.

[Não lançado]: https://github.com/di-ferreira/EditorFireBirdDB