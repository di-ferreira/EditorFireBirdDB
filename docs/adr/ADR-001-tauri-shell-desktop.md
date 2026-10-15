# ADR-001: Tauri 2 como shell desktop

**Status:** Proposta
**Data:** 14/10/2026
**Contexto:** [Spec 02 — Arquitetura](../specs/02-arquitetura.md)

## Contexto

O produto é uma IDE desktop para Linux com UI web moderna e backend nativo com acesso a
bibliotecas C (`libfbclient.so`), SQLite e cofre de segredos. Alternativas avaliadas:
Electron, Qt/Slint nativo e Tauri 2.

## Decisão

Adotar **Tauri 2** como shell desktop, com React + TypeScript no frontend e Rust no
backend.

## Consequências

- Binário nativo menor e melhor integração com ecosistema Rust.
- Fronteira de comandos IPC explícita reforça a segurança (escopos mínimos de permissão).
- Dependência de WebKitGTK varia entre distribuições — risco mapeado na
  [Spec 16](../specs/16-riscos-decisoes.md).
- Curva de aprendizado nas capabilities/permissions do Tauri 2.

## Alternativas consideradas

- Electron: maduro, porém peso de memória e Node.js no backend.
- Qt nativo: excelente integração, mas UI mais cara de desenvolver para o escopo.
