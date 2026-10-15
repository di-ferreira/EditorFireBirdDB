# Índice das Especificações — Firebird Studio Linux

Especificação técnica quebrada por tema, extraída de
[ESPECIFICACAO_TECNICA_FIREBIRD_STUDIO_LINUX.md](../original/ESPECIFICACAO_TECNICA_FIREBIRD_STUDIO_LINUX.md)
(v0.1, 14/10/2026). Cada documento é a fonte de verdade para a área correspondente.

| # | Spec | Conteúdo | Seção de origem |
|---|---|---|---|
| 00 | [Visão geral](00-visao-geral.md) | Resumo executivo, decisões confirmadas, objetivos, fora de escopo | 1–3 |
| 01 | [Compatibilidade Firebird](01-compatibilidade-firebird.md) | Matriz de versões, libfbclient, dialeto/charset | 4 |
| 02 | [Arquitetura](02-arquitetura.md) | Camadas, diagrama, stack | 5 |
| 03 | [Gerenciador de conexões](03-gerenciador-conexoes.md) | Perfis, sessão, status (FR-001/002) | 6.1 |
| 04 | [DB Explorer](04-db-explorer.md) | Árvore de objetos, catálogo (FR-003/004) | 6.2 |
| 05 | [SQL Editor](05-sql-editor.md) | Abas, execução, parser de delimitadores (FR-005/007) | 6.3 + 10 |
| 06 | [Grade de resultados](06-grade-resultados.md) | Paginação, tipos, transação (FR-006/014) | 6.4 |
| 07 | [Assistente de IA](07-assistente-ia.md) | Comandos, contexto, privacidade (FR-008–013) | 6.5 |
| 08 | [Arquitetura da IA](08-arquitetura-ia.md) | Componentes, provedores, prompts, segurança | 7 |
| 09 | [Modelo de dados local](09-modelo-dados-local.md) | Schema SQLite, migrações | 8 |
| 10 | [Contratos internos](10-contratos-internos.md) | DTOs, comandos Tauri, erros tipados | 9 |
| 11 | [Requisitos não funcionais](11-requisitos-nao-funcionais.md) | Segurança, desempenho, A11y, confiabilidade | 11 |
| 12 | [Testes e TDD](12-testes-tdd.md) | Pirâmide, matriz contínua, DoD | 12 |
| 13 | [Critérios de aceite do MVP](13-criterios-aceite-mvp.md) | Matriz FR-xxx, 12 cenários de aceite | 14–15 |
| 14 | [Plano de entrega](14-plano-entrega.md) | Fases 0–5, spike recomendado | 16 + 20 |
| 15 | [Governança](15-governanca.md) | Licença Apache-2.0, arquivos obrigatórios | 17 |
| 16 | [Riscos e decisões](16-riscos-decisoes.md) | Tabela de riscos, ADRs, fontes | 18–19 |

## Estrutura de referência cruzada

- Requisitos rastreáveis (FR-xxx) e prioridades P0/P1: [13-criterios-aceite-mvp.md](13-criterios-aceite-mvp.md)
- ADRs: [`docs/adr/`](../adr/)
- Documento original preservado: [`docs/original/`](../original/)
