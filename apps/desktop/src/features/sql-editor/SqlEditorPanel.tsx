import { useCallback, useEffect, useRef, useState, type JSX } from "react";

import {
  EditorView,
  keymap as cmKeymap,
  lineNumbers,
} from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { defaultKeymap } from "@codemirror/commands";
import { sql } from "@codemirror/lang-sql";

import { listarPerfis, consultarStatus } from "../connections/api";
import { buscarNoNoCatalogo } from "../db-explorer/api";
import type { ConnectionProfile, ConnectionStatusInfo } from "../connections/types";
import {
  executarConsulta,
  salvarScript,
} from "./api";
import type { ApiQueryResult, SqlTab } from "./types";

const novoid = () => Math.random().toString(36).slice(2, 10);

const splitStatements = (sql: string): string[] => {
  const out: string[] = [];
  let buf = "";
  let i = 0;
  let single = false;
  let double = false;
  let line = false;
  let block = false;
  while (i < sql.length) {
    const c = sql[i];
    const n = sql[i + 1];
    if (line) {
      buf += c;
      if (c === "\n") line = false;
      i++;
      continue;
    }
    if (block) {
      buf += c;
      if (c === "*" && n === "/") {
        buf += n;
        block = false;
        i += 2;
        continue;
      }
      i++;
      continue;
    }
    if (single) {
      buf += c;
      if (c === "'" && sql[i - 1] !== "\\") {
        if (n === "'") {
          buf += n;
          i += 2;
          continue;
        }
        single = false;
      }
      i++;
      continue;
    }
    if (double) {
      buf += c;
      if (c === '"') double = false;
      i++;
      continue;
    }
    if (c === "-" && n === "-") {
      buf += c + n;
      line = true;
      i += 2;
      continue;
    }
    if (c === "/" && n === "*") {
      buf += c + n;
      block = true;
      i += 2;
      continue;
    }
    if (c === "'") {
      single = true;
      buf += c;
      i++;
      continue;
    }
    if (c === '"') {
      double = true;
      buf += c;
      i++;
      continue;
    }
    if (c === ";") {
      if (buf.trim() !== "") out.push(buf.trim());
      buf = "";
      i++;
      continue;
    }
    buf += c;
    i++;
  }
  if (buf.trim() !== "") out.push(buf.trim());
  return out;
};

export default function SqlEditorPanel({ openTableRequest }: { openTableRequest?: { perfilId: string; table: string; nonce: number } | null } = {}): JSX.Element {
  const [perfilId, setPerfilId] = useState<string>("");
  const [perfis, setPerfis] = useState<ConnectionProfile[]>([]);
  const [statusPorPerfil, setStatusPorPerfil] = useState<Record<string, ConnectionStatusInfo>>({});
  const [abas, setAbas] = useState<SqlTab[]>([{ id: novoid(), title: "Nova consulta", content: "" }]);
  const [abaAtiva, setAbaAtiva] = useState<string>(abas[0].id);
  const [resultados, setResultados] = useState<ApiQueryResult[]>([]);
  const [mensagemErro, setMensagemErro] = useState<string | null>(null);
  const [executando, setExecutando] = useState<boolean>(false);
  const [objetosBusca, setObjetosBusca] = useState("");
  const [objetos, setObjetos] = useState<{ nome: string; kind: string }[]>([]);
  const [carregandoObjetos, setCarregandoObjetos] = useState(false);

  const carregarObjetos = useCallback(async (perfil: string) => {
    if (!perfil) { setObjetos([]); return; }
    setCarregandoObjetos(true);
    try {
      const linhas: { nome: string; kind: string }[] = [];
      for (const categoria of ["tabelas", "views", "procedures"] as const) {
        try {
          const r = await buscarNoNoCatalogo(perfil, { kind: "categoria", categoria }, null, false, false);
          for (const child of r.children ?? []) {
            linhas.push({ nome: child.label.trim(), kind: categoria });
          }
        } catch { /* ignora */ }
      }
      setObjetos(linhas);
    } finally {
      setCarregandoObjetos(false);
    }
  }, []);

  const [salvando, setSalvando] = useState<boolean>(false);
  const viewRef = useRef<EditorView | null>(null);
  const executarRef = useRef<(() => Promise<void>) | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  const carregarPerfis = useCallback(async () => {
    try {
      const lista = await listarPerfis();
      setPerfis(lista);
      const status: Record<string, ConnectionStatusInfo> = {};
      for (const p of lista) {
        try { status[p.id] = await consultarStatus(p.id); } catch { /* ignore */ }
      }
      setStatusPorPerfil(status);
      const conectados = lista.filter((p) => status[p.id]?.status === "connected");
      if (!perfilId && conectados.length > 0) setPerfilId(conectados[0].id);
    } catch (e) { setMensagemErro(String(e)); }
  }, [perfilId]);

  useEffect(() => { void carregarPerfis(); }, [carregarPerfis]);
  useEffect(() => { void carregarObjetos(perfilId); }, [perfilId, carregarObjetos]);

  const aba = abas.find((a) => a.id === abaAtiva) ?? abas[0];
  // Mount CodeMirror
  useEffect(() => {
    if (!containerRef.current) return;
    const view = new EditorView({
      state: EditorState.create({
        doc: aba?.content ?? "",
        extensions: [
          lineNumbers(),
          sql(),
          EditorView.lineWrapping,
          cmKeymap.of(defaultKeymap),
          cmKeymap.of([
            { key: "Ctrl-Enter", run: () => { void executarRef.current?.(); return true; }, preventDefault: true },
            { key: "Mod-s", run: () => { void salvarAbaComoScript(); return true; }, preventDefault: true },
          ]),
        ],
      }),
    });
    containerRef.current.appendChild(view.dom);
    viewRef.current = view;
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [abaAtiva]);

  const lerConteudo = () => viewRef.current?.state.doc.toString() ?? aba?.content ?? "";

  const atualizarAbaConteudo = () => {
    const texto = lerConteudo();
    setAbas((prev) => prev.map((a) => (a.id === abaAtiva ? { ...a, content: texto } : a)));
  };

  useEffect(() => {
    if (!openTableRequest) return;
    if (!openTableRequest.perfilId) return;
    setPerfilId(openTableRequest.perfilId);
    const id = novoid();
    setAbas((prev) => [...prev, { id, title: `Dados: ${openTableRequest.table}`, content: `SELECT FIRST 500 * FROM ${openTableRequest.table}` }]);
    setAbaAtiva(id);
    setTimeout(() => { void executarRef.current?.(); }, 120);
  }, [openTableRequest?.nonce]);


  const abrirDadosTabela = useCallback((tabela: string) => {
    if (!perfilId) return;
    const id = novoid();
    setAbas((prev) => [...prev, { id, title: `Dados: ${tabela}`, content: `SELECT FIRST 500 * FROM ${tabela}` }]);
    setAbaAtiva(id);
    setTimeout(() => { void executarRef.current?.(); }, 150);
  }, [perfilId]);

  const executar = useCallback(async () => {
    atualizarAbaConteudo();
    if (!perfilId) { setMensagemErro("Selecione uma conexão ativa."); return; }
    const texto = lerConteudo();
    const blocos = splitStatements(texto);
    if (blocos.length === 0) return;
    setExecutando(true);
    setMensagemErro(null);
    setResultados([]);
    try {
      const saidas: ApiQueryResult[] = [];
      for (const bloco of blocos) {
        const r = await executarConsulta(perfilId, bloco);
        saidas.push(r);
      }
      setResultados(saidas);
    } catch (e: any) {
      setMensagemErro(e?.message ? `${e.message}` : String(e));
    } finally {
      setExecutando(false);
    }
  }, [perfilId, abaAtiva]);
  useEffect(() => { executarRef.current = executar; }, [executar]);

  const novaAba = () => {
    const id = novoid();
    setAbas((prev) => [...prev, { id, title: "Nova consulta", content: "" }]);
    setAbaAtiva(id);
  };

  const fecharAba = (id: string) => {
    setAbas((prev) => {
      const rest = prev.filter((a) => a.id !== id);
      if (rest.length === 0) { setAbaAtiva(prev[0].id); return prev; }
      if (abaAtiva === id) setAbaAtiva(rest[rest.length - 1].id);
      return rest;
    });
  };

  const salvarAbaComoScript = useCallback(async () => {
    atualizarAbaConteudo();
    const nome = window.prompt("Nome do script:");
    if (!nome) return;
    setSalvando(true);
    try {
      await salvarScript(nome, lerConteudo(), perfilId || undefined);
    } catch (e: any) {
      setMensagemErro(e?.message ?? String(e));
    } finally {
      setSalvando(false);
    }
  }, [perfilId]);


  const estilo: Record<string, React.CSSProperties> = {
    raiz: { fontFamily: "system-ui, sans-serif", padding: "1rem", background: "#f7f8fa" },
    topo: { display: "flex", gap: "0.5rem", alignItems: "center", flexWrap: "wrap", marginBottom: "0.75rem" },
    selecionador: { padding: "0.35rem", borderRadius: "6px", border: "1px solid #b7bcc4" },
    abas: { display: "flex", gap: "0.25rem", flexWrap: "wrap", marginBottom: "0.5rem" },
    aba: { padding: "0.4rem 0.7rem", cursor: "pointer", border: "1px solid #d9dce1", borderRadius: "6px 6px 0 0", background: "#f0f1f4" },
    abaAtiva: { padding: "0.4rem 0.7rem", cursor: "pointer", border: "1px solid #d9dce1", borderRadius: "6px 6px 0 0", background: "#fff", fontWeight: 600 },
    botao: { padding: "0.4rem 0.8rem", cursor: "pointer", borderRadius: "6px", border: "1px solid #b7bcc4", background: "#fff" },
    editor: { border: "1px solid #d9dce1", borderRadius: "8px", minHeight: "240px", background: "#fff", marginBottom: "0.75rem" },
    lateral: { width: "300px", flexShrink: 0, border: "1px solid #d9dce1", borderRadius: "8px", padding: "0.5rem", background: "#fff", maxHeight: "70vh", overflow: "auto" },
    layout: { display: "flex", gap: "0.75rem", alignItems: "start" },
    erro: { color: "#a4262c", margin: "0.5rem 0" },
  };

  return (
    <section style={estilo.raiz}>
      <div style={estilo.topo}>
        <label>
          Conexão:
          <select style={estilo.selecionador} value={perfilId} onChange={(e) => setPerfilId(e.target.value)}>
            <option value="">— selecione —</option>
            {perfis.map((p) => (
              <option key={p.id} value={p.id} disabled={statusPorPerfil[p.id]?.status !== "connected"}>
                {p.name} ({p.host}:{p.port})
              </option>
            ))}
          </select>
        </label>
        <button type="button" style={estilo.botao} onClick={() => void executar()} disabled={executando}>
          {executando ? "Executando…" : "Executar"}
        </button>
        <button type="button" style={estilo.botao} onClick={() => void salvarAbaComoScript()} disabled={salvando}>
          {salvando ? "Salvando…" : "Salvar script"}
        </button>
        <button type="button" style={estilo.botao} onClick={() => novaAba()}>
          Nova aba
        </button>
      </div>

      {mensagemErro && <div role="alert" style={estilo.erro}>{mensagemErro}</div>}

      <div style={estilo.abas}>
        {abas.map((a) => (
          <button
            key={a.id}
            type="button"
            style={a.id === abaAtiva ? estilo.abaAtiva : estilo.aba}
            onClick={() => { atualizarAbaConteudo(); setAbaAtiva(a.id); }}
          >
            {a.title}
            {abas.length > 1 && (
              <span
                role="button"
                aria-label="Fechar aba"
                onClick={(e) => { e.stopPropagation(); fecharAba(a.id); }}
                style={{ marginLeft: "0.4rem", color: "#888" }}
              >
                ×
              </span>
            )}
          </button>
        ))}
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "240px 1fr", gap: "1rem", alignItems: "start" }}>
        <div style={{ border: "1px solid #d9dce1", borderRadius: "8px", padding: "0.5rem", background: "#fff", maxHeight: "calc(100vh - 12rem)", overflow: "auto" }}>
          <div style={{ fontWeight: 600, marginBottom: "0.4rem" }}>Objetos</div>
          <input
            type="search"
            placeholder="Filtrar…"
            value={objetosBusca}
            onChange={(e) => setObjetosBusca(e.target.value)}
            style={{ width: "100%", padding: "0.35rem", border: "1px solid #b7bcc4", borderRadius: "6px", marginBottom: "0.5rem" }}
          />
          {carregandoObjetos && <div style={{ fontSize: "0.8rem", color: "#4a4d52" }}>Carregando…</div>}
          <ul style={{ listStyle: "none", padding: 0, margin: 0, fontSize: "0.9rem" }}>
            {objetos.filter((o) => o.nome.toLowerCase().includes(objetosBusca.toLowerCase())).map((o, i) => (
              <li key={`${o.kind}-${o.nome}-${i}`} style={{ marginBottom: "0.2rem" }}>
                <button
                  type="button"
                  onDoubleClick={() => abrirDadosTabela(o.nome)}
                  style={{ width: "100%", textAlign: "left", padding: "0.3rem", border: "none", background: "transparent", cursor: "pointer" }}
                  title="Duplo clique para abrir dados"
                >
                  <span style={{ fontSize: "0.7rem", color: "#999", marginRight: "0.3rem" }}>{o.kind}</span>
                  {o.nome}
                </button>
              </li>
            ))}
          </ul>
        </div>
        <div style={{ minWidth: 0 }}>
          <div ref={containerRef} style={estilo.editor} />
          {resultados.map((r, idx) => (
            <div key={idx} style={{ marginBottom: "1rem" }}>
              <div style={{ fontSize: "0.85rem", color: "#4a4d52", marginBottom: "0.25rem" }}>
                {r.kind === "affected"
                  ? `Affected rows: ${r.affected ?? 0} · ${r.elapsedMs} ms`
                  : `${r.rowCount} linhas · ${r.elapsedMs} ms`}
              </div>
              {r.kind === "rows" && (
                <div style={{ overflow: "auto", border: "1px solid #d9dce1", borderRadius: "6px" }}>
                  <table style={{ borderCollapse: "collapse", width: "100%", fontSize: "0.9rem" }}>
                    <thead>
                      <tr>
                        {r.columns.map((c, i) => (
                          <th key={i} style={{ textAlign: "left", padding: "0.35rem", borderBottom: "1px solid #d9dce1", background: "#f0f1f4", whiteSpace: "nowrap" }}>
                            {c}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {r.rows.map((row, ri) => (
                        <tr key={ri}>
                          {row.map((cell, ci) => (
                            <td key={ci} style={{ padding: "0.3rem", borderBottom: "1px solid #eee", whiteSpace: "nowrap" }}>
                              {cell === null ? <em style={{ color: "#999" }}>NULL</em> : cell}
                            </td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          ))}
        </div>


      </div>
    </section>
  );
}