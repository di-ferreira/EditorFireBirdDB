import { useCallback, useEffect, useState, type JSX } from "react";
import { invoke } from "@tauri-apps/api/core";

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
    padding: "2rem",
    minHeight: "100vh",
    boxSizing: "border-box" as const,
    background: "#f7f8fa",
    color: "#1c1e21",
  },
  cartao: {
    maxWidth: "40rem",
    margin: "0 auto",
    background: "#ffffff",
    border: "1px solid #d9dce1",
    borderRadius: "8px",
    padding: "1.5rem",
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

/**
 * Tela inicial mínima da Fase 0: demonstra os estados completos
 * (loading, sucesso, erro acionável) da chamada `app_health`.
 */
export default function App(): JSX.Element {
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

        {estado.fase === "carregando" && (
          <p role="status" aria-live="polite">
            Verificando a saúde do aplicativo…
          </p>
        )}

        {estado.fase === "sucesso" && (
          <div role="status" aria-live="polite">
            <p>
              Firebird Studio Linux — domínio {estado.health.domain_version}
            </p>
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
    </main>
  );
}
