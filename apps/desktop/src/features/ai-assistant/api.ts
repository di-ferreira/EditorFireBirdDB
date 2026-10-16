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

export type IaContextPreview = {
  text: string;
  relacoes: number;
  relacoesOmitidas: number;
  truncado: boolean;
};

/** Monta o contexto do schema **sem enviar nada** — só devolve o texto local. */
export async function previewContexto(profileId: string): Promise<IaContextPreview> {
  try { return await invoke<IaContextPreview>("ia_context_preview", { profileId }); }
  catch (e) { throw normalizarErro(e); }
}

export type IaTemplate = { tarefa: string; rotulo: string; versao: number; instrucao: string };

export async function listarTemplates(): Promise<IaTemplate[]> {
  try { return await invoke<IaTemplate[]>("ia_template_list", {}); }
  catch (e) { throw normalizarErro(e); }
}

export type IaGenerate = {
  texto: string;
  sql?: string | null;
  modelo: string;
  interactionId: string;
  contextoRelacoes: number;
  contextoTruncado: boolean;
  contextoRemoto: boolean;
};

export async function gerar(
  profileId: string,
  providerProfileId: string,
  tarefa: string,
  prompt: string,
): Promise<IaGenerate> {
  try {
    return await invoke<IaGenerate>("ia_generate", {
      profileId,
      providerProfileId,
      task: tarefa,
      prompt,
    });
  } catch (e) { throw normalizarErro(e); }
}

export type IaInteraction = {
  id: string;
  providerProfileId?: string | null;
  taskType: string;
  promptText: string;
  responseText: string;
  acceptedSql?: string | null;
  createdAt: string;
};

export async function listarInteracoes(limite = 20): Promise<IaInteraction[]> {
  try { return await invoke<IaInteraction[]>("ia_interactions_list", { limit: limite }); }
  catch (e) { throw normalizarErro(e); }
}

export async function aceitarSql(interactionId: string, sql: string): Promise<boolean> {
  try { return await invoke<boolean>("ia_interaction_accept_sql", { interactionId, sql }); }
  catch (e) { throw normalizarErro(e); }
}

export type SqlRisk = {
  nivel: "leitura" | "transacao" | "escrita" | "estrutural" | "destrutivo";
  rotulo: string;
  exigeConfirmacao: boolean;
  motivos: string[];
  multiplasInstrucoes: boolean;
};

/** Classificação local de risco do SQL sugerido (sem rede, sem IA). */
export async function avaliarRiscoSql(sql: string): Promise<SqlRisk> {
  try { return await invoke<SqlRisk>("sql_risk_assessment", { sql }); }
  catch (e) { throw normalizarErro(e); }
}
