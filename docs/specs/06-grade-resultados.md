# Spec 06 — Grade de Resultados

**Documento de origem:** Seção 6.4 da especificação técnica
**Requisitos:** FR-006 (Resultados, P0), FR-014 (Exportar resultados, P1)

## 1. Funcionalidades

- Exibir colunas com nome, tipo quando disponível e valores nulos distinguíveis.
- Renderizar resultados por páginas/lotes; não armazenar todos os registros em memória
  sem limite.
- Permitir copiar célula/linha/seleção e exportar o conjunto efetivamente retornado.
- Tratar datas, horários, precisão decimal, booleanos, Unicode, BLOBs e tipos sem
  renderização direta.
- Para BLOBs, mostrar um preview limitado e permitir abertura em visualizador apropriado
  em fase posterior.
- Registrar truncamento de resultado e quantidade máxima aplicada.
- Permitir cancelar consultas de longa duração se o protocolo/adaptador suportar
  cancelamento confiável.

## 2. Política de transação proposta

- Consultas de leitura usam transação/read-only quando o adaptador e o contexto permitirem.
- Comandos que alteram dados/estrutura exigem modo de execução explícito e confirmação
  adicional.
- Não presumir que `SELECT` seja sempre sem efeitos: funções externas, procedures e
  construções especiais exigem política conservadora.
- Não executar automaticamente `COMMIT`, `ROLLBACK`, `DROP`, DDL ou DML gerados pela IA.

## 3. Critérios de aceite

- [ ] Exibir colunas/linhas/NULL/erros/duração com paginação (FR-006).
- [ ] Exportar resultado limitado para CSV (FR-014, P1).
- [ ] Resultados distinguem `NULL` de string vazia (teste de UI).
- [ ] Lote padrão de 500 linhas, configurável (ver
      [11-requisitos-nao-funcionais.md](11-requisitos-nao-funcionais.md)).
