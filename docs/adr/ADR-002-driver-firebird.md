# ADR-002: Driver Firebird selecionado após a prova de conceito

**Status:** Decidida (parcialmente) — Fase 0 executada em 2026-10-15; pendências da
Fase 1 listadas abaixo
**Contexto:** [Spec 01 — Compatibilidade](../specs/01-compatibilidade-firebird.md),
[Spec 14 — Plano de entrega](../specs/14-plano-entrega.md),
[PoC Fase 0 — relatório](../compatibility/POC-FASE0.md)

## Contexto

A aplicação deve suportar Firebird 2.0+ no Linux. Nenhuma biblioteca é declarada
comprovadamente compatível com 2.0 antes dos testes. Estratégias candidatas:

1. `libfbclient.so` oficial carregada dinamicamente (via `rsfbclient` em modo native).
2. Implementação/protocolo alternativo em Rust puro.
3. Duas estratégias coexistindo atrás do trait `FirebirdAdapter`.

A Fase 0 executou o spike (`crates/firebird-spike`) e o harness de integração
(`tests/integration-firebird`) contra ambientes reais. Resultados, método e limitações
completos: [docs/compatibility/POC-FASE0.md](../compatibility/POC-FASE0.md).

## Decisão

**Estratégia primária: `rsfbclient-native` 0.29 com a feature `dynamic_loading` e
`DynLoad.lib_path`**, apontando para a `libfbclient.so` escolhida pelo usuário
(descoberta + seleção manual, sem substituir bibliotecas do sistema nem criar
symlinks globais — Spec 01, §3), atrás do trait `FirebirdAdapter`.

- O modo `linking` (feature default, `-lfbclient` em tempo de link) é **descartado**:
  viola a exigência de seleção explícita da biblioteca em runtime (Spec 01, §3).
- A estratégia 2 (protocolo Rust puro) **não foi executada** nesta fase: nenhum
  candidato com suporte provado a Firebird 2.0 foi testado. Permanece apenas como
  contingência (ver Riscos).
- O adaptador permanece isolado atrás de `FirebirdAdapter` (`server_version`,
  `query_rows`, `query_rows_params`, `execute`), de modo que um fork/patch do
  `rsfbclient` ou uma implementação alternativa não vazam para o resto do sistema.

## Evidências (prova de conceito — 2026-10-15)

Checklist PoC (6 itens: conexão+versão, `RDB$RELATIONS`, `RDB$RELATION_FIELDS`,
SELECT parametrizado, Unicode/decimal/NULL, erro redigido sem senha):
**validado no checklist PoC (6/6)** nas combinações:

| Servidor | Cliente (`libfbclient`) | Resultado |
|---|---|---|
| 2.5.9 (Docker `2.5-sc`) | 2.5.9 (imagem `2.5-sc`) | 6/6 |
| 3.0.10 (Docker `3.0`) | 3.0.10 (imagem `3.0`) | 6/6 |
| 4.0.0 (Docker `4.0`) | 3.0.10 (imagem `3.0`) | 6/6 |
| 5.0.0 (Docker `v5.0`) | 3.0.10 (imagem `3.0`) | 6/6 |
| 2.5.9 local (`:3050`) | 3.0.10 (imagem `3.0`) | 6/6 |
| 2.0.x | — | **não testado — pendente de ambiente** |
| 2.1.x | — | **não testado — pendente de ambiente** |

"6/6" = validado no checklist PoC nesta data, **não** "suportado" (Spec 01, §1).
Evidências cruzadas: logs do spike (`/tmp/opencode/task5-logs/`) e do harness
(`/tmp/opencode/task4-logs/`), duas execuções independentes com o mesmo resultado.

## Riscos restantes

1. **Bug do `rsfbclient` 0.28/0.29 nativo**: `isc_info_sql_stmt_type` lê
   `isc_info_end` como tipo → todo statement vira `Select` → fetch pós-DDL dá
   **SIGSEGV com cliente 2.5** e `-504 (Cursor is not open)` com 3.0+. Contornado via
   `FirebirdAdapter::execute` (`Execute::execute`, sem fetch); **`INSERT ... RETURNING`
   e `EXECUTE BLOCK` com saída continuam bloqueados**. Não há release corrigido
   (0.29 é a última versão) — a Fase 1 precisa de fork/patch.
2. **`libfbclient.so` 3.0.11 do host nunca foi exercitada** (cópias ilegíveis para o
   usuário comum; os testes usaram 3.0.10 extraída das imagens).
3. **Firebird 2.0 e 2.1 sem ambiente reproduzível** — não testados; nenhum suporte
   pode ser declarado até haver ambiente + evidência.
4. **Bibliotecas do host ilegíveis** (`/usr/lib/libfbclient.so.*` →
   `/opt/firebird/lib`, root-only): política de descoberta/seleção precisa funcionar
   sem sudo (Fase 1/CI: extrair das imagens de teste ou instalar libs legíveis).
5. **Corrida de startup em ambiente containerizado**: conectar logo após o mapeamento
   da porta devolve `-923: connection rejected by remote interface`; é preciso aguardar
   o healthcheck `healthy` (achado da Fase 0, §5.4 do relatório).
6. **Quirk do 2.5-sc**: `RDB$GET_CONTEXT('SYSTEM','ENGINE_VERSION')` já falhou com
   `-901` em conexão simultânea (não reproduzido nos runs finais) — o fallback de
   versão por catálogo continua sendo dívida para a Fase 1.
7. **Dialeto/charset por conexão** (1 × 3; charset ≠ UTF8) não exercitados além do
   padrão UTF8 (Spec 01, §4).

## O que a Fase 1 precisa fechar

- [ ] Fork/patch do `rsfbclient-native` (ou substituto) para o bug de classificação de
      statement, liberando `INSERT ... RETURNING`/`EXECUTE BLOCK` com testes por versão.
- [ ] Ambiente reproduzível de **2.0 e 2.1** (VM/container) e execução do checklist
      nelas — sem isso, permanecem "não testadas".
- [ ] Exercitar a lib **3.0.11** (e política de `FB_POC_LIB_PATH` para CI).
- [ ] Matriz de homologação da Spec 01, §2 em CI por versão (ambientes isolados).
- [ ] Política de diagnóstico: exibir caminho + versão da biblioteca carregada e
      combinação testada no relatório do app (já exercitada na CLI do spike).

## Consequências

- O adaptador é isolado atrás do trait `FirebirdAdapter` independentemente da
  escolha — um fork do driver não altera consumidores.
- Combinações homologadas servidor/cliente serão registradas em
  `docs/compatibility/` com data e escopo ("validado no checklist ..."), nunca como
  "suportado" sem evidência.
- Se 2.0 não for viável, o problema será exposto com biblioteca/sistema exigidos, sem
  ocultar a limitação.
- Usuários e CI carregam a biblioteca explicitamente; nada é instalado ou linkado no
  sistema pelo app.

## Notas

- Nenhuma senha em log, erro ou DTO (FR-016): a CLI do spike lê a senha só de env
  `FB_PASSWORD` e redige `Debug`.
- Não criar links simbólicos globais nem substituir bibliotecas do sistema (Spec 01, §3).
