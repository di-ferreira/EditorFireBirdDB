import { invoke } from "@tauri-apps/api/core";
import { normalizarErro } from "../connections/api";
import type { ApiQueryResult, SavedScriptDto, HistoryEntryDto } from "./types";

export async function executarConsulta(perfilId: string, sql: string): Promise<ApiQueryResult> {
  try {
    return await invoke<ApiQueryResult>("query_run", { profileId: perfilId, sql });
  } catch (e) { throw normalizarErro(e); }
}
export async function listarScripts(): Promise<SavedScriptDto[]> {
  try { return await invoke<SavedScriptDto[]>("script_list", {}); }
  catch (e) { throw normalizarErro(e); }
}
export async function salvarScript(nome: string, conteudo: string, perfilId?: string, id?: string): Promise<SavedScriptDto> {
  try { return await invoke<SavedScriptDto>("script_save", { id: id ?? null, name: nome, content: conteudo, profileId: perfilId ?? null }); }
  catch (e) { throw normalizarErro(e); }
}
export async function carregarScript(id: string): Promise<SavedScriptDto | null> {
  try { return await invoke<SavedScriptDto>("script_get", { id }); }
  catch (e) { throw normalizarErro(e); }
}
export async function apagarScript(id: string): Promise<boolean> {
  try { return await invoke<boolean>("script_delete", { id }); }
  catch (e) { throw normalizarErro(e); }
}
export async function listarHistorico(limite = 50): Promise<HistoryEntryDto[]> {
  try { return await invoke<HistoryEntryDto[]>("history_list", { limit: limite }); }
  catch (e) { throw normalizarErro(e); }
}
export async function limparHistorico(): Promise<number> {
  try { return await invoke<number>("history_clear", {}); }
  catch (e) { throw normalizarErro(e); }
}
export { normalizarErro };
