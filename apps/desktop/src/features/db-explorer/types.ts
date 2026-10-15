/**
 * DTOs da fronteira do catálogo (DB Explorer).
 */
export type CatalogCategoryDto =
  | "tabelas"
  | "views"
  | "indices"
  | "dominios"
  | "generators"
  | "procedures"
  | "triggers";

export type CatalogNodeRequestDto =
  | { kind: "raiz" }
  | { kind: "categoria"; categoria: CatalogCategoryDto }
  | { kind: "campos"; relacao: string }
  | { kind: "relacao"; relacao: string }
  | { kind: "constraints"; relacao: string }
  | { kind: "indices"; relacao: string }
  | { kind: "parametros"; procedimento: string };

export type CatalogNodeKindDto =
  | "root"
  | "category"
  | "object"
  | "field"
  | "index"
  | "constraint"
  | "parameter";

export type CatalogNodeDto = {
  nodeId: string;
  label: string;
  kind: CatalogNodeKindDto;
  category?: CatalogCategoryDto;
  detail?: string;
  isSystem: boolean;
  hasChildren: boolean;
  request?: CatalogNodeRequestDto;
  children?: CatalogNodeDto[];
};

export const ROTULO_CATEGORIA: Record<CatalogCategoryDto, string> = {
  tabelas: "Tabelas",
  views: "Views",
  indices: "Índices",
  dominios: "Domínios",
  generators: "Generators",
  procedures: "Procedures",
  triggers: "Triggers",
};

export const ROTULO_KIND: Record<CatalogNodeKindDto, string> = {
  root: "Raiz",
  category: "Categoria",
  object: "Objeto",
  field: "Campo",
  index: "Índice",
  constraint: "Restrição",
  parameter: "Parâmetro",
};
