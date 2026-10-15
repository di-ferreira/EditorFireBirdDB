/**
 * Fronteira Tauri da feature de conexões.
 *
 * Uma função por comando, cada uma chamando `invoke` tipado. **Não há acesso
 * direto a Firebird, a sistema de arquivos ou a segredos** — a UI conversa
 * somente com o backend via comandos (Spec 02, Spec 03 §3).
 */
import { invoke } from "@tauri-apps/api/core";

import {
  isCommandError,
  type CloseConnectionResult,
  type CommandError,
  type ConnectionProfile,
  type ConnectionProfileInput,
  type ConnectionStatusInfo,
  type OpenConnectionRequest,
  type OpenConnectionResult,
  type TestConnectionResult,
} from "./types";

/**
 * Converte uma rejeição do `invoke` em {@link CommandError} estável.
 * Erros que chegam como texto (panic/erro de serialização do Tauri) viram
 * `code: "Unknown"` — a mensagem original é preservada para o usuário, sem
 * expor stack traces.
 */
export function normalizarErro(erro: unknown): CommandError {
  if (isCommandError(erro)) {
    return { code: erro.code as CommandError["code"], message: erro.message };
  }
  if (typeof erro === "string" && erro.length > 0) {
    return { code: "Unknown", message: erro };
  }
  if (erro instanceof Error && erro.message.length > 0) {
    return { code: "Unknown", message: erro.message };
  }
  return { code: "Unknown", message: "Falha inesperada ao falar com o backend." };
}

/** Lista todos os perfis de conexão salvos. */
export async function listarPerfis(): Promise<ConnectionProfile[]> {
  const perfis = await invoke<ConnectionProfile[]>("connection_list_profiles");
  return Array.isArray(perfis) ? perfis : [];
}

/**
 * Cria ou atualiza um perfil. O objeto enviado **nunca** inclui senha:
 * a persistência guarda apenas dados de conexão e uma `secret_ref`.
 */
export async function salvarPerfil(
  entrada: ConnectionProfileInput,
): Promise<ConnectionProfile> {
  const { id, ...campos } = entrada;
  return id === undefined
    ? await invoke<ConnectionProfile>("connection_save_profile", { profile: campos })
    : await invoke<ConnectionProfile>("connection_save_profile", {
        profile: { id, ...campos },
      });
}

/**
 * Testa a conectividade sem registrar o perfil automaticamente (Spec 03 §1).
 */
export async function testarPerfil(
  entrada: ConnectionProfileInput,
): Promise<TestConnectionResult> {
  const { id, ...campos } = entrada;
  const payload = id === undefined ? campos : { id, ...campos };
  return await invoke<TestConnectionResult>("connection_test_profile", {
    profile: payload,
  });
}

/**
 * Abre uma sessão. Este é o **único** comando que recebe a senha; o backend a
 * entrega ao cofre de segredos do sistema operacional.
 */
export async function abrirConexao(
  req: OpenConnectionRequest,
): Promise<OpenConnectionResult> {
  return await invoke<OpenConnectionResult>("connection_open", {
    profileId: req.profileId,
    password: req.password,
  });
}

/** Encerra a sessão ativa do perfil. */
export async function fecharConexao(
  profileId: string,
): Promise<CloseConnectionResult> {
  return await invoke<CloseConnectionResult>("connection_close", { profileId });
}

/** Consulta a situação atual da sessão de um perfil. */
export async function consultarStatus(
  profileId: string,
): Promise<ConnectionStatusInfo> {
  return await invoke<ConnectionStatusInfo>("connection_get_status", { profileId });
}