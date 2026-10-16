import { useCallback, useEffect, useState, type JSX } from "react";

import {
  aceitarSql,
  apagarProvedor,
  gerar,
  listarInteracoes,
  listarProvedores,
  listarTemplates,
  previewContexto,
  salvarProvedor,
  testarProvedor,
  type IaContextPreview,
  type IaGenerate,
  type IaInteraction,
  type IaTemplate,
} from "./api";
import { listarPerfis, consultarStatus } from "../connections/api";
import type { ConnectionProfile, ConnectionStatusInfo } from "../connections/types";
import type { ProvedorIaDto } from "./types";

type Estado = "carregando" | "pronto" | "erro";

/**
 * Passo 1 da Fase 3 — configuração de provedores de IA.
 *
 * A chave da API é enviada **uma única vez** no formulário e vai direto para o
 * cofre do sistema: a lista mostra apenas se existe chave, nunca o valor.
 */
export default function AiAssistantPanel({ onInserirSql }: { onInserirSql?: (sql: string, interactionId: string) => void } = {}): JSX.Element {
  const [estado, setEstado] = useState<Estado>("carregando");
  const [mensagem, setMensagem] = useState("");
  const [erro, setErro] = useState<string | null>(null);
  const [provedores, setProvedores] = useState<ProvedorIaDto[]>([]);
  const [salvando, setSalvando] = useState(false);
  const [testandoId, setTestandoId] = useState<string | null>(null);
  const [templates, setTemplates] = useState<IaTemplate[]>([]);
  const [perfis, setPerfis] = useState<ConnectionProfile[]>([]);
  const [statusPorPerfil, setStatusPorPerfil] = useState<Record<string, ConnectionStatusInfo>>({});
  const [perfilContexto, setPerfilContexto] = useState("");
  const [contexto, setContexto] = useState<IaContextPreview | null>(null);
  const [gerandoContexto, setGerandoContexto] = useState(false);
  const [provedorEscolhido, setProvedorEscolhido] = useState("");
  const [tarefa, setTarefa] = useState("gerar_sql");
  const [pedido, setPedido] = useState("");
  const [resposta, setResposta] = useState<IaGenerate | null>(null);
  const [gerando, setGerando] = useState(false);
  const [historico, setHistorico] = useState<IaInteraction[]>([]);

  const [form, setForm] = useState({
    id: "",
    nome: "",
    tipo: "openai_compat",
    baseUrl: "http://localhost:11434/v1",
    modelo: "",
    apiKey: "",
    consentirContextoRemoto: false,
  });

  const carregar = useCallback(async () => {
    setEstado("carregando");
    try {
      setProvedores(await listarProvedores());
      setEstado("pronto");
    } catch (e: any) {
      setErro(e?.message ?? String(e));
      setEstado("erro");
    }
  }, []);

  useEffect(() => {
    void carregar();
    void listarTemplates().then(setTemplates).catch(() => setTemplates([]));
    void (async () => {
      try {
        const lista = await listarPerfis();
        setPerfis(lista);
        const status: Record<string, ConnectionStatusInfo> = {};
        for (const p of lista) {
          try { status[p.id] = await consultarStatus(p.id); } catch { /* ignora */ }
        }
        setStatusPorPerfil(status);
        const conectado = lista.find((p) => status[p.id]?.status === "connected");
        if (conectado) setPerfilContexto(conectado.id);
      } catch { /* ignora */ }
    })();
  }, [carregar]);

  const carregarHistorico = useCallback(async () => {
    try { setHistorico(await listarInteracoes(20)); } catch { setHistorico([]); }
  }, []);

  useEffect(() => {
    void carregarHistorico();
  }, [carregarHistorico]);

  const pedir = useCallback(async () => {
    if (!perfilContexto || provedorEscolhido === "") {
      setErro("Selecione a conexão e o provedor antes de pedir.");
      return;
    }
    setGerando(true);
    setErro(null);
    setResposta(null);
    try {
      const r = await gerar(perfilContexto, provedorEscolhido, tarefa, pedido);
      setResposta(r);
      await carregarHistorico();
    } catch (e: any) {
      setErro(e?.message ?? String(e));
    } finally {
      setGerando(false);
    }
  }, [perfilContexto, provedorEscolhido, tarefa, pedido, carregarHistorico]);

  const inserirSugestao = useCallback(
    async (sql: string, interactionId: string) => {
      try {
        await aceitarSql(interactionId, sql);
        onInserirSql?.(sql, interactionId);
        setMensagem("SQL inserido no editor (sugestão — nada foi executado).");
      } catch (e: any) {
        setErro(e?.message ?? String(e));
      }
    },
    [onInserirSql],
  );

  const gerarContexto = useCallback(async () => {
    if (!perfilContexto) return;
    setGerandoContexto(true);
    setErro(null);
    try {
      setContexto(await previewContexto(perfilContexto));
    } catch (e: any) {
      setErro(e?.message ?? String(e));
    } finally {
      setGerandoContexto(false);
    }
  }, [perfilContexto]);

  const salvar = useCallback(async () => {
    setSalvando(true);
    setErro(null);
    setMensagem("");
    try {
      await salvarProvedor({
        id: form.id || undefined,
        nome: form.nome,
        tipo: form.tipo,
        baseUrl: form.baseUrl,
        modelo: form.modelo,
        apiKey: form.apiKey || undefined,
        consentirContextoRemoto: form.consentirContextoRemoto,
      });
      setForm({ ...form, id: "", apiKey: "", nome: "", modelo: "" });
      setMensagem("Provedor salvo. A chave, se informada, foi para o cofre do sistema.");
      await carregar();
    } catch (e: any) {
      setErro(e?.message ?? String(e));
    } finally {
      setSalvando(false);
    }
  }, [form, carregar]);

  const testar = useCallback(async (id: string) => {
    setTestandoId(id);
    setErro(null);
    try {
      setMensagem(await testarProvedor(id));
    } catch (e: any) {
      setErro(e?.message ?? String(e));
    } finally {
      setTestandoId(null);
    }
  }, []);

  const apagar = useCallback(
    async (id: string) => {
      try {
        await apagarProvedor(id);
        setMensagem("Provedor removido (perfil e chave do cofre).");
        await carregar();
      } catch (e: any) {
        setErro(e?.message ?? String(e));
      }
    },
    [carregar],
  );

  const estilos = {
    painel: { display: "flex", flexDirection: "column" as const, gap: "1rem", maxWidth: "60rem" },
    cartao: { background: "#ffffff", border: "1px solid #d9dce1", borderRadius: "8px", padding: "1rem" },
    linha: { display: "flex", alignItems: "center", gap: "0.6rem", flexWrap: "wrap" as const, marginBottom: "0.5rem" },
    entrada: { padding: "0.4rem 0.5rem", border: "1px solid #b7bcc4", borderRadius: "4px", minWidth: "14rem" },
    rotulo: { display: "flex", flexDirection: "column" as const, gap: "0.2rem", fontSize: "0.85rem" },
    botao: { padding: "0.4rem 0.8rem", cursor: "pointer", borderRadius: "4px", border: "1px solid #b7bcc4", background: "#ffffff" },
    botaoPrimario: { padding: "0.4rem 0.9rem", cursor: "pointer", borderRadius: "4px", border: "1px solid #1b5e9c", background: "#1b5e9c", color: "#ffffff" },
    aviso: { padding: "0.75rem 1rem", borderRadius: "6px", background: "#eef6ff", color: "#123a5c" },
    erro: { padding: "0.75rem 1rem", borderRadius: "6px", background: "#fdf2f2", color: "#8a1c22" },
  };

  return (
    <section style={estilos.painel} aria-labelledby="ia-titulo">
      <h2 id="ia-titulo" style={{ margin: 0, fontSize: "1.15rem" }}>
        Assistente de IA — provedores
      </h2>
      <p style={{ margin: 0, fontSize: "0.9rem", color: "#4a4d52" }}>
        Configure um provedor compatível com a API da OpenAI (Ollama, LM Studio, vLLM ou
        OpenAI). A chave de API fica no cofre do sistema — nunca no banco local nem na tela.
      </p>

      {erro && <div role="alert" style={estilos.erro}>{erro}</div>}
      {mensagem && <div role="status" style={estilos.aviso}>{mensagem}</div>}

      <div style={estilos.cartao}>
        <h3 style={{ marginTop: 0, fontSize: "1rem" }}>{form.id ? "Editar provedor" : "Novo provedor"}</h3>
        <div style={estilos.linha}>
          <label style={estilos.rotulo}>
            Nome
            <input
              style={estilos.entrada}
              value={form.nome}
              onChange={(e) => setForm({ ...form, nome: e.target.value })}
              placeholder="Ollama local"
            />
          </label>
          <label style={estilos.rotulo}>
            URL base
            <input
              style={estilos.entrada}
              value={form.baseUrl}
              onChange={(e) => setForm({ ...form, baseUrl: e.target.value })}
              placeholder="http://localhost:11434/v1"
            />
          </label>
        </div>
        <div style={estilos.linha}>
          <label style={estilos.rotulo}>
            Modelo
            <input
              style={estilos.entrada}
              value={form.modelo}
              onChange={(e) => setForm({ ...form, modelo: e.target.value })}
              placeholder="qwen2.5-coder"
            />
          </label>
          <label style={estilos.rotulo}>
            Chave da API (fica no cofre)
            <input
              style={estilos.entrada}
              type="password"
              value={form.apiKey}
              onChange={(e) => setForm({ ...form, apiKey: e.target.value })}
              placeholder={form.id ? "deixe vazio para manter a atual" : "sk-…"}
              autoComplete="off"
            />
          </label>
        </div>
        <div style={estilos.linha}>
          <label style={{ display: "flex", alignItems: "center", gap: "0.4rem", fontSize: "0.85rem" }}>
            <input
              type="checkbox"
              checked={form.consentirContextoRemoto}
              onChange={(e) => setForm({ ...form, consentirContextoRemoto: e.target.checked })}
            />
            Autorizo enviar o esquema do banco para este provedor remoto
          </label>
          <button type="button" style={estilos.botaoPrimario} onClick={() => void salvar()} disabled={salvando}>
            {salvando ? "Salvando…" : "Salvar provedor"}
          </button>
          {form.id && (
            <button type="button" style={estilos.botao} onClick={() => setForm({ ...form, id: "", apiKey: "" })}>
              Cancelar edição
            </button>
          )}
        </div>
      </div>

      <div style={estilos.cartao}>
        <h3 style={{ marginTop: 0, fontSize: "1rem" }}>Provedores configurados</h3>
        {estado === "carregando" && <div role="status">Carregando provedores…</div>}
        {estado === "erro" && (
          <button type="button" style={estilos.botao} onClick={() => void carregar()}>
            Tentar novamente
          </button>
        )}
        {estado === "pronto" && provedores.length === 0 && (
          <p role="status" style={{ margin: 0, color: "#4a4d52" }}>
            Nenhum provedor configurado ainda.
          </p>
        )}
        {estado === "pronto" && provedores.length > 0 && (
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {provedores.map((p) => (
              <li key={p.id} style={{ borderBottom: "1px solid #eee", padding: "0.5rem 0" }}>
                <div style={estilos.linha}>
                  <strong>{p.name}</strong>
                  <span style={{ color: "#4a4d52", fontSize: "0.85rem" }}>
                    {p.providerType} · {p.baseUrl} · {p.modelName}
                  </span>
                  {p.hasApiKey && <span style={{ fontSize: "0.75rem" }}>🔑 chave no cofre</span>}
                  {p.remoteContextConfirmation && (
                    <span style={{ fontSize: "0.75rem" }}>contexto remoto autorizado</span>
                  )}
                </div>
                <div style={estilos.linha}>
                  <button type="button" style={estilos.botao} onClick={() => void testar(p.id)} disabled={testandoId === p.id}>
                    {testandoId === p.id ? "Testando…" : "Testar"}
                  </button>
                  <button
                    type="button"
                    style={estilos.botao}
                    onClick={() => setForm({ id: p.id, nome: p.name, tipo: p.providerType, baseUrl: p.baseUrl, modelo: p.modelName, apiKey: "", consentirContextoRemoto: p.remoteContextConfirmation })}
                  >
                    Editar
                  </button>
                  <button type="button" style={estilos.botao} onClick={() => void apagar(p.id)}>
                    Remover
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div style={estilos.cartao}>
        <h3 style={{ marginTop: 0, fontSize: "1rem" }}>Contexto do schema (pré-visualização)</h3>
        <div style={estilos.linha}>
          <label style={estilos.rotulo}>
            Conexão
            <select
              style={estilos.entrada}
              value={perfilContexto}
              onChange={(e) => setPerfilContexto(e.target.value)}
            >
              <option value="">— selecione —</option>
              {perfis
                .filter((p) => statusPorPerfil[p.id]?.status === "connected")
                .map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
            </select>
          </label>
          <button
            type="button"
            style={estilos.botaoPrimario}
            onClick={() => void gerarContexto()}
            disabled={gerandoContexto || perfilContexto === ""}
          >
            {gerandoContexto ? "Montando…" : "Pré-visualizar contexto"}
          </button>
        </div>
        {contexto && (
          <>
            <p style={{ margin: "0.25rem 0", fontSize: "0.85rem", color: "#4a4d52" }}>
              {contexto.relacoes} relação(ões)
              {contexto.relacoesOmitidas > 0 && ` · ${contexto.relacoesOmitidas} omitida(s) pelo limite`}
              {contexto.truncado && " · contexto truncado"}
            </p>
            <pre
              style={{
                margin: 0,
                maxHeight: "18rem",
                overflow: "auto",
                background: "#f4f5f7",
                border: "1px solid #d9dce1",
                borderRadius: "4px",
                padding: "0.6rem",
                fontSize: "0.8rem",
                whiteSpace: "pre-wrap",
              }}
            >
              {contexto.text}
            </pre>
            <p style={{ fontSize: "0.8rem", color: "#4a4d52" }}>
              Este comando <strong>não envia nada</strong>: é só o texto que iria ao provedor, com
              apenas metadados (nomes, tipos e chaves) — nunca dados das tabelas.
            </p>
          </>
        )}
      </div>

      <div style={estilos.cartao}>
        <h3 style={{ marginTop: 0, fontSize: "1rem" }}>Assistente (geração, explicação, correção, revisão)</h3>
        <div style={estilos.linha}>
          <label style={estilos.rotulo}>
            Provedor
            <select
              style={estilos.entrada}
              value={provedorEscolhido}
              onChange={(e) => setProvedorEscolhido(e.target.value)}
            >
              <option value="">— selecione —</option>
              {provedores.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} ({p.modelName})
                </option>
              ))}
            </select>
          </label>
          <label style={estilos.rotulo}>
            Tarefa
            <select style={estilos.entrada} value={tarefa} onChange={(e) => setTarefa(e.target.value)}>
              <option value="gerar_sql">Gerar SQL</option>
              <option value="explicar_sql">Explicar SQL</option>
              <option value="corrigir_sql">Corrigir SQL</option>
              <option value="revisar_sql">Revisar SQL</option>
            </select>
          </label>
        </div>
        <label style={estilos.rotulo}>
          Pedido (o SQL atual, se for explicar/corrigir/revisar)
          <textarea
            style={{ ...estilos.entrada, minHeight: "5rem", width: "100%" }}
            value={pedido}
            onChange={(e) => setPedido(e.target.value)}
            placeholder="ex.: liste os clientes com mais de 10 pedidos"
          />
        </label>
        <div style={{ ...estilos.linha, marginTop: "0.5rem" }}>
          <button
            type="button"
            style={estilos.botaoPrimario}
            onClick={() => void pedir()}
            disabled={gerando || provedorEscolhido === "" || pedido.trim() === ""}
          >
            {gerando ? "Gerando…" : "Pedir ao assistente"}
          </button>
        </div>

        {resposta && (
          <div style={{ marginTop: "0.75rem" }}>
            <p style={{ margin: "0 0 0.35rem", fontSize: "0.85rem", color: "#4a4d52" }}>
              Modelo {resposta.modelo} · contexto com {resposta.contextoRelacoes} relação(ões)
              {resposta.contextoTruncado && " (truncado)"}
              {resposta.contextoRemoto && " · enviado a provedor remoto autorizado"}
            </p>
            <pre
              style={{
                margin: 0,
                background: "#f4f5f7",
                border: "1px solid #d9dce1",
                borderRadius: "4px",
                padding: "0.6rem",
                fontSize: "0.85rem",
                whiteSpace: "pre-wrap",
              }}
            >
              {resposta.texto}
            </pre>
            {resposta.sql && (
              <button
                type="button"
                style={{ ...estilos.botao, marginTop: "0.5rem" }}
                onClick={() => void inserirSugestao(resposta.sql as string, resposta.interactionId)}
              >
                Inserir SQL sugerido no editor (não executa)
              </button>
            )}
          </div>
        )}
      </div>

      <div style={estilos.cartao}>
        <h3 style={{ marginTop: 0, fontSize: "1rem" }}>Histórico do assistente</h3>
        <button type="button" style={estilos.botao} onClick={() => void carregarHistorico()}>
          Atualizar
        </button>
        {historico.length === 0 ? (
          <p role="status" style={{ margin: "0.5rem 0 0", color: "#4a4d52" }}>
            Nenhuma interação registrada.
          </p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: "0.5rem 0 0", fontSize: "0.85rem" }}>
            {historico.map((h) => (
              <li key={h.id} style={{ borderBottom: "1px solid #eee", padding: "0.4rem 0" }}>
                <strong>{h.taskType}</strong>{" "}
                <span style={{ fontSize: "0.75rem", color: "#4a4d52" }}>{h.createdAt}</span>
                {h.acceptedSql && <span style={{ fontSize: "0.75rem", color: "#217a3b" }}> · SQL aceito</span>}
                <div style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
                  {h.responseText.slice(0, 220)}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div style={estilos.cartao}>
        <h3 style={{ marginTop: 0, fontSize: "1rem" }}>Templates de prompt (versionados)</h3>
        {templates.length === 0 ? (
          <p role="status" style={{ margin: 0, color: "#4a4d52" }}>Carregando templates…</p>
        ) : (
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {templates.map((t) => (
              <li key={t.tarefa} style={{ borderBottom: "1px solid #eee", padding: "0.4rem 0" }}>
                <strong>{t.rotulo}</strong> <span style={{ fontSize: "0.75rem" }}>v{t.versao}</span>
                <div style={{ fontSize: "0.85rem", color: "#4a4d52" }}>{t.instrucao}</div>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div style={estilos.aviso}>
        <strong>Próximos passos (Fase 3):</strong> contexto do schema, templates versionados,
        geração/explicação/correção, revisão de SQL com classificação de risco. Nenhum SQL
        gerado pela IA é executado automaticamente.
      </div>
    </section>
  );
}
