# Fase 2 — Passo 1: Catálogo básico, árvore sob demanda e refresh

**Origem:** `docs/specs/14-plano-entrega.md` (Fase 2, item 1) + `docs/specs/04-db-explorer.md`
(FR-003 / FR-004).

**Escopo deste passo:** catálogo *básico* navegável — informações do banco,
relações (tabelas/views), campos, índices, constraints (PK/UK/FK), domínios,
generators, procedures (+parâmetros) e triggers — carregado **sob demanda**,
com atualização manual de nó ou da árvore inteira.

**Fora de escopo (passos seguintes da Fase 2):** CodeMirror e abas (passo 2),
execução/paginação/grade (passo 3), histórico e scripts salvos (passo 4).
Também ficam fora: menu contextual ("copiar nome qualificado / abrir definição /
inserir no editor"), "dependências de objetos" e editor de definição — anotados
como pendências.

## Decisões

1. **Um único comando de catálogo, genérico e preguiçoso.** `catalog_get_node`
   recebe o *nó* a expandir e devolve o nó + **um nível** de filhos. Cada filho
   carrega o próprio `request`, que a UI reenvia ao expandir. Isso torna a
   expansão sob demanda estrutural (Spec 04 §2) em vez de convenção, e evita N
   comandos por categoria.
2. **SQL parametrizado** (`query_rows_params`) em toda consulta nova de
   catálogo — nome de relação/procedimento e termo de busca nunca são
   concatenados. O `escapar_literal` existente permanece para compatibilidade,
   mas código novo usa parâmetros.
3. **Ausência nunca vira afirmação.** Coluna que o servidor da versão não expõe
   vira `None`/campo vazio, com variante por versão testada (Spec 01 §4).
4. **Compatibilidade 2.0–5.0 provada, não presumida.** Cada consulta nova é
   validada contra servidores reais (2.5/3.0/4.0/5.0 via
   `scripts/firebird-env.sh`); o que não roda no mais antigo testável
   (2.5) ganha variante/gate por versão. 2.0 permanece **não testável** neste
   ambiente — a compatibilidade é preservada por construção (só construções
   documentadas como existentes desde 2.0), nunca por suposição silenciosa.
5. **Objetos de sistema visíveis, porém distinguíveis.** `includeSystem` no
   comando; cada nó leva `isSystem` e a UI marca visualmente (Spec 04 §2).
   Privilégios do usuário conectado valem automaticamente: as consultas rodam
   na sessão do usuário, sem `WITH`/elevação.
6. **Cache por sessão e apenas sem filtro.** Consultas com termo de busca
   sempre vão ao servidor. `forceRefresh` ignora o cache; `connection_close`
   descarta o cache da sessão.

## Contrato da fronteira (consumido pela UI — não renomear)

Comando: `catalog_get_node`.

```rust
#[tauri::command]
fn catalog_get_node(
    profile_id: String,
    node: CatalogNodeRequestDto,
    filter: Option<String>,
    include_system: bool,
    force_refresh: bool,
    deps: State<AppState>,
) -> Result<CatalogNodeDto, CommandError>
```

A UI envia **sempre todas as chaves** (`filter: null`, `forceRefresh: false`).

```ts
type CatalogCategoryDto =
  | "tabelas" | "views" | "indices" | "dominios"
  | "generators" | "procedures" | "triggers";

type CatalogNodeRequestDto =
  | { kind: "raiz" }
  | { kind: "categoria"; categoria: CatalogCategoryDto }
  | { kind: "campos"; relacao: string }
  | { kind: "constraints"; relacao: string }
  | { kind: "indices"; relacao: string }
  | { kind: "parametros"; procedimento: string };

type CatalogNodeKindDto =
  | "root" | "category" | "object" | "field" | "index" | "constraint" | "parameter";

type CatalogNodeDto = {
  nodeId: string;
  label: string;
  kind: CatalogNodeKindDto;
  category?: CatalogCategoryDto;
  detail?: string;              // ex.: "VARCHAR(35) · NOT NULL"
  isSystem: boolean;
  hasChildren: boolean;
  request?: CatalogNodeRequestDto; // como expandir este nó (lazy)
  children?: CatalogNodeDto[];     // só no nó consultado (um nível)
};
```

DTOs em `camelCase` (`serde(rename_all = "camelCase")`); `kind`/`categoria` em
minúsculas; opcionais omitidos quando ausentes (nunca `null`); erro no padrão
`{ code, message }` já existente. A lista de comandos registrados em
`NOMES_DOS_COMANDOS_DE_CONEXAO` passa a incluir `catalog_get_node` (trava de
contrato com a UI).

## Estrutura do nó

```
(raiz)                       detalhe: caminho do banco, servidor, ODS
├── Tabelas      ──▶ objeto (tabela)
│                     ├── Campos      ──▶ field (tipo, nulabilidade, default, domínio)
│                     ├── Índices     ──▶ index (único, FK)
│                     └── Chaves      ──▶ constraint (PK/UK/FK + tabela referenciada)
├── Views         ──▶ objeto (view) ──▶ Campos
├── Índices       ──▶ index
├── Domínios      ──▶ object (domínio + tipo base)
├── Generators    ──▶ object (generator + valor inicial)
├── Procedures    ──▶ object ──▶ Parâmetros ──▶ parameter
└── Triggers      ──▶ object (relação, tipo, ativo/inativo)
```

## Critérios de pronto

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cd apps/desktop && pnpm build
```

Mais:

- toda consulta nova de catálogo tem **teste unitário** com `FakeAdapter`
  (mapeamento de colunas, limpeza de `CHAR`, `NULL` como ausência, escape) e
  **teste de integração por versão** (env-gated, ignorado sem `FB_POC_*`);
- integração validada de verdade nesta máquina nos containers 2.5/3.0/4.0/5.0
  (saída no relatório);
- UI com estados de carregamento/vazio/erro, expansão por teclado
  (`role="tree"`, `aria-expanded`, setas ↑/↓/→/←), marcação de objeto de
  sistema, busca com debounce, "Atualizar nó" e "Atualizar tudo";
- nenhum segredo em DTO, log ou erro (FR-016);
- primeiro nível carrega **sem** trazer detalhes de todos os objetos.