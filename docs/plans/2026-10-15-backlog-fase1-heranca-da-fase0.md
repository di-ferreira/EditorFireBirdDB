# Backlog da Fase 1 — herança da Fase 0 (prova de conceito)

**Origem:** revisão final do plano `docs/plans/2026-10-15-fase0-prova-de-conceito.md` (6 tasks,
todas com revisão aprovada) + triagem dos achados adiados.
**Data:** 15/10/2026 · **Estado:** nada aqui é bloqueante para o merge da Fase 0; tudo é dívida
conhecida e assumida.

## 1. Decisões (rulings) tomadas durante a Fase 0

| Decisão | Motivo | Custo se estiver errado |
|---|---|---|
| Trabalho direto em `main` (o repo **tem** remoto `origin`) | O usuário dirigiu o desenvolvimento neste diretório; worktree/branch não traria ganho | Commits da Fase 0 já publicados em `origin/main` |
| `execute()` **ratificada** na trait `FirebirdAdapter` (fora do escopo original do ruling) | Spec 06 §2 e ADR-005 exigem distinção leitura/escrita; os serviços da Fase 1 consomem `&mut dyn FirebirdAdapter` | Revisão da superfície pública da trait na Fase 1 |
| `query_rows_params` adicionado à trait | Item 5 do checklist PoC (SELECT com `?`) | — |
| Escape `'` → `''` em literais de catálogo (não `"` → `""`) | Correção de literal string Firebird; `\"` corromperia o valor e não escapar `'` seria vetor de injeção | — |
| Filtro de sistema com `(RDB$SYSTEM_FLAG = 0 OR ... IS NULL)` | `IS NOT DISTINCT FROM` só existe no 5.0+ | — |
| `supports_relation_type` com gate `>= (2,1)` | Conservador; reabrir quando houver evidência de 2.0/2.1 | Ajustar 1 linha quando houver ambiente |
| `.agents/` removido do versionamento por commit forward | Skills de terceiros não pertencem ao produto | **Histórico preserva as 218 skills**; purgar é decisão do dono |
| `domain_version` vem de `firebird_domain::VERSION` (não de `env!` do crate desktop) | Versão real, sem divergência silenciosa | — |

## 2. Lacunas de spec que a Fase 0 divulgou e que a Fase 1 deve fechar

Já estão no backlog de `docs/adr/ADR-002-driver-firebird.md` ("O que a Fase 1 precisa fechar"):

- [ ] Ambiente reproduzível de **2.0 e 2.1** (hoje: sem tag Docker; "não testado").
- [ ] **Decidir o compromisso para 2.0** (Spec 14, saída obrigatória): conexão remota, sistema suportado ou imagem compatível.
- [ ] **Avaliar a segunda estratégia de driver** (Spec 01 §3): só a `libfbclient` via `DynLoad.lib_path` foi exercitada.
- [ ] **Dialeto 1×3 e charset ≠ UTF8** por conexão (Spec 14).
- [ ] Fork/patch do `rsfbclient` para o bug de classificação de statement (libera `INSERT ... RETURNING`/`EXECUTE BLOCK`).
- [ ] Exercitar a lib **3.0.11** do host (hoje ilegível por permissão em `/opt/firebird`).
- [ ] **Prova de runtime da janela Tauri**: build de produção + E2E (`tauri dev` não aplica a CSP).

## 3. Dívidas de qualidade triadas (todos "Fase 1", nenhum bloqueante)

### Segurança / segredos
- `SECRET_KEYS` no domínio só casa `chave=` literal: `password = x`, `password: x` e valores com espaço passam. O spike tem uma segunda camada (`censurar()`); os consumidores da Fase 1 precisarão dela.
- `DomainError::Catalog` exibe `{linha:?}` cru (hoje só dados de esquema, nunca credenciais).

### Arquitetura / API
- `FakeAdapter` público e incondicional no crate de produção → mover para feature `fake`.
- `thiserror = "2"` declarado e não usado em `crates/firebird-adapter/Cargo.toml`.
- `strip_prefix("erro de driver: ")` frágil → expor `redact_message(&str)` no domínio.
- Regra de recusa de schema **duplicada** entre harness (`tests/integration-firebird/src/lib.rs`) e spike (`crates/firebird-spike/src/main.rs`):candidato: mover a função pura para `firebird-adapter`.
- `crates/firebird-spike/src/main.rs` com 1074 linhas; `executar_checklist` ~300 linhas.

### Testes
- `crates/firebird-domain/src/metadata.rs`: 2 testes de metadata são tautológicos (constroem o struct e verificam o valor inserido).
- `validar_campos_clientes` / `validar_dados_roundtrip` (definição do checklist) sem teste unitário no spike.
- RED de compilação usado na Task 1 é evidência fraca de TDD — **regra a partir da Fase 1: RED de asserção**.
- Cenário 2.0 (`RDB$VIEW_TYPE`) da `pre_checagem` sem caso próprio (a variante já é testada em `metadata.rs`).

### Operação / ergonomia
- Sem `package.json` na raiz: `pnpm build` só funciona em `apps/desktop` (agregador vem com o CI).
- Corrida de startup dos containers (`-923`): o health-wait do spike está só em script temporário → incorporar a `scripts/firebird-env.sh`.
- `firebird-env.sh` imprime a senha padrão do container no heredoc (`export FB_POC_PASSWORD=masterkey`) — credencial descartável, mas exposta no terminal do operador.
- Evidência dos logs da Fase 0 vive em `/tmp` (volátil); considerar mover para `docs/compatibility/evidence/` (censurados) ou arquivar por release.

### Correções documentais pendentes
- Comentário "disponível desde 2.0" em `crates/firebird-adapter/src/adapter.rs` (`ENGINE_VERSION` é since 2.1).
- Relatório da Task 5: "senha nunca gravada em arquivo" é impreciso (o script temporário `run-spikes.sh` tinha o valor default).

### Producto
- **Rotacionar a senha do servidor local :3050** — a credencial padrão foi aceita no PoC (ação do operador, não do repo).