import { invoke } from "@tauri-apps/api/core";
import { normalizarErro } from "../connections/api";
import type { ApiQueryResult, SavedScriptDto, HistoryEntryDto } from "./types";

/** Executa uma instrução; sem paginação (DML/DDL ou consulta única). */
export async function executarConsulta(perfilId: string, sql: string): Promise<ApiQueryResult> {
  try {
    return await invoke<ApiQueryResult>("query_run", {
      profileId: perfilId,
      sql,
      queryId: null,
      offset: null,
      limit: null,
    });
  } catch (e) {
    throw normalizarErro(e);
  }
}

/**
 * Executa uma janela de linhas de um SELECT **paginado no servidor**.
 *
 * `queryId` identifica a execução para permitir cancelamento; o mesmo id é
 * reutilizado nas páginas seguintes da mesma consulta.
 */
export async function executarPagina(
  perfilId: string,
  sql: string,
  queryId: string,
  offset: number,
  limit: number,
): Promise<ApiQueryResult> {
  try {
    return await invoke<ApiQueryResult>("query_run", {
      profileId: perfilId,
      sql,
      queryId,
      offset,
      limit,
    });
  } catch (e) {
    throw normalizarErro(e);
  }
}

/** Pede o cancelamento cooperativo da consulta em execução. */
export async function cancelarConsulta(queryId: string): Promise<boolean> {
  try {
    return await invoke<boolean>("query_cancel", { queryId });
  } catch (e) {
    throw normalizarErro(e);
  }
}

export async function salvarScript(nome: string, conteudo: string, perfilId?: string, id?: string): Promise<SavedScriptDto> {
  try {
    return await invoke<SavedScriptDto>("script_save", { id: id ?? null, name: nome, content: conteudo, profileId: perfilId ?? null });
  } catch (e) {
    throw normalizarErro(e);
  }
}

export async function listarScripts(): Promise<SavedScriptDto[]> {
  try { return await invoke<SavedScriptDto[]>("script_list", {}); }
  catch (e) { throw normalizarErro(e); }
}

export async function listarHistorico(limite = 50): Promise<HistoryEntryDto[]> {
  try { return await invoke<HistoryEntryDto[]>("history_list", { limit: limite }); }
  catch (e) { throw normalizarErro(e); }
}

export { normalizarErro };

export type TransactionStatus = { inTransaction: boolean; message: string };

/** Inicia transação manual: os próximos DML ficam pendentes. */
export async function iniciarTransacao(perfilId: string): Promise<TransactionStatus> {
  try { return await invoke<TransactionStatus>("transaction_begin", { profileId: perfilId }); }
  catch (e) { throw normalizarErro(e); }
}

/** Confirma a transação pendente (Commit). */
export async function confirmarTransacao(perfilId: string): Promise<TransactionStatus> {
  try { return await invoke<TransactionStatus>("transaction_commit", { profileId: perfilId }); }
  catch (e) { throw normalizarErro(e); }
}

/** Desfaz a transação pendente (Rollback). */
export async function desfazerTransacao(perfilId: string): Promise<TransactionStatus> {
  try { return await invoke<TransactionStatus>("transaction_rollback", { profileId: perfilId }); }
  catch (e) { throw normalizarErro(e); }
}

/** Colunas da chave primária da tabela (vazio = sem PK → grade somente leitura). */
export async function chavesDaTabela(perfilId: string, tabela: string): Promise<string[]> {
  try { return await invoke<string[]>("table_keys", { profileId: perfilId, relation: tabela }); }
  catch (e) { throw normalizarErro(e); }
}

/** Colunas da relação com o sinal de quais estouram o limite de buffer (-204). */
export type ColunaTabela = { name: string; exceedsBuffer: boolean };

export async function colunasDaTabela(perfilId: string, tabela: string): Promise<ColunaTabela[]> {
  try { return await invoke<ColunaTabela[]>("table_columns", { profileId: perfilId, relation: tabela }); }
  catch (e) { throw normalizarErro(e); }
}
