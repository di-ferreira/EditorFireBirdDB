# Changelog

Todo formato notável deste projeto é documentado neste arquivo.
Baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e
[Semantic Versioning](https://semver.org/lang/pt-BR/).

## [Não lançado]

### Adicionado

- Estrutura inicial do repositório (monorepo pnpm + workspace Cargo Rust).
- Especificação técnica extraída por tema em `docs/specs/` (17 documentos, do documento
  original v0.1 preservado em `docs/original/`).
- Templates de ADR-001 a ADR-006 em `docs/adr/`.
- Agents de desenvolvimento do OpenCode em `.opencode/agent/` (spec-analyst, architect,
  rust-domain-dev, firebird-adapter-dev, frontend-dev, test-engineer, security-reviewer,
  docs-writer).
- Governança: LICENSE (Apache-2.0), README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY,
  THIRD_PARTY_NOTICES.
- Crates Rust da Fase 0: `firebird-domain` (domínio puro, erros redigidos FR-016),
  `firebird-adapter` (trait `FirebirdAdapter`, descoberta/validação ELF da
  `libfbclient.so`, `MetadataRepository` com variantes `RDB$` por versão, adapter
  `rsfbclient-native` em dynamic loading).
- Harness de integração por versão do Firebird (`scripts/firebird-env.sh` +
  `tests/integration-firebird`): checklist de 6 itens contra ambientes Docker
  2.5/3.0/4.0/5.0 e servidor local; sem as env vars `FB_POC_*` é ignorado e nunca
  falha o `cargo test`.
- Binário `crates/firebird-spike` (CLI da prova de conceito): executa o checklist de
  compatibilidade contra um servidor real com `libfbclient.so` escolhida (`--lib`);
  senha exclusivamente via env `FB_PASSWORD`, nunca em flag/argv/ajuda, com `Debug`
  redigido (FR-016); saída por combinação com classificação ✅/⚠️/❌.
- Relatório da prova de conceito da Fase 0 em `docs/compatibility/POC-FASE0.md`: em
  2026-10-15 o checklist PoC foi **validado (6/6)** nas combinações
  servidor 2.5.9 × cliente 2.5.9 (Docker), 3.0.10 × 3.0.10 (Docker),
  4.0.0 × 3.0.10 (Docker), 5.0.0 × 3.0.10 (Docker) e 2.5.9 local × 3.0.10 (local
  `:3050`); **2.0/2.1: não testados — pendente de ambiente**; lib do host 3.0.11 não
  exercitada (limitações e falhas reais registradas no relatório).

### Alterado

- ADR-002 (driver Firebird) → status **"Decidida (parcialmente)"**: estratégia primária
  `rsfbclient-native` 0.29 com `dynamic_loading` + `DynLoad.lib_path`; evidências da
  matriz PoC, riscos restantes (bug de classificação de statement/`RETURNING`, lib
  3.0.11 não testada, 2.0/2.1 sem ambiente, libs do host ilegíveis) e pendências da
  Fase 1 documentados no ADR.

[Não lançado]: https://github.com/firebird-studio-linux/firebird-studio-linux
