import { invoke } from "@tauri-apps/api/core";
import { normalizarErro } from "../connections/api";
import type { ProvedorIaDto } from "./types";

export async function listarProvedores(): Promise<ProvedorIaDto[]> {
  try { return await invoke<ProvedorIaDto[]>("ia_provider_list", {}); }
  catch (e) { throw normalizarErro(e); }
}

export async function salvarProvedor(dados: {
  id?: string;
  nome: string;
  tipo: string;
  baseUrl: string;
  modelo: string;
  apiKey?: string;
  timeoutMs?: number;
  consentirContextoRemoto?: boolean;
}): Promise<ProvedorIaDto> {
  try {
    return await invoke<ProvedorIaDto>("ia_provider_save", {
      id: dados.id ?? null,
      name: dados.nome,
      providerType: dados.tipo,
      baseUrl: dados.baseUrl,
      modelName: dados.modelo,
      apiKey: dados.apiKey ?? null,
      timeoutMs: dados.timeoutMs ?? 30000,
      remoteContextConfirmation: dados.consentirContextoRemoto ?? false,
    });
  } catch (e) { throw normalizarErro(e); }
}

export async function testarProvedor(id: string): Promise<string> {
  try { return await invoke<string>("ia_provider_test", { id }); }
  catch (e) { throw normalizarErro(e); }
}

export async function apagarProvedor(id: string): Promise<boolean> {
  try { return await invoke<boolean>("ia_provider_delete", { id }); }
  catch (e) { throw normalizarErro(e); }
}

export { normalizarErro };
