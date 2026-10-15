# Spec 10 — Contratos Internos Principais

**Documento de origem:** Seção 9 da especificação técnica

Os exemplos abaixo são conceituais. DTOs finais deverão ter validação e limites explícitos.

## 1. Conexão

```ts
export type ConnectionProfile = {
  id: string;
  name: string;
  host: string;
  port: number;
  databasePath: string;
  username: string;
  roleName?: string;
  charset: string;
  sqlDialect?: 1 | 3;
  clientLibraryPath?: string;
};

export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'error';
```

O perfil enviado pela interface não inclui senha. O comando de conexão recebe uma
referência de segredo ou consulta o cofre no backend.

## 2. Resultado SQL

```ts
export type QueryColumn = {
  name: string;
  typeName?: string;
  nullable?: boolean;
  precision?: number;
  scale?: number;
};

export type QueryExecutionResult = {
  columns: QueryColumn[];
  rows: unknown[][];
  rowsReturned: number;
  truncated: boolean;
  durationMs: number;
  transactionState: 'read-only' | 'write' | 'unknown';
  warnings: string[];
};
```

Resultados grandes devem usar paginação/streaming ou comandos de busca de lote, em vez
de devolver todas as linhas em uma única resposta IPC.

## 3. Operações Tauri sugeridas

`connection_list_profiles` · `connection_save_profile` · `connection_test_profile` ·
`connection_open` · `connection_close` · `connection_get_status` ·
`schema_get_tree` · `schema_refresh` · `schema_get_object_details` ·
`sql_execute` · `sql_fetch_next_page` · `sql_cancel` ·
`query_history_list` · `query_history_clear` ·
`saved_script_save` · `saved_script_load` ·
`ai_list_providers` · `ai_test_provider` ·
`ai_generate_sql` · `ai_explain_sql` · `ai_repair_sql` ·
`ai_history_list` · `settings_get` · `settings_update`

Os comandos devem retornar erros tipados (`ConnectionError`,
`UnsupportedServerFeature`, `QueryError`, `PermissionDenied`, `ProviderError`), sem expor
stack traces ou segredos diretamente na interface.
