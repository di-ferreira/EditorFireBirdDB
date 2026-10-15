# ADR-004: Contratos de provedores de IA e APIs suportadas

**Status:** Proposta
**Data:** 14/10/2026
**Contexto:** [Spec 08 — Arquitetura da IA](../specs/08-arquitetura-ia.md)

## Contexto

O assistente de IA deve funcionar com provedor local (ex.: servidor local) e APIs
remotas, sem prender o produto a um único vendor e sem enviar tokens à interface.

## Decisão

- Primeira integração: endpoint **OpenAI-compatible** (Chat Completions) para os dois
  modos (local e remoto), cobrindo a maioria dos servidores locais e APIs externas.
- Interface `AIProvider` no backend Rust, exposta ao frontend apenas via DTOs
  serializáveis (o contrato TS da spec é conceitual).
- Chaves API guardadas no cofre do sistema; URL base, modelo e timeout separados do
  armazenamento de chaves.
- Histórico de interações local, desativável, sem credenciais.

## Consequências

- Novos provedores entram por novo adaptador implementando `AIProvider`.
- Provedores que não suportam Chat Completions exigirão adaptador dedicado (futuro).
- Envio remoto exige consentimento explícito e resumo do contexto visível ao usuário.

## Alternativas consideradas

- Adapter por vendor (OpenAI, Anthropic, etc.): mais fidelidade, mais manutenção inicial.
- Protocolo MCP/funcionalidades novas: avaliar após o MVP.
