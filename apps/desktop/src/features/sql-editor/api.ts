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
