/**
 * DTOs da fronteira de conexões — espelham a Spec 10 (`10-contratos-internos.md`)
 * e os comandos Tauri implementados no backend.
 *
 * Regra inviolável: **nenhum tipo deste arquivo carrega senha persistida**.
 * A senha viaja apenas dentro de {@link OpenConnectionRequest}, cujo único uso
 * é o comando `connection_open` (que a encaminha ao cofre de segredos no
 * backend, conforme Spec 03).
 */

/** Dialeto SQL suportado pelo Firebird (1 = legado, 3 = Firebird 3+). */
export type SqlDialect = 1 | 3;

/**
 * Situação de uma sessão, separada do perfil salvo (Spec 03 §1).
 * `error` cobre falha de conexão ou de reconexão.
 */
export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

/** Códigos de erro serializados pelos comandos Tauri (Spec 10 §3). */
export type CommandErrorCode =
  | "ConnectionError"
  | "UnsupportedServerFeature"
  | "QueryError"
  | "PermissionDenied"
  | "ProviderError"
  | "Unknown";

/** Erro de comando serializado como `{ code, message }`. */
export type CommandError = {
  code: CommandErrorCode;
  message: string;
};

/**
 * Perfil de conexão salvo. Não possui — e nunca deve receber — campo de
 * senha: o segredo vive no cofre do sistema operacional (Spec 03 §1, Spec 11 §1).
 */
export type ConnectionProfile = {
  id: string;
  name: string;
  host: string;
  port: number;
  databasePath: string;
  username: string;
  roleName?: string;
  charset: string;
  sqlDialect?: SqlDialect;
  clientLibraryPath?: string;
  connectTimeoutMs?: number;
};

/**
 * Dados de um perfil na hora de criar/editar. `id` ausente = criação;
 * presente = edição do perfil existente.
 */
export type ConnectionProfileInput = Omit<ConnectionProfile, "id"> & {
  id?: string;
};

/** Situação da sessão de um perfil, devolvida por `connection_get_status`. */
export type ConnectionStatusInfo = {
  profileId: string;
  status: ConnectionStatus;
  sessionId?: string;
  /** Caminho da biblioteca cliente efetivamente carregada (Spec 03 §2). */
  clientLibraryPath?: string;
  /** Detalhe legível do erro, quando `status === 'error'`. Nunca contém segredo. */
  message?: string;
};

/** Retorno de `connection_test_profile` (teste não registra o perfil). */
export type TestConnectionResult = {
  ok: boolean;
  clientLibraryPath?: string;
  serverVersion?: string;
  message?: string;
};

/** Retorno de `connection_open`. */
export type OpenConnectionResult = {
  profileId: string;
  sessionId: string;
  status: ConnectionStatus;
  clientLibraryPath?: string;
};

/** Retorno de `connection_close`. */
export type CloseConnectionResult = {
  profileId: string;
  status: ConnectionStatus;
};

/**
 * Argumentos de `connection_open`. A senha, quando informada, é usada
 * exclusivamente por este comando — o backend a guarda no cofre.
 */
export type OpenConnectionRequest = {
  profileId: string;
  /** Opcional; nunca devolvido, nunca salvo pelo frontend. */
  password?: string;
};

/** Rótulos em pt-BR para {@link ConnectionStatus} (Spec 11 §3). */
export const ROTULO_STATUS: Record<ConnectionStatus, string> = {
  disconnected: "desconectado",
  connecting: "conectando",
  connected: "conectado",
  reconnecting: "reconectando",
  error: "erro",
};

/** Rótulos em pt-BR para {@link CommandErrorCode}. */
export const ROTULO_CODIGO_ERRO: Record<CommandErrorCode, string> = {
  ConnectionError: "Erro de conexão",
  UnsupportedServerFeature: "Recurso não suportado pelo servidor",
  QueryError: "Erro na consulta",
  PermissionDenied: "Permissão negada",
  ProviderError: "Erro no provedor",
  Unknown: "Erro desconhecido",
};

/** Verifica se um valor vindo do backend tem a forma de {@link CommandError}. */
export function isCommandError(valor: unknown): valor is CommandError {
  if (typeof valor !== "object" || valor === null) return false;
  const cand = valor as { code?: unknown; message?: unknown };
  return typeof cand.code === "string" && typeof cand.message === "string";
}