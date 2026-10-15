# Third-Party Notices

Notificações de licenças de componentes de terceiros usados pelo **Firebird Studio Linux**.

> **Status:** documento inicial (Fase 0). A lista completa de dependências e suas
> licenças será consolidada após a prova de conceito (Fase 0) e antes da primeira
> publicação, acompanhando o SBOM por release (ver [ADR-006](docs/adr/ADR-006-licenca-redistribuicao.md)).

## Componentes previstos (a confirmar)

| Componente | Uso | Licença (prevista) | Observação |
|---|---|---|---|
| Tauri 2 | Shell desktop | Apache-2.0 / MIT | A confirmar na versão pinada |
| React / Vite / TypeScript | Frontend | MIT | A confirmar |
| CodeMirror 6 | Editor SQL | MIT | A confirmar |
| TanStack Table | Grades de resultados | MIT | A confirmar |
| SQLite (crate Rust) | Persistência local | Domínio público | A confirmar |
| keyring / libsecret | Cofre de segredos | A confirmar | |
| Cliente Firebird (`libfbclient.so`) | Acesso ao banco | IPL/CDDL-1.0 | Vinculação dinâmica; política de redistribuição pendente (ADR-006) |

## Pendências antes do primeiro release

- Auditoria completa de licenças via `cargo deny` / `pnpm license` e consolidação no SBOM.
- Definição da política de redistribuição das bibliotecas Firebird nos pacotes `.deb`.
