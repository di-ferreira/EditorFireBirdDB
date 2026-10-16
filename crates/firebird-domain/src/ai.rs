//! Domínio da assistência de IA (Spec 07).
//!
//! Tipos puros: descrevem **o que** pedir ao provedor e **o que** ele devolve.
//! Nada de rede, nada de segredo — a chave da API vive no cofre e só aparece no
//! provedor, nunca aqui (FR-016).

use std::fmt;

/// Tipo de provedor suportado.
///
/// `OpenAiCompat` cobre Ollama, LM Studio, vLLM e a própria API da OpenAI: o
/// protocolo `POST /v1/chat/completions` é o mesmo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoProvedorIa {
    /// Endpoint compatível com OpenAI (local ou remoto).
    OpenAiCompat,
}

impl TipoProvedorIa {
    /// Identificador estável usado na persistência.
    pub fn as_str(self) -> &'static str {
        match self {
            TipoProvedorIa::OpenAiCompat => "openai_compat",
        }
    }

    /// Converte o identificador persistido; `None` quando desconhecido.
    pub fn from_str_bruto(bruto: &str) -> Option<Self> {
        match bruto {
            "openai_compat" => Some(TipoProvedorIa::OpenAiCompat),
            _ => None,
        }
    }
}

impl fmt::Display for TipoProvedorIa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Configuração de um provedor (sem segredo: a chave fica no cofre).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvedorIaConfig {
    /// Nome exibido.
    pub name: String,
    /// Tipo do provedor.
    pub provider_type: TipoProvedorIa,
    /// URL base da API (ex.: `http://localhost:11434/v1`).
    pub base_url: String,
    /// Modelo usado.
    pub model_name: String,
    /// Timeout da chamada, em milissegundos.
    pub timeout_ms: i64,
    /// `true` quando o usuário permite enviar contexto a este provedor remoto.
    pub remote_context_confirmation: bool,
}

impl ProvedorIaConfig {
    /// `true` quando o provedor está na própria máquina (sem consentimento).
    pub fn is_local(&self) -> bool {
        let base = self.base_url.to_lowercase();
        base.contains("localhost") || base.contains("127.0.0.1") || base.contains("[::1]")
    }

    /// URL completa de chat completions.
    pub fn endpoint_chat(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

/// Tarefa pedida ao modelo (mapeia os templates versionados).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TarefaIa {
    /// Gera SQL a partir de um pedido em linguagem natural.
    GerarSql,
    /// Explica um SQL existente.
    ExplicarSql,
    /// Sugere correção para um SQL com erro.
    CorrigirSql,
    /// Revisa um SQL e aponta riscos.
    RevisarSql,
}

impl TarefaIa {
    /// Identificador estável da tarefa.
    pub fn as_str(self) -> &'static str {
        match self {
            TarefaIa::GerarSql => "gerar_sql",
            TarefaIa::ExplicarSql => "explicar_sql",
            TarefaIa::CorrigirSql => "corrigir_sql",
            TarefaIa::RevisarSql => "revisar_sql",
        }
    }
}

/// Pedido ao provedor: tarefa, texto do usuário e contexto do banco.
///
/// `contexto` é o **resumo do schema** (nomes de tabelas/colunas), nunca dados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoIa {
    /// O que se quer.
    pub tarefa: TarefaIa,
    /// Texto do usuário (pergunta ou SQL).
    pub prompt: String,
    /// Contexto do schema já resumido.
    pub contexto: String,
}

/// Resposta do modelo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespostaIa {
    /// Texto devolvido pelo modelo.
    pub texto: String,
    /// SQL sugerido, quando a tarefa produce SQL.
    pub sql: Option<String>,
    /// Modelo que respondeu (para auditoria).
    pub modelo: String,
}

/// Erro do provedor, sempre sem credencial na mensagem.
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroIa {
    /// Configuração inválida (URL vazia, tipo desconhecido…).
    #[error("configuração do provedor inválida: {0}")]
    Configuracao(String),
    /// Falha de rede/HTTP já redigida.
    #[error("provedor indisponível: {0}")]
    Indisponivel(String),
    /// Resposta fora do esperado (JSON inválido, campos ausentes).
    #[error("resposta inválida do provedor: {0}")]
    Resposta(String),
    /// Consentimento remoto ausente ou negado.
    #[error("envio de contexto a provedor remoto exige consentimento explícito")]
    SemConsentimento,
}

impl fmt::Debug for ErroIa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Redige qualquer coisa que pareça credencial na mensagem.
        let texto = crate::error::redigir_segredo(&self.to_string());
        f.write_str(&texto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipo_de_provedor_via_texto() {
        assert_eq!(
            TipoProvedorIa::from_str_bruto("openai_compat"),
            Some(TipoProvedorIa::OpenAiCompat)
        );
        assert_eq!(TipoProvedorIa::from_str_bruto("desconhecido"), None);
        assert_eq!(TipoProvedorIa::OpenAiCompat.to_string(), "openai_compat");
    }

    #[test]
    fn config_reconhece_provedor_local() {
        let local = ProvedorIaConfig {
            name: "Ollama".into(),
            provider_type: TipoProvedorIa::OpenAiCompat,
            base_url: "http://localhost:11434/v1".into(),
            model_name: "qwen".into(),
            timeout_ms: 30_000,
            remote_context_confirmation: false,
        };
        assert!(local.is_local());
        assert_eq!(
            local.endpoint_chat(),
            "http://localhost:11434/v1/chat/completions"
        );

        let remoto = ProvedorIaConfig {
            base_url: "https://api.exemplo.com/v1/".into(),
            ..local
        };
        assert!(!remoto.is_local());
        assert_eq!(
            remoto.endpoint_chat(),
            "https://api.exemplo.com/v1/chat/completions"
        );
    }

    #[test]
    fn erro_de_ia_redige_credencial_na_mensagem() {
        let erro = ErroIa::Indisponivel("401 token=sk-secreto123".into());
        let debug = format!("{erro:?}");
        assert!(!debug.contains("sk-secreto123"), "debug redigido: {debug}");
        assert!(debug.contains("***"));
    }
}
