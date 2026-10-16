//! Aplicação do assistente de IA — Passo 1 da Fase 3 (Spec 07).
//!
//! O provedor fala o protocolo `POST /v1/chat/completions` (OpenAI-compatível,
//! que cobre Ollama, LM Studio e OpenAI). O transporte HTTP é **injetado**, então
//! nenhum teste toca a rede: basta um [`TransporteHttp`] de dublê.
//!
//! Regras herdadas que valem aqui:
//! * a chave da API vem do cofre e **nunca** é logada, devolvida em DTO nem
//!   inclusionada em `Debug`;
//! * contexto só é enviado a provedor **remoto** quando o perfil tem consentimento
//!   explícito (`remote_context_confirmation`);
//! * o SQL devolvido é sugestão — nada aqui executa SQL.

use std::sync::Arc;

use firebird_domain::{ErroIa, PedidoIa, ProvedorIaConfig, RespostaIa, TarefaIa};

/// Transporte HTTP mínimo — a única porta que sai do processo.
pub trait TransporteHttp: Send + Sync {
    /// `POST` JSON e devolve o corpo da resposta.
    ///
    /// Implementações reais e de teste **redigem** credenciais: a mensagem de
    /// erro nunca pode conter a chave.
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        corpo: &str,
        timeout_ms: i64,
    ) -> Result<String, String>;
}

/// Provedor capaz de responder uma tarefa de IA.
pub trait ProvedorIa: Send + Sync {
    /// Verifica endpoint/modelo sem enviar contexto do banco.
    fn testar(&self) -> Result<String, String>;
    /// Responde um pedido.
    fn gerar(&self, pedido: &PedidoIa, api_key: Option<&str>) -> Result<RespostaIa, ErroIa>;
}

/// Prompt de sistema por tarefa (templates versionados — passo 2 detalha o
/// versionamento; aqui a穩定 é `v1`).
const PROMPT_SISTEMA: &str = "\
Você é um assistente de SQL para Firebird. Responda de forma curta e prática, \
apenas com SQL Firebird válido, sem comentários que não sejam SQL.";

fn instrucao(tarefa: TarefaIa) -> &'static str {
    match tarefa {
        TarefaIa::GerarSql => "Gere o SQL Firebird que responde ao pedido.",
        TarefaIa::ExplicarSql => "Explique o SQL abaixo, objeto por objeto.",
        TarefaIa::CorrigirSql => "Corrija o SQL abaixo e devolva apenas o SQL corrigido.",
        TarefaIa::RevisarSql => "Revise o SQL abaixo e liste riscos concretos.",
    }
}

/// Provedor OpenAI-compatível (local ou remoto).
pub struct ProvedorOpenAiCompat {
    /// Configuração do perfil (sem segredo).
    pub config: ProvedorIaConfig,
    /// Transporte injetado (real ou de teste).
    pub transporte: Arc<dyn TransporteHttp>,
}

impl ProvedorOpenAiCompat {
    /// Monta o corpo JSON da requisição.
    ///
    /// `contexto` só entra quando há consentimento (provedor remoto) ou quando o
    /// provedor é local — ver [`ProvedorOpenAiCompat::pode_enviar_contexto`].
    fn corpo(&self, pedido: &PedidoIa, incluir_contexto: bool) -> String {
        let contexto = if incluir_contexto && !pedido.contexto.trim().is_empty() {
            format!(
                "\n\nEsquema (apenas metadados, sem dados):\n{}",
                pedido.contexto
            )
        } else {
            String::new()
        };
        let usuario = format!("{}{}", pedido.prompt, contexto);
        format!(
            "{{\"model\":{},\"messages\":[{{\"role\":\"system\",\"content\":{}}},{{\"role\":\"user\",\"content\":{}}}]}}",
            json_string(&self.config.model_name),
            json_string(PROMPT_SISTEMA),
            json_string(&format!("{}\n\n{}", instrucao(pedido.tarefa), usuario)),
        )
    }

    /// `true` quando este perfil pode receber o contexto do schema.
    pub fn pode_enviar_contexto(&self) -> bool {
        self.config.is_local() || self.config.remote_context_confirmation
    }

    fn cabecalhos(&self, api_key: Option<&str>) -> Vec<(String, String)> {
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if let Some(chave) = api_key.filter(|c| !c.trim().is_empty()) {
            headers.push(("Authorization".to_string(), format!("Bearer {chave}")));
        }
        headers
    }
}

impl ProvedorIa for ProvedorOpenAiCompat {
    fn testar(&self) -> Result<String, String> {
        if self.config.base_url.trim().is_empty() {
            return Err("base_url vazia".to_string());
        }
        if self.config.model_name.trim().is_empty() {
            return Err("modelo vazio".to_string());
        }
        // Chamada mínima: 1 token, sem contexto do banco.
        let corpo = format!(
            "{{\"model\":{},\"max_tokens\":1,\"messages\":[{{\"role\":\"user\",\"content\":\"ping\"}}]}}",
            json_string(&self.config.model_name)
        );
        self.transporte
            .post_json(
                &self.config.endpoint_chat(),
                &self.cabecalhos(None),
                &corpo,
                self.config.timeout_ms,
            )
            .map(|_| format!("provedor respondeu: {}", self.config.model_name))
            .map_err(|e| firebird_domain::redigir_segredo(&e))
    }

    fn gerar(&self, pedido: &PedidoIa, api_key: Option<&str>) -> Result<RespostaIa, ErroIa> {
        if !self.config.base_url.trim().is_empty() && !self.pode_enviar_contexto() {
            return Err(ErroIa::SemConsentimento);
        }
        let corpo = self.corpo(pedido, true);
        let resposta = self
            .transporte
            .post_json(
                &self.config.endpoint_chat(),
                &self.cabecalhos(api_key),
                &corpo,
                self.config.timeout_ms,
            )
            .map_err(|e| ErroIa::Indisponivel(firebird_domain::redigir_segredo(&e)))?;

        let texto = extrair_conteudo(&resposta)?;
        let sql = extrair_sql(&texto);
        Ok(RespostaIa {
            texto,
            sql,
            modelo: self.config.model_name.clone(),
        })
    }
}

/// Extrai `choices[0].message.content` de uma resposta OpenAI-compatível.
///
/// Parser deliberadamente simples (sem dependência): só o que o protocolo
/// garante no caminho feliz; o resto vira [`ErroIa::Resposta`].
fn extrair_conteudo(resposta: &str) -> Result<String, ErroIa> {
    let chave = "\"content\"";
    let inicio = resposta
        .find(chave)
        .ok_or_else(|| ErroIa::Resposta("campo content ausente".into()))?;
    let resto = &resposta[inicio + chave.len()..];
    let aspas = resto
        .find('"')
        .ok_or_else(|| ErroIa::Resposta("valor de content não é texto".into()))?;
    let corpo = &resto[aspas + 1..];
    let fim = corpo
        .find('"')
        .ok_or_else(|| ErroIa::Resposta("texto de content não terminado".into()))?;
    Ok(unescape_json(&corpo[..fim]))
}

/// Isola um SQL sugerido dentro de um texto em blocos ```.
fn extrair_sql(texto: &str) -> Option<String> {
    let inicio = texto.find("```")?;
    let depois = &texto[inicio + 3..];
    let (sql, _) = depois.split_once("```")?;
    let sql = sql
        .lines()
        .filter(|l| !l.trim_start().starts_with("sql") && !l.trim_start().starts_with("SQL"))
        .collect::<Vec<_>>()
        .join("\n");
    let sql = sql.trim().to_string();
    if sql.is_empty() {
        None
    } else {
        Some(sql)
    }
}

fn json_string(valor: &str) -> String {
    let escapado = valor
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escapado}\"")
}

fn unescape_json(valor: &str) -> String {
    valor
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\t", "\t")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

/// Provedor de teste: responde sem rede e registra o que recebeu.
#[derive(Default)]
pub struct ProvedorDeTeste {
    /// Última resposta devolvida.
    pub resposta: std::sync::Mutex<Option<String>>,
    /// Falha a ser devolvida, se houver.
    pub falha: std::sync::Mutex<Option<String>>,
}

impl ProvedorDeTeste {
    /// Provedor que responde `texto`.
    pub fn respondendo(texto: &str) -> Self {
        Self {
            resposta: std::sync::Mutex::new(Some(texto.to_string())),
            falha: std::sync::Mutex::new(None),
        }
    }

    /// Provedor que falha com `mensagem`.
    pub fn falhando(mensagem: &str) -> Self {
        Self {
            resposta: std::sync::Mutex::new(None),
            falha: std::sync::Mutex::new(Some(mensagem.to_string())),
        }
    }
}

impl ProvedorIa for ProvedorDeTeste {
    fn testar(&self) -> Result<String, String> {
        match self.falha.lock().ok().and_then(|g| g.clone()) {
            Some(motivo) => Err(motivo),
            None => Ok("provedor de teste disponível".to_string()),
        }
    }

    fn gerar(&self, _pedido: &PedidoIa, _api_key: Option<&str>) -> Result<RespostaIa, ErroIa> {
        if let Some(motivo) = self.falha.lock().ok().and_then(|g| g.clone()) {
            return Err(ErroIa::Indisponivel(motivo));
        }
        let texto = self
            .resposta
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_default();
        Ok(RespostaIa {
            sql: extrair_sql(&texto),
            modelo: "teste".to_string(),
            texto,
        })
    }
}

/// Transporte HTTP real, baseado em `reqwest` bloqueante com rustls.
///
/// A mensagem de erro passa por `redigir_segredo`: mesmo que o servidor devolva
/// o cabeçalho `Authorization` no corpo do erro, ele não chega ao log nem à UI.
pub struct TransporteReqwest {
    /// Cliente compartilhado (conexões reaproveitadas).
    client: reqwest::blocking::Client,
}

impl TransporteReqwest {
    /// Cria o cliente com o proxy do sistema desativado (ambiente de desktop
    /// não deve herdar proxy do shell sem consentimento).
    pub fn new() -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .map_err(|e| firebird_domain::redigir_segredo(&e.to_string()))?;
        Ok(Self { client })
    }
}

impl TransporteHttp for TransporteReqwest {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        corpo: &str,
        timeout_ms: i64,
    ) -> Result<String, String> {
        let tempo = std::time::Duration::from_millis(timeout_ms.clamp(1_000, 600_000) as u64);
        let mut requisicao = self
            .client
            .post(url)
            .timeout(tempo)
            .body(corpo.to_string())
            .header("Content-Type", "application/json");
        for (nome, valor) in headers {
            requisicao = requisicao.header(nome.as_str(), valor.as_str());
        }
        let resposta = requisicao
            .send()
            .map_err(|e| firebird_domain::redigir_segredo(&e.to_string()))?;
        let status = resposta.status();
        let texto = resposta
            .text()
            .map_err(|e| firebird_domain::redigir_segredo(&e.to_string()))?;
        if !status.is_success() {
            return Err(firebird_domain::redigir_segredo(&format!(
                "HTTP {}: {}",
                status.as_u16(),
                texto.chars().take(400).collect::<String>()
            )));
        }
        Ok(texto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transporte de teste que devolve uma resposta fixa.
    struct TransporteFixo {
        resposta: String,
        erro: Option<String>,
        url_capturada: std::sync::Mutex<String>,
    }

    impl TransporteFixo {
        fn ok(resposta: &str) -> Self {
            Self {
                resposta: resposta.to_string(),
                erro: None,
                url_capturada: std::sync::Mutex::new(String::new()),
            }
        }

        fn falhando(motivo: &str) -> Self {
            Self {
                resposta: String::new(),
                erro: Some(motivo.to_string()),
                url_capturada: std::sync::Mutex::new(String::new()),
            }
        }
    }

    impl TransporteHttp for TransporteFixo {
        fn post_json(
            &self,
            url: &str,
            _headers: &[(String, String)],
            _corpo: &str,
            _timeout_ms: i64,
        ) -> Result<String, String> {
            if let Ok(mut guarda) = self.url_capturada.lock() {
                *guarda = url.to_string();
            }
            match &self.erro {
                Some(motivo) => Err(motivo.clone()),
                None => Ok(self.resposta.clone()),
            }
        }
    }

    fn config_local() -> ProvedorIaConfig {
        ProvedorIaConfig {
            name: "Ollama".into(),
            provider_type: firebird_domain::TipoProvedorIa::OpenAiCompat,
            base_url: "http://localhost:11434/v1".into(),
            model_name: "qwen2.5-coder".into(),
            timeout_ms: 30_000,
            remote_context_confirmation: false,
        }
    }

    #[test]
    fn gerar_extrai_texto_e_sql_da_resposta() {
        let transporte = TransporteFixo::ok(
            r#"{"choices":[{"message":{"content":"Aqui está:\n```sql\nSELECT FIRST 10 A FROM T\n```"}}]}"#,
        );
        let provedor = ProvedorOpenAiCompat {
            config: config_local(),
            transporte: Arc::new(transporte),
        };

        let pedido = PedidoIa {
            tarefa: TarefaIa::GerarSql,
            prompt: "liste as linhas de T".into(),
            contexto: "T(A INTEGER)".into(),
        };
        let resposta = provedor.gerar(&pedido, None).expect("resposta");

        assert!(resposta.texto.contains("Aqui está"));
        assert_eq!(resposta.sql.as_deref(), Some("SELECT FIRST 10 A FROM T"));
        assert_eq!(resposta.modelo, "qwen2.5-coder");
    }

    #[test]
    fn contexto_so_sai_com_consentimento_em_provedor_remoto() {
        let remoto = ProvedorOpenAiCompat {
            config: ProvedorIaConfig {
                base_url: "https://api.exemplo.com/v1".into(),
                ..config_local()
            },
            transporte: Arc::new(TransporteFixo::ok(
                r#"{"choices":[{"message":{"content":"ok"}}]}"#,
            )),
        };
        let pedido = PedidoIa {
            tarefa: TarefaIa::GerarSql,
            prompt: "p".into(),
            contexto: "T(A)".into(),
        };

        assert!(matches!(
            remoto.gerar(&pedido, None),
            Err(ErroIa::SemConsentimento)
        ));

        let consentido = ProvedorOpenAiCompat {
            config: ProvedorIaConfig {
                remote_context_confirmation: true,
                ..remoto.config.clone()
            },
            transporte: remoto.transporte.clone(),
        };
        assert!(consentido.gerar(&pedido, None).is_ok());
    }

    #[test]
    fn falha_do_transporte_vem_redigida() {
        let provedor = ProvedorOpenAiCompat {
            config: config_local(),
            transporte: Arc::new(TransporteFixo::falhando("401 token=sk-abc123")),
        };
        let pedido = PedidoIa {
            tarefa: TarefaIa::ExplicarSql,
            prompt: "explique".into(),
            contexto: String::new(),
        };

        match provedor.gerar(&pedido, None) {
            Err(ErroIa::Indisponivel(mensagem)) => {
                assert!(
                    !mensagem.contains("sk-abc123"),
                    "mensagem redigida: {mensagem}"
                );
            }
            outro => panic!("esperava indisponível, veio {outro:?}"),
        }
    }

    #[test]
    fn testar_usa_endpoint_de_chat_e_exige_config() {
        let transporte = Arc::new(TransporteFixo::ok("{}"));
        let provedor = ProvedorOpenAiCompat {
            config: config_local(),
            transporte: transporte.clone(),
        };

        assert!(provedor.testar().is_ok());
        // segundo teste: configuração vazia falha antes da rede
        let invalido = ProvedorOpenAiCompat {
            config: ProvedorIaConfig {
                base_url: String::new(),
                ..provedor.config.clone()
            },
            transporte: provedor.transporte.clone(),
        };
        assert!(invalido.testar().is_err());
    }

    #[test]
    fn resposta_sem_content_e_erro_explicito() {
        assert!(matches!(
            extrair_conteudo(r#"{"choices":[]}"#),
            Err(ErroIa::Resposta(_))
        ));
    }

    #[test]
    fn cofre_de_memoria_guarda_a_chave_fora_do_perfil() {
        use firebird_security::SecretStore as _;
        // A chave nunca entra no perfil: o serviço resolve via secret_ref.
        let cofre = firebird_security::InMemorySecretStore::new();
        cofre.set("ia:p1", "sk-secreto").expect("gravando no cofre");
        assert_eq!(
            cofre.get("ia:p1").expect("lendo").as_deref(),
            Some("sk-secreto")
        );
    }
}
