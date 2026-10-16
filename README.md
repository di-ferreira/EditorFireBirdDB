# Firebird Studio Linux

**IDE desktop open source para Firebird com assistência de IA contextual.**

[![Licença](https://img.shields.io/badge/licença-Apache--2.0-blue.svg)](LICENSE)
[![Plataforma](https://img.shields.io/badge/plataforma-Linux%20x86__64-orange.svg)](docs/specs/01-compatibilidade-firebird.md)
[![Firebird](https://img.shields.io/badge/Firebird-2.0%20%E2%80%93%205.0-green.svg)](docs/compatibility/)

> **Status:** Fase 3 (IA contextual) — passos 1 a 4 implementados; Fase 2 (DB Explorer + SQL Editor) concluída e validada contra um banco
> Firebird real (2.5.9): CI, persistência SQLite, cofre de segredos, gerenciador de
> conexões, catálogo sob demanda, editor SQL com CodeMirror, execução de consultas com
> grade rolável, histórico e scripts salvos. Firebird **2.5.9, 3.0.10, 4.0.0 e 5.0.0**
> foram validados no checklist PoC (6/6 cada); **2.0/2.1 seguem não testados** — nenhuma
> versão é declarada "suportada" sem evidência de teste reproduzível.

## Instalação

```sh
sudo apt install "./Firebird.Studio.Linux_0.1.0_amd64.deb"
```

Guia completo (biblioteca cliente, conexão, IA, diagnóstico):
**[`docs/instalacao.md`](docs/instalacao.md)** ·
Matriz de compatibilidade: **[`docs/compatibility/MATRIX.md`](docs/compatibility/MATRIX.md)** ·
SBOM: **`docs/security/sbom.cdx.json`** · Segurança: **[`SECURITY.md`](SECURITY.md)**

## Proposta

O Firebird Studio Linux é uma aplicação desktop para conectar e administrar bancos
Firebird, escrever e executar SQL, explorar metadados e gerar consultas com IA que
conhece o esquema real do banco ativo. O produto atende usuários legados de Firebird
2.x e ambientes modernos 3.0–5.0 dentro de uma **matriz de compatibilidade publicada e
testada**, com suporte inicial a Linux Mint/Ubuntu x86_64.

### Capacidades do MVP

| Módulo | Descrição | Spec |
|---|---|---|
| 🔌 Gerenciador de conexões | Perfis TCP/IP, teste de conexão, biblioteca cliente configurável por perfil, senhas no cofre do sistema | [Spec 03](docs/specs/03-gerenciador-conexoes.md) |
| 🌳 DB Explorer | Tabelas, views, campos, PK/FK/índices, domínios, procedures, triggers e generators — expansão sob demanda | [Spec 04](docs/specs/04-db-explorer.md) |
| 📝 SQL Editor | Abas múltiplas, realce Firebird SQL/PSQL, execução de instrução/seleção, histórico local | [Spec 05](docs/specs/05-sql-editor.md) |
| 📊 Grade de resultados | Paginação por lotes, `NULL` distinguível, tipos completos, exportação CSV | [Spec 06](docs/specs/06-grade-resultados.md) |
| 🤖 Assistente de IA | Geração/explicação/correção de SQL com contexto de metadados; provedores locais (OpenAI-compatible) e remotos | [Spec 07](docs/specs/07-assistente-ia.md) |

### Princípios inegociáveis

- **Nenhum SQL gerado pela IA é executado automaticamente** — sempre revisão e ação
  explícita do usuário.
- **Segredos nunca em texto puro**: senhas e API keys vivem no cofre do sistema operacional.
- **Privacidade por padrão**: sem telemetria; o contexto enviado a provedores remotos é
  visível e confirmável; nada de registros de negócio nos prompts por padrão.
- **Compatibilidade comprovada, não presumida**: cada versão declarada passa por testes
  de integração.

## Arquitetura

Interface **React + TypeScript** (Vite, CodeMirror 6, TanStack Table) falha com o
backend **Rust** através da fronteira de comandos **Tauri 2**. Serviços de aplicação
orquestram sessões, catálogo, execução e IA; o domínio é puro e testável sem banco real;
o acesso ao Firebird fica isolado atrás de um trait `FirebirdAdapter` (biblioteca
`libfbclient.so` carregada de forma controlada, decisão pendente na Fase 0 —
[ADR-002](docs/adr/ADR-002-driver-firebird.md)). Preferências e histórico ficam em
**SQLite** local.

```
apps/desktop/src/         → React + TypeScript (features: connections, db-explorer,
                            sql-editor, query-results, ai-assistant, settings)
apps/desktop/src-tauri/   → Rust (domain, application, firebird, metadata, query,
                            ai, storage, security, logging, commands)
crates/firebird-storage/  → SQLite local (perfis, histórico, scripts) + migrações
crates/firebird-security/ → cofre de segredos do sistema (Secret Service)
docs/specs/               → especificação técnica quebrada por tema (fonte de verdade)
docs/adr/                 → decisões de arquitetura
tests/                    → integração Firebird por versão, E2E e fixtures sintéticas
```

Diagrama completo e stack: [Spec 02 — Arquitetura](docs/specs/02-arquitetura.md).

## Documentação

| Documento | Conteúdo |
|---|---|
| [Índice das specs](docs/specs/README.md) | Especificação técnica completa por tema |
| [Documento original](docs/original/ESPECIFICACAO_TECNICA_FIREBIRD_STUDIO_LINUX.md) | Especificação monolítica v0.1 |
| [Plano de entrega](docs/specs/14-plano-entrega.md) | Fases 0–5 |
| [Critérios de aceite do MVP](docs/specs/13-criterios-aceite-mvp.md) | Requisitos FR-xxx e cenários de aceite |
| [Riscos e ADRs](docs/specs/16-riscos-decisoes.md) | Decisões técnicas registradas |
| [CONTRIBUTING](CONTRIBUTING.md) | Como contribuir (TDD obrigatório no domínio) |
| [SECURITY](SECURITY.md) | Política de reporte de vulnerabilidades |

## Desenvolvimento

Pré-requisitos (Fase 0/1): Rust (estável), Node.js ≥ 20, pnpm e as dependências nativas
do Tauri 2 para Linux (WebKitGTK). Comandos previstos — a serem definidos na Fase 0/1:

```bash
pnpm install        # dependências JS (workspace)
pnpm test           # testes de UI (Vitest)
cargo test          # testes Rust (domínio, serviços, integração)
```

O projeto segue **TDD** em domínio e serviços: o teste vem antes da implementação
([Spec 12 — Testes e TDD](docs/specs/12-testes-tdd.md)).

## Compatibilidade

Suportar "Firebird 2.0+" significa que a versão passou por testes de integração na
matriz publicada — não que todos os recursos existam em todas as versões. Séries 2.0/2.1/2.5
são oficiais EOL; o produto sinaliza versões legadas e não afirma que estejam seguras.
Matriz e relatórios: [`docs/compatibility/`](docs/compatibility/).

## Licença

[Apache-2.0](LICENSE). Antes da primeira publicação, as licenças de todas as dependências
e a redistribuição das bibliotecas Firebird serão revisadas
([ADR-006](docs/adr/ADR-006-licenca-redistribuicao.md), [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES.md)).
