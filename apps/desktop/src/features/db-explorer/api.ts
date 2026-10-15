import { invoke } from "@tauri-apps/api/core";

import { normalizarErro } from "../connections/api";
import type {
  CatalogNodeDto,
  CatalogNodeRequestDto,
} from "./types";
import type { CommandError } from "../connections/types";
import { isCommandError } from "../connections/types";

export type { CommandError };

/**
 * Fronteira Tauri do catálogo. Única função de IO da feature.
 *
 * Sempre envia **todas** as chaves do contrato (`filter`, `includeSystem`,
 * `forceRefresh`), mesmo quando não aplicáveis — o backend/servidor de
 * contrato exige a presença delas.
 */
export async function buscarNoNoCatalogo(
  perfilId: string,
  node: CatalogNodeRequestDto,
  filtro: string | null,
  incluirSistema: boolean,
  forcarRefresh: boolean,
): Promise<CatalogNodeDto> {
  try {
    return await invoke<CatalogNodeDto>("catalog_get_node", {
      profileId: perfilId,
      node,
      filter: filtro,
      includeSystem: incluirSistema,
      forceRefresh: forcarRefresh,
    });
  } catch (erro) {
    throw normalizarErro(erro);
  }
}

// Reexporta utilitários reusados para evitar import paralelo profundo.
export { normalizarErro };
export { isCommandError };
