import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useState,
  type FormEvent,
  type InputHTMLAttributes,
  type JSX,
} from "react";

import {
  abrirConexao,
  consultarStatus,
  fecharConexao,
  listarPerfis,
  normalizarErro,
  salvarPerfil,
  testarPerfil,
} from "./api";
import {
  ROTULO_CODIGO_ERRO,
  ROTULO_STATUS,
  type CommandError,
  type ConnectionProfile,
  type ConnectionProfileInput,
  type ConnectionStatus,
  type SqlDialect,
} from "./types";

/* ------------------------------------------------------------------ */
/* Estilos (inline — o scaffold não usa CSS externo nem libs de UI)     */
/* ------------------------------------------------------------------ */

const estilos = {
  painel: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "1.25rem",
  },
  cartao: {
    background: "#ffffff",
    border: "1px solid #d9dce1",
    borderRadius: "8px",
    padding: "1rem 1.25rem",
  },
  tabela: {
    width: "100%",
    borderCollapse: "collapse" as const,
    fontSize: "0.9rem",
  },
  th: {
    textAlign: "left" as const,
    padding: "0.5rem",
    borderBottom: "2px solid #d9dce1",
    whiteSpace: "nowrap" as const,
  },
  td: {
    padding: "0.5rem",
    borderBottom: "1px solid #eceef1",
    verticalAlign: "top" as const,
  },
  campo: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "0.25rem",
    fontSize: "0.85rem",
  },
  entrada: {
    padding: "0.4rem 0.5rem",
    fontSize: "0.9rem",
    border: "1px solid #b7bcc4",
    borderRadius: "4px",
  },
  grade: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(13rem, 1fr))",
    gap: "0.75rem",
  },
  botao: {
    padding: "0.35rem 0.7rem",
    fontSize: "0.85rem",
    cursor: "pointer",
    borderRadius: "4px",
    border: "1px solid #b7bcc4",
    background: "#ffffff",
  },
  botaoPrimario: {
    padding: "0.45rem 0.9rem",
    fontSize: "0.9rem",
    cursor: "pointer",
    borderRadius: "4px",
    border: "1px solid #1b5e9c",
    background: "#1b5e9c",
    color: "#ffffff",
  },
  aviso: {
    padding: "0.75rem 1rem",
    borderRadius: "6px",
    border: "1px solid #e2b8b8",
    background: "#fdf2f2",
    color: "#8a1c22",
  },
} as const;

/* ------------------------------------------------------------------ */
/* Tipos internos de estado                                             */
/* ------------------------------------------------------------------ */

type EstadoLista =
  | { fase: "carregando" }
  | { fase: "pronto"; perfis: ConnectionProfile[] }
  | { fase: "erro"; erro: CommandError };

type AcaoPerfil = "testar" | "conectar" | "desconectar";

type Formulario = {
  id?: string;
  nome: string;
  host: string;
  porta: string;
  banco: string;
  usuario: string;
  role: string;
  charset: string;
  dialeto: "1" | "3";
  timeout: string;
  caminhoBiblioteca: string;
  /**
   * Senha usada **somente** na ação "Conectar". Nunca entra em
   * `ConnectionProfileInput`, nunca é persistida e é apagada após o uso.
   */
  senha: string;
};

const FORMULARIO_VAZIO: Formulario = {
  nome: "",
  host: "localhost",
  porta: "3050",
  banco: "",
  usuario: "",
  role: "",
  charset: "UTF8",
  dialeto: "3",
  timeout: "15000",
  caminhoBiblioteca: "",
  senha: "",
};

const ROTULO_ACAO: Record<AcaoPerfil, string> = {
  testar: "Testando conexão…",
  conectar: "Conectando…",
  desconectar: "Desconectando…",
};

/* ------------------------------------------------------------------ */
/* Formulário de criação/edição                                          */
/* ------------------------------------------------------------------ */

export function FormularioPerfil(props: {
  inicial: ConnectionProfile | null;
  aoSalvar: (entrada: ConnectionProfileInput, senha: string) => Promise<void>;
  aoCancelar: () => void;
  salvando: boolean;
}): JSX.Element {
  const { inicial, aoSalvar, aoCancelar, salvando } = props;
  const idBase = useId();

  const [form, setForm] = useState<Formulario>(() =>
    inicial
      ? {
          id: inicial.id,
          nome: inicial.name,
          host: inicial.host,
          porta: String(inicial.port),
          banco: inicial.databasePath,
          usuario: inicial.username,
          role: inicial.roleName ?? "",
          charset: inicial.charset,
          dialeto: String(inicial.sqlDialect ?? 3) as "1" | "3",
          timeout: String(inicial.connectTimeoutMs ?? 15000),
          caminhoBiblioteca: inicial.clientLibraryPath ?? "",
          senha: "",
        }
      : FORMULARIO_VAZIO,
  );
  const [erros, setErros] = useState<Record<string, string>>({});

  // Todos os campos editáveis do formulário são textos; `id` nunca é editado.
  const definir = (campo: keyof Formulario, valor: string) =>
    setForm((atual) => ({ ...atual, [campo]: valor }) as Formulario);

  function validar(): { erros: Record<string, string>; entrada: ConnectionProfileInput } {
    const novos: Record<string, string> = {};
    if (form.nome.trim().length === 0) novos.nome = "Informe o nome do perfil.";
    if (form.host.trim().length === 0) novos.host = "Informe o host do servidor.";
    const porta = Number(form.porta);
    if (!Number.isInteger(porta) || porta < 1 || porta > 65535)
      novos.porta = "Porta deve ser um número entre 1 e 65535.";
    if (form.banco.trim().length === 0)
      novos.banco = "Informe o caminho do banco ou o alias.";
    if (form.usuario.trim().length === 0) novos.usuario = "Informe o usuário.";
    const timeout = Number(form.timeout);
    if (!Number.isInteger(timeout) || timeout < 0)
      novos.timeout = "Timeout deve ser um número de milissegundos (0 ou maior).";

    if (Object.keys(novos).length > 0) return { erros: novos, entrada: semEntrada(form) };

    return {
      erros: {},
      entrada: {
        ...(form.id === undefined ? {} : { id: form.id }),
        name: form.nome.trim(),
        host: form.host.trim(),
        port: porta,
        databasePath: form.banco.trim(),
        username: form.usuario.trim(),
        charset: form.charset.trim() || "UTF8",
        sqlDialect: (Number(form.dialeto) === 1 ? 1 : 3) as SqlDialect,
        connectTimeoutMs: timeout,
        ...(form.role.trim() ? { roleName: form.role.trim() } : {}),
        ...(form.caminhoBiblioteca.trim()
          ? { clientLibraryPath: form.caminhoBiblioteca.trim() }
          : {}),
      },
    };
  }

  async function aoEnviar(evento: FormEvent<HTMLFormElement>): Promise<void> {
    evento.preventDefault();
    const { erros: novosErros, entrada } = validar();
    setErros(novosErros);
    if (Object.keys(novosErros).length > 0) {
      const primeiro = document.getElementById(`${idBase}-${Object.keys(novosErros)[0]}`);
      primeiro?.focus();
      return;
    }
    await aoSalvar(entrada, form.senha);
    // A senha nunca é guardada em estado persistente do perfil.
    definir("senha", "");
  }

  const campo = (
    chave: string,
    rotulo: string,
    no: keyof Formulario,
    propsInput: InputHTMLAttributes<HTMLInputElement> = {},
  ) => (
    <div style={estilos.campo} key={chave}>
      <label htmlFor={`${idBase}-${chave}`}>{rotulo}</label>
      <input
        id={`${idBase}-${chave}`}
        style={estilos.entrada}
        value={String(form[no] ?? "")}
        onChange={(e) => {
          definir(no, e.target.value);
          setErros((atuais) => {
            if (!(chave in atuais)) return atuais;
            const copia = { ...atuais };
            delete copia[chave];
            return copia;
          });
        }}
        {...propsInput}
      />
      {erros[chave] && (
        <span role="alert" style={{ color: "#a4262c" }}>
          {erros[chave]}
        </span>
      )}
    </div>
  );

  return (
    <form style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }} onSubmit={(e) => void aoEnviar(e)} noValidate>
      <h3 style={{ margin: 0, fontSize: "1rem" }}>
        {inicial ? `Editar perfil: ${inicial.name}` : "Novo perfil de conexão"}
      </h3>

      <div style={estilos.grade}>
        {campo("nome", "Nome do perfil", "nome", { required: true })}
        {campo("host", "Host", "host", { required: true, autoComplete: "off" })}
        {campo("porta", "Porta", "porta", { type: "number", min: 1, max: 65535 })}
        {campo(
          "banco",
          "Banco de dados (caminho ou alias)",
          "banco",
          { required: true, autoComplete: "off" },
        )}
        {campo("usuario", "Usuário", "usuario", {
          required: true,
          autoComplete: "off",
        })}
        {campo("role", "Role (opcional)", "role", { autoComplete: "off" })}
        {campo("charset", "Charset", "charset", { autoComplete: "off" })}
        <div style={estilos.campo}>
          <label htmlFor={`${idBase}-dialeto`}>Dialeto SQL</label>
          <select
            id={`${idBase}-dialeto`}
            style={estilos.entrada}
            value={form.dialeto}
            onChange={(e) => definir("dialeto", e.target.value)}
          >
            <option value="1">1 (Firebird 2.0)</option>
            <option value="3">3 (Firebird 3.0 ou superior)</option>
          </select>
        </div>
        {campo("timeout", "Timeout de conexão (ms)", "timeout", { type: "number", min: 0 })}
        {campo(
          "caminhoBiblioteca",
          "Caminho da biblioteca cliente (opcional)",
          "caminhoBiblioteca",
          { autoComplete: "off" },
        )}
        {campo("senha", "Senha (somente para conectar — nunca é salva)", "senha", {
          type: "password",
          autoComplete: "current-password",
        })}
      </div>

      <p style={{ margin: 0, fontSize: "0.8rem", color: "#4a4d52" }}>
        A senha é usada apenas no comando de conexão, que a entrega ao cofre de
        segredos do sistema. Ela não faz parte do perfil salvo.
      </p>

      <div style={{ display: "flex", gap: "0.5rem" }}>
        <button type="submit" style={estilos.botaoPrimario} disabled={salvando}>
          {salvando ? "Salvando…" : inicial ? "Salvar alterações" : "Salvar perfil"}
        </button>
        <button type="button" style={estilos.botao} onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

/** Perfil inexistente usado apenas para satisfazer o tipo ao falhar a validação. */
function semEntrada(form: Formulario): ConnectionProfileInput {
  return {
    name: form.nome,
    host: form.host,
    port: Number(form.porta) || 0,
    databasePath: form.banco,
    username: form.usuario,
    charset: form.charset,
  };
}

/* ------------------------------------------------------------------ */
/* Linha da tabela de perfis                                            */
/* ------------------------------------------------------------------ */

function LinhaPerfil(props: {
  perfil: ConnectionProfile;
  status: ConnectionStatus;
  statusInfo?: string;
  acao: AcaoPerfil | null;
  onTestar: () => void;
  onConectar: () => void;
  onDesconectar: () => void;
  onEditar: () => void;
  onAbrirSenha: () => void;
}): JSX.Element {
  const { perfil, status, statusInfo, acao, onTestar, onConectar, onDesconectar, onEditar, onAbrirSenha } =
    props;
  const conectado = status === "connected" || status === "reconnecting";
  const ocupado = acao !== null;

  return (
    <tr>
      <td style={estilos.td}>
        <strong>{perfil.name}</strong>
      </td>
      <td style={estilos.td}>
        {perfil.host}:{perfil.port}
      </td>
      <td style={estilos.td}>{perfil.databasePath}</td>
      <td style={estilos.td}>{perfil.username}</td>
      <td style={estilos.td}>{perfil.charset}</td>
      <td style={estilos.td}>{perfil.sqlDialect ?? 3}</td>
      <td style={estilos.td}>
        <span aria-live="polite">
          {ROTULO_STATUS[status]}
          {statusInfo ? ` — ${statusInfo}` : ""}
        </span>
      </td>
      <td style={{ ...estilos.td, display: "flex", flexWrap: "wrap", gap: "0.35rem" }}>
        <button
          type="button"
          style={estilos.botao}
          onClick={onTestar}
          disabled={ocupado}
          aria-label={`Testar conexão do perfil ${perfil.name}`}
        >
          {acao === "testar" ? ROTULO_ACAO.testar : "Testar conexão"}
        </button>
        {conectado ? (
          <button
            type="button"
            style={estilos.botao}
            onClick={onDesconectar}
            disabled={ocupado}
            aria-label={`Desconectar do perfil ${perfil.name}`}
          >
            {acao === "desconectar" ? ROTULO_ACAO.desconectar : "Desconectar"}
          </button>
        ) : (
          <button
            type="button"
            style={estilos.botao}
            onClick={onConectar}
            disabled={ocupado}
            aria-label={`Conectar ao perfil ${perfil.name}`}
          >
            {acao === "conectar" ? ROTULO_ACAO.conectar : "Conectar"}
          </button>
        )}
        <button
          type="button"
          style={estilos.botao}
          onClick={onAbrirSenha}
          disabled={ocupado}
        >
          Informar senha
        </button>
        <button type="button" style={estilos.botao} onClick={onEditar} disabled={ocupado}>
          Editar
        </button>
      </td>
    </tr>
  );
}

/* ------------------------------------------------------------------ */
/* Painel                                                               */
/* ------------------------------------------------------------------ */

/**
 * Painel do Gerenciador de Conexões (Spec 03, FR-001/FR-002).
 *
 * Estados completos: carregando, vazio ("Nenhum perfil salvo"), erro
 * acionável e lista. Todos os textos em pt-BR, com `aria-live` nas
 * atualizações de status e `role="alert"` nos erros.
 */
export default function ConnectionsPanel(): JSX.Element {
  const [lista, setLista] = useState<EstadoLista>({ fase: "carregando" });
  const [statusPorPerfil, setStatusPorPerfil] = useState<Record<string, ConnectionStatus>>({});
  const [detalhePorPerfil, setDetalhePorPerfil] = useState<Record<string, string>>({});
  const [acaoPorPerfil, setAcaoPorPerfil] = useState<Record<string, AcaoPerfil | null>>({});
  const [erroPorPerfil, setErroPorPerfil] = useState<Record<string, CommandError>>({});
  const [mensagem, setMensagem] = useState<string>("");
  const [editando, setEditando] = useState<ConnectionProfile | null>(null);
  const [formAberto, setFormAberto] = useState(false);
  const [salvando, setSalvando] = useState(false);
  const [senhaPorPerfil, setSenhaPorPerfil] = useState<Record<string, string>>({});
  const [idSenhaAberta, setIdSenhaAberta] = useState<string | null>(null);

  const carregar = useCallback(async () => {
    setLista({ fase: "carregando" });
    try {
      const perfis = await listarPerfis();
      setLista({ fase: "pronto", perfis });
      setStatusPorPerfil((atuais) => {
        const proximo: Record<string, ConnectionStatus> = {};
        for (const p of perfis) {
          proximo[p.id] = atuais[p.id] ?? "disconnected";
        }
        return proximo;
      });
      // Sincroniza o status real de cada sessão com o que o backend informa.
      await Promise.all(
        perfis.map(async (p) => {
          try {
            const info = await consultarStatus(p.id);
            setStatusPorPerfil((atuais) => ({ ...atuais, [p.id]: info.status }));
            if (info.clientLibraryPath) {
              setDetalhePorPerfil((atuais) => ({
                ...atuais,
                [p.id]: `biblioteca: ${info.clientLibraryPath}`,
              }));
            }
          } catch {
            // Perfil sem sessão ativa no backend: permanece desconectado.
            setStatusPorPerfil((atuais) => ({ ...atuais, [p.id]: "disconnected" }));
          }
        }),
      );
    } catch (erro) {
      setLista({ fase: "erro", erro: normalizarErro(erro) });
    }
  }, []);

  useEffect(() => {
    void carregar();
  }, [carregar]);

  const definirStatus = useCallback((profileId: string, status: ConnectionStatus) => {
    setStatusPorPerfil((atuais) => ({ ...atuais, [profileId]: status }));
  }, []);

  const definirDetalhe = useCallback((profileId: string, detalhe?: string) => {
    setDetalhePorPerfil((atuais) => {
      const proximo = { ...atuais };
      if (detalhe) proximo[profileId] = detalhe;
      else delete proximo[profileId];
      return proximo;
    });
  }, []);

  const iniciarAcao = (profileId: string, acao: AcaoPerfil) =>
    setAcaoPorPerfil((atuais) => ({ ...atuais, [profileId]: acao }));

  const encerrarAcao = (profileId: string) =>
    setAcaoPorPerfil((atuais) => ({ ...atuais, [profileId]: null }));

  const registrarErro = (profileId: string, erro: CommandError) =>
    setErroPorPerfil((atuais) => ({ ...atuais, [profileId]: erro }));

  const limparErro = (profileId: string) =>
    setErroPorPerfil((atuais) => {
      if (!(profileId in atuais)) return atuais;
      const copia = { ...atuais };
      delete copia[profileId];
      return copia;
    });

  /** Testar conexão: não abre sessão nem altera o perfil salvo (Spec 03 §1). */
  async function aoTestar(perfil: ConnectionProfile): Promise<void> {
    iniciarAcao(perfil.id, "testar");
    limparErro(perfil.id);
    setMensagem(`Testando conexão com "${perfil.name}"…`);
    try {
      const resultado = await testarPerfil({
        id: perfil.id,
        name: perfil.name,
        host: perfil.host,
        port: perfil.port,
        databasePath: perfil.databasePath,
        username: perfil.username,
        charset: perfil.charset,
        ...(perfil.roleName ? { roleName: perfil.roleName } : {}),
        ...(perfil.sqlDialect ? { sqlDialect: perfil.sqlDialect } : {}),
        ...(perfil.clientLibraryPath ? { clientLibraryPath: perfil.clientLibraryPath } : {}),
        ...(perfil.connectTimeoutMs !== undefined
          ? { connectTimeoutMs: perfil.connectTimeoutMs }
          : {}),
      });
      definirDetalhe(
        perfil.id,
        resultado.clientLibraryPath
          ? `biblioteca: ${resultado.clientLibraryPath}`
          : resultado.serverVersion,
      );
      setMensagem(
        resultado.ok
          ? `Conexão com "${perfil.name}" testada com sucesso.`
          : (resultado.message ?? `Falha ao testar "${perfil.name}".`),
      );
    } catch (erro) {
      const cmd = normalizarErro(erro);
      registrarErro(perfil.id, cmd);
      definirStatus(perfil.id, "error");
      definirDetalhe(perfil.id, cmd.message);
      setMensagem(`Não foi possível testar "${perfil.name}".`);
    } finally {
      encerrarAcao(perfil.id);
    }
  }

  /** Conectar: único fluxo que envia a senha — sempre via `connection_open`. */
  async function aoConectar(perfil: ConnectionProfile): Promise<void> {
    const senha = senhaPorPerfil[perfil.id] ?? "";
    iniciarAcao(perfil.id, "conectar");
    limparErro(perfil.id);
    definirStatus(perfil.id, "connecting");
    definirDetalhe(perfil.id);
    setMensagem(`Conectando a "${perfil.name}"…`);
    try {
      const resultado = await abrirConexao({ profileId: perfil.id, password: senha });
      definirStatus(perfil.id, resultado.status ?? "connected");
      definirDetalhe(
        perfil.id,
        resultado.clientLibraryPath ? `biblioteca: ${resultado.clientLibraryPath}` : undefined,
      );
      // A senha é descartada da memória da UI assim que o cofre a recebe.
      setSenhaPorPerfil((atuais) => {
        const copia = { ...atuais };
        delete copia[perfil.id];
        return copia;
      });
      setIdSenhaAberta(null);
      setMensagem(`Conectado a "${perfil.name}".`);
    } catch (erro) {
      const cmd = normalizarErro(erro);
      registrarErro(perfil.id, cmd);
      definirStatus(perfil.id, "error");
      definirDetalhe(perfil.id, cmd.message);
      setMensagem(`Não foi possível conectar a "${perfil.name}".`);
    } finally {
      encerrarAcao(perfil.id);
    }
  }

  async function aoDesconectar(perfil: ConnectionProfile): Promise<void> {
    iniciarAcao(perfil.id, "desconectar");
    limparErro(perfil.id);
    definirStatus(perfil.id, "reconnecting");
    setMensagem(`Desconectando de "${perfil.name}"…`);
    try {
      await fecharConexao(perfil.id);
      definirStatus(perfil.id, "disconnected");
      definirDetalhe(perfil.id);
      setMensagem(`Desconectado de "${perfil.name}".`);
    } catch (erro) {
      const cmd = normalizarErro(erro);
      registrarErro(perfil.id, cmd);
      definirStatus(perfil.id, "error");
      definirDetalhe(perfil.id, cmd.message);
      setMensagem(`Não foi possível desconectar de "${perfil.name}".`);
    } finally {
      encerrarAcao(perfil.id);
    }
  }

  async function aoSalvar(entrada: ConnectionProfileInput, _senha: string): Promise<void> {
    setSalvando(true);
    try {
      await salvarPerfil(entrada);
      setMensagem(
        entrada.id === undefined
          ? "Perfil salvo. Use “Conectar” para abrir uma sessão."
          : "Perfil atualizado.",
      );
      setFormAberto(false);
      setEditando(null);
      await carregar();
    } catch (erro) {
      const cmd = normalizarErro(erro);
      setMensagem(`${ROTULO_CODIGO_ERRO[cmd.code]}: ${cmd.message}`);
    } finally {
      setSalvando(false);
    }
  }

  const perfis = useMemo(
    () => (lista.fase === "pronto" ? lista.perfis : []),
    [lista],
  );

  return (
    <section style={estilos.painel} aria-labelledby="conexoes-titulo">
      <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
        <h2 id="conexoes-titulo" style={{ margin: 0, fontSize: "1.15rem" }}>
          Gerenciador de conexões
        </h2>
        <button
          type="button"
          style={estilos.botaoPrimario}
          onClick={() => {
            setEditando(null);
            setFormAberto((aberto) => !aberto);
          }}
        >
          {formAberto ? "Fechar formulário" : "Novo perfil"}
        </button>
        <button type="button" style={estilos.botao} onClick={() => void carregar()}>
          Atualizar lista
        </button>
      </div>

      {/* Canal único de anúncios: teclado e leitores de tela percebem as mudanças. */}
      <p role="status" aria-live="polite" style={{ margin: 0, minHeight: "1.2rem" }}>
        {mensagem}
      </p>

      {lista.fase === "carregando" && (
        <div role="status" aria-live="polite">
          Carregando perfis de conexão…
        </div>
      )}

      {lista.fase === "erro" && (
        <div role="alert" style={estilos.aviso}>
          <p style={{ margin: "0 0 0.5rem" }}>
            Não foi possível carregar os perfis de conexão.
          </p>
          <p style={{ margin: "0 0 0.5rem" }}>
            {ROTULO_CODIGO_ERRO[lista.erro.code]}: {lista.erro.message}
          </p>
          <p style={{ margin: "0 0 0.5rem" }}>
            Verifique se o backend está em execução e tente novamente.
          </p>
          <button type="button" style={estilos.botao} onClick={() => void carregar()}>
            Tentar novamente
          </button>
        </div>
      )}

      {lista.fase === "pronto" && perfis.length === 0 && (
        <div role="status" style={estilos.cartao}>
          <p style={{ margin: 0 }}>Nenhum perfil salvo</p>
          <p style={{ margin: "0.25rem 0 0", fontSize: "0.85rem", color: "#4a4d52" }}>
            Crie um perfil com host, porta e banco para começar.
          </p>
        </div>
      )}

      {lista.fase === "pronto" && perfis.length > 0 && (
        <div style={{ ...estilos.cartao, overflowX: "auto" }}>
          <table style={estilos.tabela}>
            <caption style={{ ...estilos.th, captionSide: "top", textAlign: "left" }}>
              Perfis de conexão salvos
            </caption>
            <thead>
              <tr>
                <th scope="col" style={estilos.th}>
                  Nome
                </th>
                <th scope="col" style={estilos.th}>
                  Host:porta
                </th>
                <th scope="col" style={estilos.th}>
                  Banco
                </th>
                <th scope="col" style={estilos.th}>
                  Usuário
                </th>
                <th scope="col" style={estilos.th}>
                  Charset
                </th>
                <th scope="col" style={estilos.th}>
                  Dialeto
                </th>
                <th scope="col" style={estilos.th}>
                  Status
                </th>
                <th scope="col" style={estilos.th}>
                  Ações
                </th>
              </tr>
            </thead>
            <tbody>
              {perfis.map((perfil) => (
                <LinhaPerfil
                  key={perfil.id}
                  perfil={perfil}
                  status={statusPorPerfil[perfil.id] ?? "disconnected"}
                  statusInfo={detalhePorPerfil[perfil.id]}
                  acao={acaoPorPerfil[perfil.id] ?? null}
                  onTestar={() => void aoTestar(perfil)}
                  onConectar={() => void aoConectar(perfil)}
                  onDesconectar={() => void aoDesconectar(perfil)}
                  onEditar={() => {
                    setEditando(perfil);
                    setFormAberto(true);
                  }}
                  onAbrirSenha={() =>
                    setIdSenhaAberta((atual) => (atual === perfil.id ? null : perfil.id))
                  }
                />
              ))}
            </tbody>
          </table>

          {idSenhaAberta !== null && (
            <SenhaParaConectar
              perfil={perfis.find((p) => p.id === idSenhaAberta) ?? null}
              valor={senhaPorPerfil[idSenhaAberta] ?? ""}
              aoAlterar={(valor) =>
                setSenhaPorPerfil((atuais) => ({ ...atuais, [idSenhaAberta]: valor }))
              }
              aoFechar={() => setIdSenhaAberta(null)}
            />
          )}

          {Object.entries(erroPorPerfil).map(([id, erro]) => (
            <div key={id} role="alert" style={{ ...estilos.aviso, marginTop: "0.75rem" }}>
              <strong>{ROTULO_CODIGO_ERRO[erro.code]}:</strong> {erro.message}
              <button
                type="button"
                style={{ ...estilos.botao, marginLeft: "0.5rem" }}
                onClick={() => limparErro(id)}
              >
                Dispensar
              </button>
            </div>
          ))}
        </div>
      )}

      {formAberto && (
        <div style={estilos.cartao}>
          <FormularioPerfil
            inicial={editando}
            aoSalvar={aoSalvar}
            aoCancelar={() => {
              setFormAberto(false);
              setEditando(null);
            }}
            salvando={salvando}
          />
        </div>
      )}
    </section>
  );
}

/** Entrada da senha usada apenas na ação de conectar (nunca persistida). */
function SenhaParaConectar(props: {
  perfil: ConnectionProfile | null;
  valor: string;
  aoAlterar: (valor: string) => void;
  aoFechar: () => void;
}): JSX.Element {
  const { perfil, valor, aoAlterar, aoFechar } = props;
  const idCampo = useId();
  if (perfil === null) return <></>;

  return (
    <div style={{ ...estilos.cartao, marginTop: "0.75rem", background: "#fbfbfc" }}>
      <label htmlFor={idCampo} style={{ fontSize: "0.85rem", display: "block" }}>
        Senha para “{perfil.name}” (usada só nesta conexão, guardada no cofre do sistema)
      </label>
      <input
        id={idCampo}
        type="password"
        autoComplete="current-password"
        style={{ ...estilos.entrada, marginTop: "0.25rem", minWidth: "18rem" }}
        value={valor}
        onChange={(e) => aoAlterar(e.target.value)}
      />
      <button
        type="button"
        style={{ ...estilos.botao, marginLeft: "0.5rem" }}
        onClick={aoFechar}
      >
        Fechar
      </button>
    </div>
  );
}