# ADR-005: Execução de SQL, transações, cancelamento e políticas de escrita

**Status:** Parcialmente implementada (Fase 1: leitura/escrita separadas via `query_rows`/`execute`; cancelamento e confirmação de escrita chegam na Fase 2)
**Data:** 14/10/2026
**Contexto:** [Spec 05 — SQL Editor](../specs/05-sql-editor.md),
[Spec 06 — Grade de resultados](../specs/06-grade-resultados.md)

## Contexto

Executar SQL de forma segura e previsível exige definir como instruções são separadas,
quando a escrita é permitida e o que acontece em falha.

## Decisão

- Execução de **uma instrução preparada por vez** ou do texto selecionado no MVP.
- Separação de scripts multi-instrução apenas com parser consciente de delimitadores e
  blocos PSQL; **proibido** split ingênuo por `;`.
- Leituras em transação/read-only quando o adaptador permitir; escritas exigem modo
  explícito e confirmação adicional.
- `SELECT` não é presumido sem efeitos — política conservadora para funções externas,
  procedures e construções especiais.
- Nenhum SQL gerado pela IA é executado automaticamente; `COMMIT`/`ROLLBACK`/DDL/DML da IA
  nunca rodam sem ação humana.
- Cancelamento de consultas longas quando o adaptador suportar cancelamento confiável.
- Falha de operação dispara rollback quando aplicável (coberto por teste de integração).

## Consequências

- Suporte completo a `SET TERM` pode ficar fora do MVP; limitação informada ao usuário.
- Classificação de risco do SQL é barreira auxiliar, não sandbox (Spec 08, §5).
- Resultados usam paginação/lotes (500 linhas por lote por padrão).
