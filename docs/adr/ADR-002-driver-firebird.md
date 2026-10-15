# ADR-002: Driver Firebird selecionado após a prova de conceito

**Status:** Pendente — decisão na Fase 0
**Contexto:** [Spec 01 — Compatibilidade](../specs/01-compatibilidade-firebird.md),
[Spec 14 — Plano de entrega](../specs/14-plano-entrega.md)

## Contexto

A aplicação deve suportar Firebird 2.0+ no Linux. Nenhuma biblioteca é declarada
comprovadamente compatível com 2.0 antes dos testes. Estratégias candidatas:

1. `libfbclient.so` oficial carregada dinamicamente (via `rsfbclient` em modo native).
2. Implementação/protocolo alternativo em Rust puro.
3. Duas estratégias coexistindo atrás do trait `FirebirdAdapter`.

## Decisão (pendente)

Realizar spike com Rust que carregue a biblioteca, conecte a Firebird 2.0, 2.1, 2.5 e
uma versão moderna, leia metadados (`RDB$RELATIONS`, `RDB$RELATION_FIELDS`) e execute
consulta parametrizada. A escolha do driver sai do relatório do spike.

## Consequências esperadas

- O adaptador será isolado atrás do trait `FirebirdAdapter` independentemente da escolha.
- Combinações homologadas servidor/cliente serão registradas em `docs/compatibility/`.
- Se 2.0 não for viável, o problema será exposto com biblioteca/sistema exigidos, sem
  ocultar a limitação.

## Notas

- Não criar links simbólicos globais nem substituir bibliotecas do sistema (Spec 01, §3).
