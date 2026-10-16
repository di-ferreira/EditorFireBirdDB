import { useCallback, useEffect, useState, type JSX } from "react";
import { invoke } from "@tauri-apps/api/core";

import ConnectionsPanel from "./features/connections/ConnectionsPanel";
import DbExplorerPanel from "./features/db-explorer/DbExplorerPanel";
import SqlEditorPanel from "./features/sql-editor/SqlEditorPanel";

/**
 * Resposta do comando Rust `app_health` (src-tauri/src/lib.rs).
 *
 * O frontend acessa o backend **exclusivamente** via `invoke` de comandos
 * Tauri tipados: nenhuma chamada direta a Firebird, arquivos ou segredos.
 */
interface AppHealth {
  status: string;
  domain_version: string;
}

type Estado =
  | { fase: "carregando" }
  | { fase: "sucesso"; health: AppHealth }
  | { fase: "erro"; mensagem: string };

const estilos = {
  raiz: {
    fontFamily:
      'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", sans-serif',
    margin: "0",
    padding: "1rem",
    minHeight: "calc(100vh - 1rem)",
    height: "calc(100vh - 1rem)",
    boxSizing: "border-box" as const,
    background: "#f7f8fa",
    color: "#1c1e21",
  },
  cartao: {
    maxWidth: "100%",
    margin: "0 auto",
    background: "#ffffff",
    border: "1px solid #d9dce1",
    borderRadius: "8px",
    padding: "1.5rem",
  },
  abas: {
    display: "flex",
    gap: "0.5rem",
    marginBottom: "1.25rem",
    borderBottom: "1px solid #d9dce1",
  },
  aba: {
    padding: "0.5rem 0.9rem",
    fontSize: "0.95rem",
    cursor: "pointer",
    border: "1px solid transparent",
    borderBottom: "none",
    borderRadius: "6px 6px 0 0",
    background: "transparent",
    color: "#4a4d52",
  },
  abaAtiva: {
    padding: "0.5rem 0.9rem",
    fontSize: "0.95rem",
    cursor: "pointer",
    border: "1px solid #d9dce1",
    borderBottom: "2px solid #1b5e9c",
    borderRadius: "6px 6px 0 0",
    background: "#ffffff",
    color: "#1b5e9c",
    fontWeight: 600,
  },
  botao: {
    marginTop: "1rem",
    padding: "0.5rem 1rem",
    fontSize: "0.95rem",
    cursor: "pointer",
    borderRadius: "6px",
    border: "1px solid #b7bcc4",
    background: "#ffffff",
  },
};

/** Abas da janela principal (Spec 11 §3: navegação por teclado, foco visível). */
type Aba = "saude" | "conexoes" | "explorador" | "sql";

const ABAS: { id: Aba; rotulo: string }[] = [
  { id: "saude", rotulo: "Saúde" },
  { id: "conexoes", rotulo: "Conexões" },
  { id: "explorador", rotulo: "Explorador" },
  { id: "sql", rotulo: "SQL" },
];

/**
 * Tela inicial da Fase 1: tela de saúde do backend (`app_health`) e o
 * Gerenciador de Conexões (Spec 03). Todo acesso ao backend passa por
 * `invoke` de comandos Tauri tipados.
 */
export default function App(): JSX.Element {
  const [aba, setAba] = useState<Aba>("saude");
  const [pendingOpenTable, setPendingOpenTable] = useState<{ perfilId: string; table: string; nonce: number } | null>(null);
  const [estado, setEstado] = useState<Estado>({ fase: "carregando" });

  const carregarSaude = useCallback(async () => {
    setEstado({ fase: "carregando" });
    try {
      const health = await invoke<AppHealth>("app_health");
      setEstado({ fase: "sucesso", health });
    } catch (erro) {
      const mensagem = erro instanceof Error ? erro.message : String(erro);
      setEstado({ fase: "erro", mensagem });
    }
  }, []);

  useEffect(() => {
    void carregarSaude();
  }, [carregarSaude]);

  return (
    <main style={estilos.raiz}>
      <div style={estilos.cartao}>
        <h1 style={{ marginTop: 0, fontSize: "1.4rem" }}>Firebird Studio Linux</h1>

        <div style={estilos.abas} role="tablist" aria-label="Seções do aplicativo">
          {ABAS.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              id={`aba-${item.id}`}
              aria-selected={aba === item.id}
              aria-controls={`painel-${item.id}`}
              tabIndex={aba === item.id ? 0 : -1}
              style={aba === item.id ? estilos.abaAtiva : estilos.aba}
              onClick={() => setAba(item.id)}
            >
              {item.rotulo}
            </button>
          ))}
        </div>

        <div
          role="tabpanel"
          id={`painel-${aba}`}
          aria-labelledby={`aba-${aba}`}
          hidden={aba !== "saude"}
        >
          {estado.fase === "carregando" && (
            <p role="status" aria-live="polite">
              Verificando a saúde do aplicativo…
            </p>
          )}

          {estado.fase === "sucesso" && (
            <div role="status" aria-live="polite">
              <p>Firebird Studio Linux — domínio {estado.health.domain_version}</p>
              <p style={{ color: "#4a4d52" }}>
                Status do backend: <strong>{estado.health.status}</strong>
              </p>
            </div>
          )}

          {estado.fase === "erro" && (
            <div role="alert">
              <p style={{ color: "#a4262c" }}>
                Não foi possível consultar a saúde do aplicativo.
              </p>
              <p style={{ color: "#4a4d52" }}>Detalhe: {estado.mensagem}</p>
              <button type="button" style={estilos.botao} onClick={() => void carregarSaude()}>
                Tentar novamente
              </button>
            </div>
          )}
        </div>

        <div
          role="tabpanel"
          id="painel-conexoes"
          aria-labelledby="aba-conexoes"
          hidden={aba !== "conexoes"}
        >
          {/* Montado apenas quando a aba é aberta: a UI inicia sem I/O de conexões. */}
          {aba === "conexoes" && <ConnectionsPanel />}
        </div>

        <div
          role="tabpanel"
          id="painel-explorador"
          aria-labelledby="aba-explorador"
          hidden={aba !== "explorador"}
        >
          {/* Montado apenas quando a aba é aberta. */}
          {aba === "explorador" && <DbExplorerPanel onOpenTable={(perfilId, relacao) => { setAba("sql"); setPendingOpenTable({ perfilId, table: relacao, nonce: Date.now() }); }} />}
        </div>

        <div
          role="tabpanel"
          id="painel-sql"
          aria-labelledby="aba-sql"
          hidden={aba !== "sql"}
        >
          {aba === "sql" && (
            <div style={{ display: "grid", gridTemplateColumns: "320px 1fr", gap: "1rem", alignItems: "start" }}>
              <div style={{ overflow: "auto", maxHeight: "70vh" }}>
                <DbExplorerPanel onOpenTable={(perfilId, relacao) => { setAba("sql"); setPendingOpenTable({ perfilId, table: relacao, nonce: Date.now() }); }} />
              </div>
              <div style={{ minWidth: 0 }}>
                <SqlEditorPanel openTableRequest={pendingOpenTable} />
              </div>
            </div>
          )}
        </div>
      </div>
    </main>
  );
}
