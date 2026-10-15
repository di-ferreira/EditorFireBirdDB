# Contributing — Firebird Studio Linux

Obrigado por contribuir! Este projeto é uma IDE desktop para Firebird (Tauri 2 +
React + Rust) e segue engenharia guiada por especificação e TDD.

## Começando

1. Leia o [README](README.md) e o [índice das specs](docs/specs/README.md).
2. Pré-requisitos: Rust estável, Node.js ≥ 20, pnpm e dependências nativas do Tauri 2
   no Linux (WebKitGTK).
3. `pnpm install` no repositório raiz (workspace pnpm).

## Regras de contribuição

### TDD obrigatório (domínio e serviços)

- Escreva o teste **antes** da implementação em `domain/` e `application/`
  ([Spec 12](docs/specs/12-testes-tdd.md)).
- Só considere a tarefa concluída com a Definition of Done completa (testes, critérios de
  aceite, docs, revisão, estados de UI).

### Spec é a fonte de verdade

- Toda funcionalidade rastreia um requisito FR-xxx
  ([matriz](docs/specs/13-criterios-aceite-mvp.md)).
- Mudança de comportamento atualiza a spec correspondente; decisão significativa vira
  ADR em `docs/adr/` (fale com o arquiteto/crie a ADR no PR).

### Código

- PRs pequenos, um tema lógico por PR, revisão obrigatória.
- Formatação e lint no CI: `cargo fmt`, `cargo clippy`, `pnpm lint`, `pnpm test`.
- Commits em [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`,
  `docs:`, `test:`, `chore:...`).

### Segurança e dados

- **Nunca** commite segredos, senhas, chaves, bancos reais ou dumps de clientes.
- Fixtures apenas sintéticas (ex.: esquema `CLIENTES`/`PEDIDOS` fictício).
- Nenhuma credencial em logs, erros ou DTOs (FR-016).
- Reporte vulnerabilidades conforme [SECURITY.md](SECURITY.md) — não abra issue pública.

## Estrutura do repositório

```text
apps/desktop/src/         frontend React + TypeScript
apps/desktop/src-tauri/   backend Rust (domain → application → adapters)
docs/specs/               especificação por tema (fonte de verdade)
docs/adr/                 decisões de arquitetura
tests/                    integração Firebird por versão, E2E, fixtures
.opencode/agent/          agents de desenvolvimento do projeto
```

## Pull requests

1. Crie um branch temático (`feat/`, `fix/`, `docs/`).
2. Inclua testes e atualize specs/docs/CHANGELOG quando aplicável.
3. Descreva o cenário testado e o resultado (`cargo test`, `pnpm test`).
4. Confirme que nenhum dado sensível entrou no diff.

## Código de conduta

Ao contribuir, você concorda com o [Código de Conduta](CODE_OF_CONDUCT.md).
