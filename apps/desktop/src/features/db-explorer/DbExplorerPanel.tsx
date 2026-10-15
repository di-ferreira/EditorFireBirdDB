import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type JSX,
  type KeyboardEvent,
} from "react";

import {
  buscarNoNoCatalogo,
  normalizarErro,
  type CommandError,
} from "./api";
import type { CatalogNodeDto } from "./types";
import {
  listarPerfis,
  consultarStatus,
} from "../connections/api";
import { isCommandError, ROTULO_CODIGO_ERRO, type ConnectionProfile } from "../connections/types";

const estilos = {
  painel: { display: "flex", flexDirection: "column" as const, gap: "1rem" },
  cartao: { background: "#ffffff", border: "1px solid #d9dce1", borderRadius: "8px", padding: "1rem 1.25rem" },
  linha: { display: "flex", alignItems: "center", gap: "0.5rem", flexWrap: "wrap" as const },
  entrada: { padding: "0.4rem 0.5rem", fontSize: "0.9rem", border: "1px solid #b7bcc4", borderRadius: "4px" },
  botao: { padding: "0.3rem 0.6rem", fontSize: "0.85rem", cursor: "pointer", borderRadius: "4px", border: "1px solid #b7bcc4", background: "#ffffff" },
  botaoPrimario: { padding: "0.4rem 0.8rem", fontSize: "0.9rem", cursor: "pointer", borderRadius: "4px", border: "1px solid #1b5e9c", background: "#1b5e9c", color: "#ffffff" },
  aviso: { padding: "0.75rem 1rem", borderRadius: "6px", border: "1px solid #e2b8b8", background: "#fdf2f2", color: "#8a1c22" },
  erroNo: { color: "#a4262c", fontSize: "0.85rem" },
  sistema: { color: "#6b6f76", fontStyle: "italic" as const },
  badgeSistema: { fontSize: "0.7rem", padding: "0.1rem 0.4rem", borderRadius: "999px", border: "1px solid #c9ccd2", color: "#6b6f76", background: "#f1f2f4" },
  item: { display: "flex", alignItems: "center", gap: "0.4rem", padding: "0.2rem 0", outline: "none" },
  detalhe: { color: "#4a4d52", fontSize: "0.8rem" },
  toggleBtn: { border: "none", background: "transparent", cursor: "pointer", padding: "0 0.2rem", fontSize: "0.9rem", lineHeight: 1 },
  grupo: { marginLeft: "1.25rem", borderLeft: "1px dashed #d9dce1", paddingLeft: "0.5rem" },
};

type PerfilConectado = { id: string; nome: string };

type TreeNodeProps = {
  node: CatalogNodeDto;
  perfilId: string;
  incluirSistema: boolean;
  filtro: string;
  nivel: number;
  onMensagem: (m: string) => void;
};

function TreeNode({ node: nodeInicial, perfilId, incluirSistema, filtro, nivel, onMensagem }: TreeNodeProps): JSX.Element {
  const [node, setNode] = useState<CatalogNodeDto>(nodeInicial);
  const [expandido, setExpandido] = useState(false);
  const [carregando, setCarregando] = useState(false);
  const [erro, setErro] = useState<CommandError | null>(null);
  const [filhos, setFilhos] = useState<CatalogNodeDto[] | null>(null);

  const temFilhosProvaveis = node.hasChildren && node.request !== undefined;

  const buscar = useCallback(
    async (forcar: boolean, expandirDepois: boolean) => {
      if (!node.request) return;
      setCarregando(true);
      setErro(null);
      try {
        const resultado = await buscarNoNoCatalogo(
          perfilId,
          node.request,
          filtro.length > 0 ? filtro : null,
          incluirSistema,
          forcar,
        );
        setNode((anterior) => ({
          ...anterior,
          ...resultado,
          children: undefined,
        }));
        if (expandirDepois || expandido) {
          setFilhos(resultado.children ?? []);
          setExpandido(true);
        }
        onMensagem(`Nó "${resultado.label}" atualizado.`);
      } catch (e) {
        setErro(normalizarErro(e));
      } finally {
        setCarregando(false);
      }
    },
    [node.request, perfilId, filtro, incluirSistema, expandido, onMensagem],
  );

  async function aoToggle(): Promise<void> {
    if (!temFilhosProvaveis) return;
    if (expandido) {
      setExpandido(false);
      return;
    }
    if (filhos !== null) {
      setExpandido(true);
      return;
    }
    await buscar(false, true);
  }

  async function aoAtualizar(): Promise<void> {
    await buscar(true, expandido || temFilhosProvaveis);
    if (!temFilhosProvaveis) {
      onMensagem(`Nó "${node.label}" atualizado.`);
    }
  }

  const ariaExpanded = temFilhosProvaveis ? expandido : undefined;

  return (
    <div>
      <div
        role="treeitem"
        aria-level={nivel}
        aria-expanded={ariaExpanded}
        data-level={nivel}
        tabIndex={0}
        style={estilos.item}
      >
        {temFilhosProvaveis && (
          <button
            type="button"
            className="toggle"
            style={estilos.toggleBtn}
            onClick={() => void aoToggle()}
            aria-label={expandido ? "Recolher" : "Expandir"}
            aria-hidden="true"
            tabIndex={-1}
          >
            {carregando ? "…" : expandido ? "▾" : "▸"}
          </button>
        )}
        {!temFilhosProvaveis && <span style={{ width: "1rem", display: "inline-block" }} />}

        <span style={{ fontWeight: 500 }}>
          {node.label}
        </span>
        {node.isSystem && (
          <span style={estilos.badgeSistema} aria-label="objeto de sistema">
            sistema
          </span>
        )}
        {node.detail && <span style={estilos.detalhe}>— {node.detail}</span>}
        {node.request && (
          <button
            type="button"
            style={{ ...estilos.botao, marginLeft: "0.5rem", padding: "0.1rem 0.4rem", fontSize: "0.75rem" }}
            onClick={() => void aoAtualizar()}
            disabled={carregando}
            aria-label={`Atualizar nó ${node.label}`}
          >
            {carregando ? "Atualizando…" : "Atualizar"}
          </button>
        )}
      </div>

      {erro && (
        <div role="alert" style={{ ...estilos.erroNo, marginLeft: "1.25rem" }}>
          {ROTULO_CODIGO_ERRO[erro.code]}: {erro.message}
          {node.request && (
            <button
              type="button"
              style={{ ...estilos.botao, marginLeft: "0.5rem" }}
              onClick={() => void aoToggle()}
            >
              Tentar novamente
            </button>
          )}
        </div>
      )}

      {expandido && filhos !== null && (
        <div role="group" style={estilos.grupo}>
          {filhos.length === 0 && (
            <p role="status" style={{ margin: "0.25rem 0", fontSize: "0.85rem", color: "#4a4d52" }}>
              Nenhum item neste nível.
            </p>
          )}
          {filhos.map((filho) => (
            <TreeNode
              key={filho.nodeId}
              node={filho}
              perfilId={perfilId}
              incluirSistema={incluirSistema}
              filtro={filtro}
              nivel={nivel + 1}
              onMensagem={onMensagem}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default function DbExplorerPanel(): JSX.Element {
  const [perfis, setPerfis] = useState<PerfilConectado[]>([]);
  const [carregandoPerfis, setCarregandoPerfis] = useState(true);
  const [erroPerfis, setErroPerfis] = useState<CommandError | null>(null);
  const [perfilId, setPerfilId] = useState<string>("");
  const [incluirSistema, setIncluirSistema] = useState(false);
  const [buscaDigitada, setBuscaDigitada] = useState("");
  const [buscaAplicada, setBuscaAplicada] = useState("");
  const [raiz, setRaiz] = useState<CatalogNodeDto | null>(null);
  const [carregandoRaiz, setCarregandoRaiz] = useState(false);
  const [erroRaiz, setErroRaiz] = useState<CommandError | null>(null);
  const [chaveArvore, setChaveArvore] = useState(0);
  const [mensagem, setMensagem] = useState<string>("");
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const t = setTimeout(() => setBuscaAplicada(buscaDigitada.trim()), 300);
    return () => clearTimeout(t);
  }, [buscaDigitada]);

  const carregarPerfisConectados = useCallback(async () => {
    setCarregandoPerfis(true);
    setErroPerfis(null);
    try {
      const lista = await listarPerfis();
      const conectados: PerfilConectado[] = [];
      await Promise.all(
        lista.map(async (p: ConnectionProfile) => {
          try {
            const info = await consultarStatus(p.id);
            if (info.status === "connected" || info.status === "reconnecting") {
              conectados.push({ id: p.id, nome: p.name });
            }
          } catch {
            // ignora
          }
        }),
      );
      setPerfis(conectados);
      if (conectados.length > 0 && perfilId === "") {
        setPerfilId(conectados[0].id);
      }
    } catch (e) {
      const cmd = isCommandError(e) ? (e as CommandError) : normalizarErro(e);
      setErroPerfis(cmd);
    } finally {
      setCarregandoPerfis(false);
    }
  }, [perfilId]);

  useEffect(() => {
    void carregarPerfisConectados();
  }, []);

  const carregarRaiz = useCallback(
    async (forcar: boolean) => {
      if (perfilId === "") return;
      setCarregandoRaiz(true);
      setErroRaiz(null);
      try {
        const r = await buscarNoNoCatalogo(
          perfilId,
          { kind: "raiz" },
          buscaAplicada.length > 0 ? buscaAplicada : null,
          incluirSistema,
          forcar,
        );
        setRaiz(r);
      } catch (e) {
        setErroRaiz(normalizarErro(e));
      } finally {
        setCarregandoRaiz(false);
      }
    },
    [perfilId, buscaAplicada, incluirSistema],
  );

  useEffect(() => {
    void carregarRaiz(false);
  }, [carregarRaiz]);

  const aoAtualizarTudo = () => {
    setChaveArvore((k) => k + 1);
    void carregarRaiz(true);
  };

  const aoTeclado = useCallback((evento: KeyboardEvent<HTMLDivElement>) => {
    const container = containerRef.current;
    if (!container) return;
    const items = Array.from(container.querySelectorAll('[role="treeitem"]')) as HTMLElement[];
    const idx = items.indexOf(document.activeElement as HTMLElement);
    if (idx < 0) return;
    switch (evento.key) {
      case "ArrowDown": {
        evento.preventDefault();
        items[Math.min(idx + 1, items.length - 1)]?.focus();
        break;
      }
      case "ArrowUp": {
        evento.preventDefault();
        items[Math.max(idx - 1, 0)]?.focus();
        break;
      }
      case "ArrowRight": {
        evento.preventDefault();
        const el = items[idx];
        const expanded = el.getAttribute("aria-expanded");
        if (expanded === "false") {
          const btn = el.querySelector("button.toggle") as HTMLButtonElement | null;
          btn?.click();
        } else {
          const nivelAtual = Number(el.getAttribute("data-level"));
          const proximo = items.find((it, i) => i > idx && Number(it.getAttribute("data-level")) === nivelAtual + 1);
          proximo?.focus();
        }
        break;
      }
      case "ArrowLeft": {
        evento.preventDefault();
        const el = items[idx];
        const expanded = el.getAttribute("aria-expanded");
        if (expanded === "true") {
          const btn = el.querySelector("button.toggle") as HTMLButtonElement | null;
          btn?.click();
        } else {
          const nivelAtual = Number(el.getAttribute("data-level"));
          let pai: HTMLElement | null = null;
          for (let i = idx - 1; i >= 0; i--) {
            if (Number(items[i].getAttribute("data-level")) === nivelAtual - 1) {
              pai = items[i];
              break;
            }
          }
          pai?.focus();
        }
        break;
      }
      case "Enter":
      case " ": {
        evento.preventDefault();
        const btn = items[idx].querySelector("button.toggle") as HTMLButtonElement | null;
        btn?.click();
        break;
      }
      default:
        break;
    }
  }, []);

  if (carregandoPerfis) {
    return (
      <section style={estilos.painel} aria-labelledby="dbexplorer-titulo">
        <h2 id="dbexplorer-titulo" style={{ margin: 0, fontSize: "1.15rem" }}>Explorador de banco de dados</h2>
        <div role="status" aria-live="polite">Carregando perfis…</div>
      </section>
    );
  }

  if (erroPerfis) {
    return (
      <section style={estilos.painel} aria-labelledby="dbexplorer-titulo">
        <h2 id="dbexplorer-titulo" style={{ margin: 0, fontSize: "1.15rem" }}>Explorador de banco de dados</h2>
        <div role="alert" style={estilos.aviso}>
          <p style={{ margin: "0 0 0.5rem" }}>Não foi possível carregar os perfis.</p>
          <p style={{ margin: "0 0 0.5rem" }}>
            {ROTULO_CODIGO_ERRO[erroPerfis.code]}: {erroPerfis.message}
          </p>
          <button type="button" style={estilos.botao} onClick={() => void carregarPerfisConectados()}>
            Tentar novamente
          </button>
        </div>
      </section>
    );
  }

  return (
    <section style={estilos.painel} aria-labelledby="dbexplorer-titulo">
      <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", flexWrap: "wrap" }}>
        <h2 id="dbexplorer-titulo" style={{ margin: 0, fontSize: "1.15rem" }}>
          Explorador de banco de dados
        </h2>
      </div>

      <div style={estilos.cartao}>
        <div style={{ ...estilos.linha, gap: "1rem" }}>
          <label>
            Perfil conectado:
            <select
              style={{ ...estilos.entrada, marginLeft: "0.5rem" }}
              value={perfilId}
              onChange={(e) => setPerfilId(e.target.value)}
              aria-label="Perfil de conexão conectado"
            >
              <option value="">— selecione —</option>
              {perfis.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.nome}
                </option>
              ))}
            </select>
          </label>

          <label style={{ display: "flex", alignItems: "center", gap: "0.35rem" }}>
            <input
              type="checkbox"
              checked={incluirSistema}
              onChange={(e) => setIncluirSistema(e.target.checked)}
            />
            Incluir objetos de sistema
          </label>

          <input
            type="search"
            placeholder="Filtrar objetos…"
            value={buscaDigitada}
            onChange={(e) => setBuscaDigitada(e.target.value)}
            style={estilos.entrada}
            aria-label="Filtro de busca"
          />

          <button type="button" style={estilos.botaoPrimario} onClick={aoAtualizarTudo} disabled={perfilId === ""}>
            Atualizar tudo
          </button>

          <button type="button" style={estilos.botao} onClick={() => void carregarPerfisConectados()}>
            Atualizar perfis
          </button>
        </div>
      </div>

      <p role="status" aria-live="polite" style={{ margin: 0, minHeight: "1.2rem" }}>
        {mensagem}
      </p>

      {perfis.length === 0 && (
        <div role="status" style={estilos.cartao}>
          <p style={{ margin: 0 }}>Nenhum perfil conectado.</p>
          <p style={{ margin: "0.25rem 0 0", fontSize: "0.85rem", color: "#4a4d52" }}>
            Conecte-se em um perfil na aba "Conexões" para explorar o banco.
          </p>
        </div>
      )}

      {perfilId !== "" && (
        <div style={estilos.cartao}>
          {carregandoRaiz && (
            <div role="status" aria-live="polite">Carregando catálogo…</div>
          )}
          {erroRaiz && (
            <div role="alert" style={estilos.aviso}>
              <p style={{ margin: "0 0 0.5rem" }}>Não foi possível carregar o catálogo.</p>
              <p style={{ margin: "0 0 0.5rem" }}>
                {ROTULO_CODIGO_ERRO[erroRaiz.code]}: {erroRaiz.message}
              </p>
              <button type="button" style={estilos.botao} onClick={() => void carregarRaiz(false)}>
                Tentar novamente
              </button>
            </div>
          )}
          {raiz && !carregandoRaiz && !erroRaiz && (
            <div
              ref={containerRef}
              role="tree"
              aria-label="Explorador de banco de dados"
              onKeyDown={aoTeclado}
            >
              {raiz.children
                ? raiz.children.map((filho) => (
                    <TreeNode
                      key={`${chaveArvore}-${filho.nodeId}`}
                      node={filho}
                      perfilId={perfilId}
                      incluirSistema={incluirSistema}
                      filtro={buscaAplicada}
                      nivel={1}
                      onMensagem={setMensagem}
                    />
                  ))
                : (
                    <TreeNode
                      key={`${chaveArvore}-${raiz.nodeId}`}
                      node={raiz}
                      perfilId={perfilId}
                      incluirSistema={incluirSistema}
                      filtro={buscaAplicada}
                      nivel={1}
                      onMensagem={setMensagem}
                    />
                  )}
              {raiz.children && raiz.children.length === 0 && (
                <p role="status" style={{ margin: "0.25rem 0", fontSize: "0.85rem", color: "#4a4d52" }}>
                  Nenhum item encontrado.
                </p>
              )}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
