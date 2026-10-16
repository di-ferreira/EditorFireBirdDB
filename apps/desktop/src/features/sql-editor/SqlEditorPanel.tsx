import { useCallback, useEffect, useRef, useState, type JSX } from "react";

import {
  EditorView,
  keymap as cmKeymap,
  lineNumbers,
} from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
import { defaultKeymap } from "@codemirror/commands";
import { sql } from "@codemirror/lang-sql";

import { listarPerfis, consultarStatus } from "../connections/api";
import { buscarNoNoCatalogo } from "../db-explorer/api";
import type { ConnectionProfile, ConnectionStatusInfo } from "../connections/types";
import {
  cancelarConsulta,
  executarConsulta,
  executarPagina,
  salvarScript,
} from "./api";
import type { ApiQueryResult, SqlTab } from "./types";

/** Uma execução na grade, com o estado de paginação da consulta. */
type BlocoResultado = {
  id: string;
  sql: string;
  resultado: ApiQueryResult;
  queryId: string;
  temMais: boolean;
  carregandoMais: boolean;
};

const novoid = () => Math.random().toString(36).slice(2, 10);

/** Linhas por página (paginação no servidor + scroll progressivo). */
const TAMANHO_PAGINA = 100;

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

export default function SqlEditorPanel({ openTableRequest, escuro }: { openTableRequest?: { perfilId: string; table: string; nonce: number } | null; escuro?: boolean } = {}): JSX.Element {
  const [perfilId, setPerfilId] = useState<string>("");
  const [perfis, setPerfis] = useState<ConnectionProfile[]>([]);
  const [statusPorPerfil, setStatusPorPerfil] = useState<Record<string, ConnectionStatusInfo>>({});
  const [abas, setAbas] = useState<SqlTab[]>([{ id: novoid(), title: "Nova consulta", content: "" }]);
  const [abaAtiva, setAbaAtiva] = useState<string>(abas[0].id);
  const [blocos, setBlocos] = useState<BlocoResultado[]>([]);
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

      // Campos das tabelas (limitado) para o autocomplete de colunas.
      const mapa: Record<string, string[]> = {};
      const tabelas = linhas.filter((l) => l.kind === "tabelas").slice(0, 40);
      for (const t of tabelas) {
        try {
          const campos = await buscarNoNoCatalogo(perfil, { kind: "campos", relacao: t.nome }, null, false, false);
          mapa[t.nome] = (campos.children ?? []).map((c) => c.label.trim());
        } catch { /* ignora */ }
      }
      camposPorTabelaRef.current = mapa;
    } finally {
      setCarregandoObjetos(false);
    }
  }, []);

  const [salvando, setSalvando] = useState<boolean>(false);
  const viewRef = useRef<EditorView | null>(null);
  const executarRef = useRef<(() => Promise<void>) | null>(null);
  const objetosRef = useRef<{ nome: string; kind: string }[]>([]);
  const camposPorTabelaRef = useRef<Record<string, string[]>>({});
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
    const palavrasChave = ["SELECT","FROM","WHERE","GROUP","BY","ORDER","HAVING","JOIN","LEFT","RIGHT","INNER","OUTER","ON","LIKE","IN","NOT","AND","OR","AS","DISTINCT","FIRST","SKIP","STARTING","WITH","UNION","ALL","INSERT","INTO","VALUES","UPDATE","SET","DELETE","CREATE","ALTER","TABLE","VIEW","INDEX","PROCEDURE","TRIGGER","COMMIT","ROLLBACK"];
    const fonteCompletacao = (context: CompletionContext) => {
      const word = context.matchBefore(/\w*/);
      if (!word || (word.from === word.to && !context.explicit)) return null;
      const nomesDeCampos = Array.from(new Set(Object.values(camposPorTabelaRef.current).flat()));
      const itens = [
        ...palavrasChave.map((k) => ({ label: k, type: "keyword" })),
        ...objetosRef.current.map((o) => ({ label: o.nome, type: o.kind, info: o.kind })),
        ...nomesDeCampos.map((c) => ({ label: c, type: "field", info: "campo" })),
      ];
      const lower = word.text.toLowerCase();
      const filtrados = itens.filter((i) => i.label.toLowerCase().startsWith(lower));
      if (filtrados.length === 0) return null;
      return { from: word.from, options: filtrados.slice(0, 60), validFor: /^\w*$/ };
    };

    const view = new EditorView({
      state: EditorState.create({
        doc: aba?.content ?? "",
        extensions: [
          lineNumbers(),
          sql(),
          EditorView.lineWrapping,
          autocompletion({ activateOnTyping: true, override: [fonteCompletacao] }),
          cmKeymap.of(defaultKeymap),
          cmKeymap.of([
            { key: "Ctrl-Enter", run: () => { void executarRef.current?.(); return true; }, preventDefault: true },
            { key: "Ctrl-Shift-Enter", run: () => { void executarRef.current?.(); return true; }, preventDefault: true },
            { key: "Mod-s", run: () => { void salvarAbaComoScript(); return true; }, preventDefault: true },
            { key: "Mod-n", run: () => { novaAba(); return true; }, preventDefault: true },
            { key: "Mod-w", run: () => {
              if (abas.length > 1) fecharAba(abaAtiva);
              return true;
            }, preventDefault: true },
            { key: "F9", run: () => { void executarRef.current?.(); return true; }, preventDefault: true },
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

  const consultaAtivaRef = useRef<string | null>(null);

  const executar = useCallback(async () => {
    atualizarAbaConteudo();
    if (!perfilId) { setMensagemErro("Selecione uma conexão ativa."); return; }
    const texto = lerConteudo();
    const instrucoes = splitStatements(texto);
    if (instrucoes.length === 0) return;
    setExecutando(true);
    setMensagemErro(null);
    setBlocos([]);
    try {
      for (const instrucao of instrucoes) {
        const ehConsulta = /^\s*(select|with)\b/i.test(instrucao);
        if (ehConsulta) {
          const queryId = novoid();
          consultaAtivaRef.current = queryId;
          const pagina = await executarPagina(perfilId, instrucao, queryId, 0, TAMANHO_PAGINA);
          setBlocos((prev) => [
            ...prev,
            {
              id: novoid(),
              sql: instrucao,
              resultado: pagina,
              queryId,
              temMais: pagina.hasMore ?? false,
              carregandoMais: false,
            },
          ]);
        } else {
          const r = await executarConsulta(perfilId, instrucao);
          setBlocos((prev) => [
            ...prev,
            { id: novoid(), sql: instrucao, resultado: r, queryId: "", temMais: false, carregandoMais: false },
          ]);
        }
      }
    } catch (e: any) {
      setMensagemErro(e?.message ? `${e.message}` : String(e));
    } finally {
      consultaAtivaRef.current = null;
      setExecutando(false);
    }
  }, [perfilId, abaAtiva]);

  /** Carrega a próxima janela de 100 linhas de um bloco com scroll. */
  const carregarMais = useCallback(async (bloco: BlocoResultado) => {
    if (!bloco.temMais || bloco.carregandoMais || !perfilId) return;
    setBlocos((prev) =>
      prev.map((b) => (b.id === bloco.id ? { ...b, carregandoMais: true } : b)),
    );
    try {
      const pagina = await executarPagina(
        perfilId,
        bloco.sql,
        bloco.queryId,
        bloco.resultado.rows.length,
        TAMANHO_PAGINA,
      );
      setBlocos((prev) =>
        prev.map((b) =>
          b.id === bloco.id
            ? {
                ...b,
                resultado: {
                  ...b.resultado,
                  columns: pagina.columns.length > 0 ? pagina.columns : b.resultado.columns,
                  rows: [...b.resultado.rows, ...pagina.rows],
                  rowCount: pagina.rowCount,
                  elapsedMs: b.resultado.elapsedMs + pagina.elapsedMs,
                },
                temMais: pagina.hasMore ?? false,
                carregandoMais: false,
              }
            : b,
        ),
      );
    } catch (e: any) {
      setBlocos((prev) => prev.map((b) => (b.id === bloco.id ? { ...b, carregandoMais: false } : b)));
      setMensagemErro(e?.message ? `${e.message}` : String(e));
    }
  }, [perfilId]);

  /** Cancela a consulta em execução (cancelamento cooperativo no servidor). */
  const cancelar = useCallback(async () => {
    const queryId = consultaAtivaRef.current;
    if (!queryId) return;
    await cancelarConsulta(queryId).catch(() => false);
  }, []);

  /** Exporta as linhas já carregadas do bloco em CSV (UTF-8 com BOM). */
  const exportarCsv = useCallback((bloco: BlocoResultado) => {
    const escapar = (valor: string | null) =>
      valor === null ? "" : `"${valor.replace(/"/g, '""')}"`;
    const cabecalho = bloco.resultado.columns.map(escapar).join(";");
    const corpo = bloco.resultado.rows.map((linha) => linha.map(escapar).join(";")).join("\r\n");
    const conteudo = `\uFEFF${[cabecalho, corpo].filter(Boolean).join("\r\n")}`;
    const blob = new Blob([conteudo], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "firebird-studio-resultado.csv";
    link.click();
    URL.revokeObjectURL(url);
  }, []);

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
    <>
      {escuro && (
        <style>{`
.dark-mode, .dark-mode * { color: #e8eaed; }
.dark-mode [style*="background: #fff"], .dark-mode [style*="background: #ffffff"] { background: #1b1f26 !important; }
.dark-mode [style*="border"] { border-color: #3b4048 !important; }
.dark-mode input, .dark-mode select, .dark-mode button, .dark-mode textarea { background: #23272f !important; color: #e8eaed !important; border-color: #3b4048 !important; }
.dark-mode .cm-editor, .dark-mode .cm-gutters { background: #1b1f26 !important; color: #e8eaed !important; }
.dark-mode .cm-content { color: #e8eaed !important; }
`}</style>
      )}
    <section style={{ ...estilo.raiz, background: escuro ? "#121417" : "#f7f8fa", color: escuro ? "#e8eaed" : "#1c1e21" }} className={escuro ? "dark-mode" : "light-mode"}>
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
        {executando && (
          <button type="button" style={{ ...estilo.botao, borderColor: "#a4262c", color: "#a4262c" }} onClick={() => void cancelar()}>
            Cancelar
          </button>
        )}
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
          {blocos.map((bloco) => {
            const r = bloco.resultado;
            return (
              <div key={bloco.id} style={{ marginBottom: "1rem" }}>
                <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", flexWrap: "wrap", marginBottom: "0.25rem" }}>
                  <span style={{ fontSize: "0.85rem", color: "#4a4d52" }}>
                    {r.kind === "affected"
                      ? `Affected rows: ${r.affected ?? 0} · ${r.elapsedMs} ms`
                      : `${r.rows.length} carregadas${bloco.temMais ? " (há mais no servidor)" : ""} · ${r.elapsedMs} ms`}
                  </span>
                  {r.kind === "rows" && (
                    <>
                      <button type="button" style={estilo.botao} onClick={() => exportarCsv(bloco)}>
                        Exportar CSV
                      </button>
                      {bloco.temMais && (
                        <button
                          type="button"
                          style={estilo.botao}
                          disabled={bloco.carregandoMais}
                          onClick={() => void carregarMais(bloco)}
                        >
                          {bloco.carregandoMais ? "Carregando…" : "Carregar mais"}
                        </button>
                      )}
                    </>
                  )}
                </div>
                {r.kind === "rows" && (
                  <div
                    style={{ overflowY: "auto", maxHeight: "45vh", border: "1px solid #d9dce1", borderRadius: "6px" }}
                    onScroll={(e) => {
                      const el = e.currentTarget;
                      if (bloco.temMais && el.scrollHeight - el.scrollTop - el.clientHeight < 96) {
                        void carregarMais(bloco);
                      }
                    }}
                  >
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
                    {bloco.carregandoMais && (
                      <p role="status" style={{ padding: "0.4rem", fontSize: "0.8rem", color: "#4a4d52" }}>
                        Buscando próxima página no servidor…
                      </p>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>


      </div>
    </section>
    </>
  );
}