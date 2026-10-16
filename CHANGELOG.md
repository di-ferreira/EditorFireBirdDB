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
- **Paginação no servidor, cancelamento e exportação CSV**:
  - `DomainError::Cancelled` + `CommandErrorCode::Cancelled` (cancelamento não é falha).
  - `FirebirdAdapter::query_page_with_columns`: embrulha o `SELECT` com
    `ROWS <offset+1> TO <offset+limit+1>` e lê linha a linha checando a bandeira de
    cancelamento; fallback executa e fatia localmente quando o SQL não pode ser
    subconsulta.
  - Registro de cancelamento por `queryId` no `AppState`
    (`registrar_cancelamento`/`encerrar_cancelamento`/`cancelar`), comando
    `query_cancel`, e `query_run` estendido com `queryId`/`offset`/`limit`
    (parâmetros opcionais: quem não envia mantém o comportamento anterior).
  - Grade consome páginas de 100 linhas ao chegar ao fim do scroll, com botão
    **Carregar mais** e **Exportar CSV** (UTF-8 com BOM, `;` e aspas duplicadas).
  - CSP ampliada para `blob:` (download do CSV).
- **Edição de dados na grade e transação manual**:
  - `MetadataRepository::chave_primaria` (`RDB$RELATION_CONSTRAINTS` →
    `RDB$INDICES` → `RDB$INDEX_SEGMENTS`) e comando `table_keys`; o nome da relação
    vai escapado com `escapar_literal`.
  - Métodos de transação no `FirebirdAdapter` (`begin_transaction`, `commit`,
    `rollback`, `in_transaction`) sobre `Connection::begin_transaction/commit/rollback`
    do rsfbclient (com espelho do estado, pois o campo é privado no driver) e comandos
    `transaction_begin`, `transaction_commit`, `transaction_rollback`.
  - UI: interruptor **Transação manual** + botões **Commit**/**Rollback** ao lado de
    Executar, com aviso de estado; as alterações passam a respeitar a transação aberta.
  - Grade editável em abas de dados (duplo clique numa tabela): duplo clique na célula
    edita e grava com `UPDATE ... WHERE <chave primária>`. Sem PK (ou sem a PK no
    SELECT) a edição é bloqueada com aviso — nunca há UPDATE sem WHERE.
- **Fallback do erro `-204` (block size)**:
  - `ColunaInfo::excede_buffer_seguro` no domínio (BLOB ou campo > 4.000 bytes) e
    `MetadataRepository::colunas` (`RDB$RELATION_FIELDS` + `RDB$FIELDS`) com o
    comando `table_columns`.
  - Ao abrir uma tabela cujo `SELECT *` estoura o limite de buffer do engine, o
    editor refaz a consulta **sem as colunas largas/BLOB**, avisa quantas foram
    omitidas e reescreve o SQL da aba com o `SELECT` seguro.
- **Fase 3 — Passo 1: fundação do provedor de IA**:
  - Plano da fase em `docs/plans/2026-10-16-fase3-ia-contextual.md` (4 passos).
  - Domínio `firebird_domain::ai`: `TipoProvedorIa`, `ProvedorIaConfig` (detecta
    provedor local), `PedidoIa`, `RespostaIa`, `ErroIa` (Debug redigido).
  - Storage: repositório da tabela `ai_provider_profiles` (Spec 09) com
    `secret_ref` — nenhuma chave em coluna/Log/DTO.
  - Aplicação: `TransporteHttp` injetável, `ProvedorOpenAiCompat`
    (Ollama/LM Studio/OpenAI), `TransporteReqwest` (rustls, sem proxy herdado) e
    `IaService` que resolve a chave no cofre, preserva a chave na edição e apaga
    segredo órfão junto do perfil.
  - Comandos `ia_provider_list`, `ia_provider_save`, `ia_provider_test`,
    `ia_provider_delete` (DTO sem chave; apenas `hasApiKey`).
  - Consentimento remoto (`remote_context_confirmation`) bloqueia o envio de
    contexto a provedor não-local.
  - UI `features/ai-assistant` (aba "IA"): listar, criar, editar, testar e remover
    provedor, com pt-BR e estados.
- **Fase 3 — Passo 2: contexto do schema e templates versionados**:
  - `ContextoBuilder` monta o contexto só de metadados (relações, colunas, tipos,
    nullability e PK), com tetos de relações/colunas/caracteres e sinalização de
    `truncado` — nenhum valor de dado entra no prompt.
  - Templates versionados (`VERSAO_TEMPLATES = 1`) para gerar/explicar/corrigir/revisar,
    com montagem de prompt (`montar_prompt`) e regra fixa de segurança.
  - Comandos `ia_context_preview` (devolve o texto **sem enviar nada**) e
    `ia_template_list`; UI na aba IA com perfil conectado, pré-visualização do
    contexto e lista de templates com versão.
- **Fase 3 — Passo 3: geração/explicação/correção com histórico**:
  - Storage: repositório de `ai_interactions` (prompt, resumo do contexto, resposta,
    `accepted_sql`) com `Debug` redigido.
  - `IaService::gerar` monta contexto → prompt versionado → chave lida do cofre →
    chamada ao provedor → registro no histórico; `historico` e `marcar_sql_aceito`
    para auditoria. Impressão digital do contexto (FNV-1a) sem guardar o esquema.
  - Comandos `ia_generate`, `ia_interactions_list`, `ia_interaction_accept_sql`.
  - UI: pedido (provedor + tarefa + texto), resposta com metadados do envio,
    botão **Inserir SQL sugerido no editor** (vai para a aba SQL ativa, **sem
    executar**) e histórico do assistente.
- **Fase 3 — Passo 4: revisão de SQL, risco e consentimento**:
  - `firebird_domain::risk`: `NivelRisco` (leitura → transação → escrita →
    estrutural → destrutivo), `avaliar_sql` removendo comentários/literais antes de
    classificar, detecção de escrita escondida em CTE e de múltiplas instruções.
    Desconhecido cai sempre em **destrutivo** (conservador).
  - Comando `sql_risk_assessment` (local, sem rede e sem IA).
  - Editor: antes de executar, mostra o risco; escrita/estrutura/destrutivo abrem
    diálogo de confirmação com os motivos. `SELECT` passa direto.
  - Assistente: aceitar uma sugestão com risco exige confirmação explícita, e o
    SQL só é inserido no editor — **nunca executado**.
- **Fase 4 — Estabilização e distribuição**:
  - Pacote `.deb` como entrega principal (`bundle.active`, alvo `deb`, dependências
    `libwebkit2gtk-4.1-0`, `libgtk-3-0`, `libssl3`, `ca-certificates`; recomenda
    `libfbclient2`); janela padrão 1440×900.
  - AppImage gerado (`targets: ["deb", "appimage"]`, ~86 MiB, WebKitGTK embutido)
    com conjunto de ícones PNG em RGBA (32/64/128/256/512) — o `appimagetool`
    exige ícone quadrado e o Tauri exige RGBA.
  - Guia rápido do AppImage em `docs/instalacao.md` (executar, `--appimage-extract`,
    FUSE ausente).
  - Release `v0.1.0` publicada com o `.deb` e arquivo `.sha256`.
  - Guia de instalação em `docs/instalacao.md` (pacote, código-fonte, biblioteca
    cliente, primeiro uso, IA, diagnóstico).
  - Matriz de compatibilidade publicada em `docs/compatibility/MATRIX.md`.
  - SBOM CycloneDX 1.5 gerado por `scripts/gen_sbom.py` (613 componentes a partir de
    `Cargo.lock` + `pnpm-lock.yaml`, sem rede).
  - `SECURITY.md` com séries suportadas, superfície do aplicativo e verificação do SBOM.
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