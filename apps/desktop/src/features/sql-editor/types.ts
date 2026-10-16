export type SqlTab = { id: string; title: string; content: string };

export type ApiQueryResult = {
  columns: string[];
  rows: (string | null)[][];
  rowCount: number;
  elapsedMs: number;
  kind: "rows" | "affected";
  affected?: number;
  message?: string | null;
  /** `true` quando existe mais linhas no servidor além desta janela. */
  hasMore?: boolean;
};

export type SavedScriptDto = {
  id: string;
  name: string;
  content: string;
  connectionProfileId?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type HistoryEntryDto = {
  id: string;
  connectionProfileId?: string | null;
  queryText: string;
  status: string;
  durationMs?: number | null;
  rowsReturned?: number | null;
  errorCode?: string | null;
  errorMessage?: string | null;
  executedAt: string;
  executionMode: string;
};
