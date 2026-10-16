export type ProvedorIaDto = {
  id: string;
  name: string;
  providerType: string;
  baseUrl: string;
  modelName: string;
  /** `true` quando há chave guardada no cofre (a chave nunca vem para a UI). */
  hasApiKey: boolean;
  timeoutMs: number;
  remoteContextConfirmation: boolean;
};
