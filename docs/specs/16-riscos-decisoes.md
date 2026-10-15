# Spec 16 — Riscos, Decisões em Aberto e Fontes

**Documento de origem:** Seções 18 e 19 da especificação técnica

## 1. Riscos e decisões em aberto

| Risco/decisão | Impacto | Mitigação/ação |
|---|---|---|
| Firebird 2.0 no Linux moderno | Pode impedir parte do suporte amplo | Prova de conceito e matriz por arquitetura/cliente |
| Driver escolhido | Pode ter APIs/types incompatíveis ou lacunas em versões antigas | Abstração `FirebirdAdapter` e teste comparativo antes de decidir |
| SQL multi-statement/PSQL | Separação de instruções é complexa por causa de blocos e delimitadores | Limitar escopo inicial e proibir split ingênuo por `;` |
| Identificadores e metadados antigos | Charset e metadados podem ter casos legados | Testes de Unicode/charset e normalizador de catálogo |
| IA alucina objeto ou sintaxe | SQL inválido ou perigoso | Contexto preciso, validação auxiliar, revisão obrigatória e sem autoexecução |
| Exposição de esquema por API remota | Pode revelar estrutura sensível | Consentimento, visualização do contexto e provedor local por opção |
| Grandes volumes de resultados | Memória e UI degradadas | Paginação, limites e virtualização |
| Distribuição Linux | WebKitGTK e bibliotecas nativas variam entre distros | `.deb` inicial, dependências documentadas e CI por distro alvo |
| EOL do Firebird 2.x | Risco operacional e de segurança no lado servidor | Avisos no produto e documentação sem prometer manutenção do servidor |

## 2. Decisões técnicas que devem virar ADR

| ADR | Tema | Status |
|---|---|---|
| [ADR-001](../adr/ADR-001-tauri-shell-desktop.md) | Tauri 2 como shell desktop | Proposta |
| [ADR-002](../adr/ADR-002-driver-firebird.md) | Driver Firebird selecionado após a prova de conceito | Pendente (Fase 0) |
| [ADR-003](../adr/ADR-003-persistencia-sqlite-keyring.md) | Persistência SQLite e keyring do sistema | Proposta |
| [ADR-004](../adr/ADR-004-contratos-ia.md) | Contratos de provedores de IA e APIs suportadas | Proposta |
| [ADR-005](../adr/ADR-005-execucao-sql-transacoes.md) | Execução de SQL, transações, cancelamento e políticas de escrita | Proposta |
| [ADR-006](../adr/ADR-006-licenca-redistribuicao.md) | Política de licença e redistribuição de bibliotecas | Proposta (licença Apache-2.0 decidida; redistribuição pendente) |

## 3. Fontes técnicas para validação

Fontes consultadas em 14/10/2026; revisar no início da implementação e a cada atualização
relevante de dependências.

1. Firebird — versões e séries descontinuadas: https://firebirdsql.org/pt/server-packages/
2. Firebird 2.0/2.1 Quick Start: https://www.firebirdsql.org/file/documentation/reference_manuals/user_manuals/Firebird-2-QuickStart.pdf
3. Firebird 2.5 Quick Start: https://www.firebirdsql.org/file/documentation/reference_manuals/user_manuals/Firebird-2.5-QuickStart.pdf
4. Firebird 2.5 Compatibility Issues: https://www.firebirdsql.org/file/documentation/release_notes/html/en/2_5/rnfb25-compat.html
5. Firebird system tables: https://www.firebirdsql.org/file/documentation/chunk/en/refdocs/fblangref25/fblangref25-appx04-systables.html
6. Tauri — arquitetura: https://tauri.app/concept/architecture/
7. Tauri — capabilities e permissões: https://tauri.app/security/capabilities/
8. Tauri SQL plugin (não inclui Firebird): https://v2.tauri.app/plugin/sql/
9. rsfbclient: https://docs.rs/rsfbclient/latest/rsfbclient/
10. CodeMirror: https://codemirror.net/docs/
