# ADR-003: Persistência SQLite e keyring do sistema

**Status:** Decidida — implementada na Fase 1 (`crates/firebird-storage`, `crates/firebird-security`)
**Data:** 14/10/2026
**Contexto:** [Spec 09 — Modelo de dados local](../specs/09-modelo-dados-local.md)

## Contexto

A aplicação precisa armazenar preferências, perfis de conexão, histórico SQL, scripts
salvos e histórico de IA localmente, sem servidor e sem armazenar credenciais em texto
puro.

## Decisão

- **SQLite** (via crate Rust) para todos os metadados locais, com schema versionado por
  `schema_migrations` e migrações transacionais/idempotentes.
- **Cofre de segredos do sistema** (Secret Service/libsecret ou `keyring` multiplataforma)
  para senhas e API keys; o SQLite guarda apenas `secret_ref`.

## Consequências

- Nenhuma senha em coluna SQLite nem em logs de migração.
- Histórico de SQL/IA pode ser desativado ou limpo pelo usuário.
- Cópia de segurança do banco local antes de migrações destrutivas.
- O SQLite não substitui nem copia o banco Firebird — apenas metadados da aplicação.

## Alternativas consideradas

- SQLite com coluna de senha cifrada: concentra risco da chave de cifra no mesmo arquivo.
- Arquivos JSON: sem transacionalidade nem migrações estruturadas.
