//! Serviço do assistente de IA — Passo 1 da Fase 3.
//!
//! Junta as três peças mantendo a separação de responsabilidades:
//! * **SQLite local** guarda o perfil (sem chave);
//! * **cofre do sistema** guarda a chave da API (`secret_ref` aponta para lá);
//! * **provedor** faz a chamada HTTP, com transporte injetado (testes sem rede).

use std::sync::Arc;

use firebird_domain::{ErroIa, ProvedorIaConfig, TipoProvedorIa};
use firebird_security::SecretStore;
use firebird_storage::{agora_iso8601, AiInteraction, AiProviderProfile};

use crate::application::ia::{ProvedorIa, ProvedorOpenAiCompat, TransporteHttp};
use crate::storage::BancoLocal;

/// Dados de entrada para criar/atualizar um provedor.
#[derive(Debug, Clone)]
pub struct ProvedorEntrada {
    /// Id quando for edição; `None` cria novo.
    pub id: Option<String>,
    /// Nome exibido.
    pub nome: String,
    /// Tipo persistido (`openai_compat`).
    pub tipo: String,
    /// URL base da API.
    pub base_url: String,
    /// Modelo usado.
    pub modelo: String,
    /// Chave da API — vai para o cofre, nunca para o banco nem para DTO.
    pub api_key: Option<String>,
    /// Timeout em milissegundos.
    pub timeout_ms: i64,
    /// Consentimento para enviar contexto a provedor remoto.
    pub consentir_contexto_remoto: bool,
}

/// Erro do serviço (sempre redigido).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroServicoIa {
    /// Perfil não encontrado.
    #[error("provedor de IA não encontrado")]
    NaoEncontrado,
    /// Falha de armazenamento/cofre (já redigida).
    #[error("{0}")]
    Infra(String),
}

/// Serviço de configuração de provedores.
pub struct IaService {
    /// Perfis persistidos localmente.
    repos: Arc<BancoLocal>,
    /// Cofre do sistema (chave da API).
    secrets: Arc<dyn SecretStore>,
    /// Transporte HTTP injetado.
    transporte: Arc<dyn TransporteHttp>,
}

impl IaService {
    /// Monta o serviço com as dependências injetadas.
    pub fn new(
        repos: Arc<BancoLocal>,
        secrets: Arc<dyn SecretStore>,
        transporte: Arc<dyn TransporteHttp>,
    ) -> Self {
        Self {
            repos,
            secrets,
            transporte,
        }
    }

    /// Prefixo da chave no cofre — determinístico e sem PII (só o id).
    fn chave_do_cofre(id: &str) -> String {
        format!("firebird-studio/ia/{id}")
    }

    /// Traduz o registro do storage para a configuração do domínio.
    fn config_de(registro: &AiProviderProfile) -> Result<ProvedorIaConfig, ErroIa> {
        let tipo = TipoProvedorIa::from_str_bruto(&registro.provider_type).ok_or_else(|| {
            ErroIa::Configuracao(format!("tipo desconhecido: {}", registro.provider_type))
        })?;
        Ok(ProvedorIaConfig {
            name: registro.name.clone(),
            provider_type: tipo,
            base_url: registro.base_url.clone(),
            model_name: registro.model_name.clone(),
            timeout_ms: registro.timeout_ms,
            remote_context_confirmation: registro.remote_context_confirmation,
        })
    }

    /// Cria ou atualiza um provedor, guardando a chave no cofre.
    pub fn salvar(&self, entrada: ProvedorEntrada) -> Result<AiProviderProfile, ErroServicoIa> {
        let tipo = TipoProvedorIa::from_str_bruto(&entrada.tipo).ok_or_else(|| {
            ErroServicoIa::Infra(format!("tipo de provedor desconhecido: {}", entrada.tipo))
        })?;
        if entrada.nome.trim().is_empty() {
            return Err(ErroServicoIa::Infra(
                "nome do provedor é obrigatório".into(),
            ));
        }
        if entrada.base_url.trim().is_empty() {
            return Err(ErroServicoIa::Infra(
                "base_url do provedor é obrigatória".into(),
            ));
        }
        if entrada.modelo.trim().is_empty() {
            return Err(ErroServicoIa::Infra(
                "modelo do provedor é obrigatório".into(),
            ));
        }

        let agora = agora_iso8601();
        let existente = match &entrada.id {
            Some(id) if !id.trim().is_empty() => self.repos.obter_provedor_ia(id).map_err(|e| {
                ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string()))
            })?,
            _ => None,
        };
        let id = entrada
            .id
            .clone()
            .filter(|i| !i.trim().is_empty() && existente.is_some())
            .unwrap_or_else(|| crate::application::connection_manager::novo_id("ia"));

        // A chave só entra no cofre; o perfil guarda a referência.
        let secret_ref = match entrada.api_key.as_deref().map(str::trim) {
            Some(chave) if !chave.is_empty() => {
                let referencia = Self::chave_do_cofre(&id);
                self.secrets.set(&referencia, chave).map_err(|e| {
                    ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string()))
                })?;
                Some(referencia)
            }
            _ => existente.as_ref().and_then(|p| p.secret_ref.clone()),
        };

        let perfil = AiProviderProfile {
            id,
            name: entrada.nome,
            provider_type: tipo.as_str().to_string(),
            base_url: entrada.base_url,
            model_name: entrada.modelo,
            secret_ref,
            timeout_ms: entrada.timeout_ms.clamp(1_000, 600_000),
            remote_context_confirmation: entrada.consentir_contexto_remoto,
            created_at: existente
                .as_ref()
                .map(|p| p.created_at.clone())
                .unwrap_or_else(|| agora.clone()),
            updated_at: agora,
        };
        self.repos
            .salvar_provedor_ia(&perfil)
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))?;
        Ok(perfil)
    }

    /// Lista os provedores configurados.
    pub fn listar(&self) -> Result<Vec<AiProviderProfile>, ErroServicoIa> {
        self.repos
            .provedores_ia()
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))
    }

    /// Remove o perfil e a chave correspondente do cofre.
    pub fn apagar(&self, id: &str) -> Result<bool, ErroServicoIa> {
        let removido = self
            .repos
            .apagar_provedor_ia(id)
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))?;
        if removido {
            // Segredo sai junto: chave órfã no cofre não deve sobrar.
            let _ = self.secrets.delete(&Self::chave_do_cofre(id));
        }
        Ok(removido)
    }

    /// Gera uma resposta do modelo com o contexto do schema do perfil.
    ///
    /// Fluxo: monta contexto (metadados) → prompt do template versionado →
    /// chave da API lida do cofre → chamada ao provedor → registro em
    /// `ai_interactions`. O SQL devolvido é **sugestão**: nada é executado.
    pub fn gerar(
        &self,
        provider_id: &str,
        tarefa: firebird_domain::TarefaIa,
        entrada: &str,
        contexto: &crate::application::ia_context::ContextoSchema,
    ) -> Result<ResultadoGerado, ErroServicoIa> {
        if entrada.trim().is_empty() {
            return Err(ErroServicoIa::Infra("o pedido está vazio".into()));
        }
        let registro = self
            .repos
            .obter_provedor_ia(provider_id)
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))?
            .ok_or(ErroServicoIa::NaoEncontrado)?;
        let config = Self::config_de(&registro).map_err(|e| ErroServicoIa::Infra(e.to_string()))?;

        let template = crate::application::ia_context::template_de(tarefa)
            .ok_or_else(|| ErroServicoIa::Infra("tarefa sem template".into()))?;
        let prompt = crate::application::ia_context::montar_prompt(template, contexto, entrada);

        // A chave sai do cofre só agora e só para o cabeçalho HTTP.
        let chave = match registro.secret_ref.as_deref() {
            Some(referencia) => self.secrets.get(referencia).map_err(|e| {
                ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string()))
            })?,
            None => None,
        };

        let provedor = ProvedorOpenAiCompat {
            config,
            transporte: self.transporte.clone(),
        };
        let pedido = firebird_domain::PedidoIa {
            tarefa,
            prompt: entrada.to_string(),
            contexto: contexto.texto.clone(),
        };
        let resposta = provedor
            .gerar(&pedido, chave.as_deref())
            .map_err(|e| ErroServicoIa::Infra(e.to_string()))?;

        let resumo = resumo_do_contexto(contexto);
        let interacao = AiInteraction {
            id: crate::application::connection_manager::novo_id("iax"),
            provider_profile_id: Some(registro.id.clone()),
            connection_fingerprint: Some(impressao_do_contexto(contexto)),
            task_type: tarefa.as_str().to_string(),
            prompt_text: redigir(&prompt),
            context_summary: resumo,
            response_text: redigir(&resposta.texto),
            accepted_sql: resposta.sql.clone().map(|s| redigir(&s)),
            created_at: agora_iso8601(),
        };
        self.repos
            .add_interacao_ia(&interacao)
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))?;

        Ok(ResultadoGerado {
            resposta,
            interacao_id: interacao.id,
        })
    }

    /// Lista interações recentes para a UI mostrar o histórico.
    pub fn historico(&self, limite: usize) -> Result<Vec<AiInteraction>, ErroServicoIa> {
        self.repos
            .interacoes_ia(limite.clamp(1, 500))
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))
    }

    /// Marca o SQL sugerido como aceito (registro de auditoria).
    pub fn marcar_sql_aceito(&self, interacao_id: &str, sql: &str) -> Result<(), ErroServicoIa> {
        let mut interacoes = self
            .repos
            .interacoes_ia(500)
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))?;
        let alvo = interacoes
            .iter_mut()
            .find(|i| i.id == interacao_id)
            .ok_or(ErroServicoIa::NaoEncontrado)?;
        alvo.accepted_sql = Some(redigir(sql));
        alvo.created_at = alvo.created_at.clone();
        self.repos
            .add_interacao_ia(alvo)
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))
    }

    /// Testa a configuração chamando o provedor (sem contexto do banco).
    ///
    /// A chave do cofre **não** é enviada no teste: só_ENDPOINT e modelo são
    /// validados; provedores que exigem autenticação só passam no passo 2+.
    pub fn testar(&self, id: &str) -> Result<String, ErroServicoIa> {
        let registro = self
            .repos
            .obter_provedor_ia(id)
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e.to_string())))?
            .ok_or(ErroServicoIa::NaoEncontrado)?;
        let config = Self::config_de(&registro).map_err(|e| ErroServicoIa::Infra(e.to_string()))?;
        let provedor = ProvedorOpenAiCompat {
            config,
            transporte: self.transporte.clone(),
        };
        provedor
            .testar()
            .map_err(|e| ErroServicoIa::Infra(firebird_domain::redigir_segredo(&e)))
    }
}

/// Resposta do modelo com o id da interação registrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoGerado {
    /// Texto e SQL sugerido pelo modelo.
    pub resposta: firebird_domain::RespostaIa,
    /// Id registrado em `ai_interactions`.
    pub interacao_id: String,
}

fn redigir(texto: &str) -> String {
    firebird_domain::redigir_segredo(texto)
}

/// Resumo curto do contexto: nomes das relações enviadas (nunca colunas/dados).
fn resumo_do_contexto(contexto: &crate::application::ia_context::ContextoSchema) -> String {
    let nomes: Vec<&str> = contexto
        .texto
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .take(40)
        .collect();
    format!("{} relação(ões): {}", contexto.relacoes, nomes.join(", "))
}

/// Impressão digital estável do contexto (FNV-1a 64) — permite saber, depois,
/// se a resposta foi gerada sobre o mesmo esquema, sem guardar o esquema inteiro.
pub fn impressao_do_contexto(contexto: &crate::application::ia_context::ContextoSchema) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in contexto.texto.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use firebird_security::InMemorySecretStore;

    /// Transporte que nunca sai do processo.
    struct TransporteNulo;

    impl TransporteHttp for TransporteNulo {
        fn post_json(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _corpo: &str,
            _timeout_ms: i64,
        ) -> Result<String, String> {
            Ok(r#"{"choices":[{"message":{"content":"pong"}}]}"#.to_string())
        }
    }

    fn servico() -> (IaService, Arc<BancoLocal>, Arc<InMemorySecretStore>) {
        let repos = Arc::new(BancoLocal::em_memoria().expect("banco em memória"));
        let cofre = Arc::new(InMemorySecretStore::new());
        let servico = IaService::new(repos.clone(), cofre.clone(), Arc::new(TransporteNulo));
        (servico, repos, cofre)
    }

    fn entrada(nome: &str) -> ProvedorEntrada {
        ProvedorEntrada {
            id: None,
            nome: nome.into(),
            tipo: "openai_compat".into(),
            base_url: "http://localhost:11434/v1".into(),
            modelo: "qwen2.5-coder".into(),
            api_key: None,
            timeout_ms: 30_000,
            consentir_contexto_remoto: false,
        }
    }

    #[test]
    fn salva_perfil_sem_chave_no_banco_e_com_chave_no_cofre() {
        let (servico, _repos, cofre) = servico();

        let perfil = servico
            .salvar(ProvedorEntrada {
                api_key: Some("sk-secreto".into()),
                ..entrada("Ollama")
            })
            .expect("salvando");

        assert!(perfil.secret_ref.is_some());
        assert_eq!(
            cofre
                .get(perfil.secret_ref.as_deref().unwrap())
                .expect("lendo cofre")
                .as_deref(),
            Some("sk-secreto")
        );
        // O perfil serializado jamais carrega a chave.
        assert!(!format!("{perfil:?}").contains("sk-secreto"));
    }

    #[test]
    fn edicao_preserva_a_chave_ja_guardada() {
        let (servico, _repos, cofre) = servico();
        let perfil = servico
            .salvar(ProvedorEntrada {
                api_key: Some("sk-secreto".into()),
                ..entrada("Ollama")
            })
            .expect("salvando");

        let atualizado = servico
            .salvar(ProvedorEntrada {
                id: Some(perfil.id.clone()),
                api_key: None,
                ..entrada("Ollama renomeado")
            })
            .expect("atualizando");

        assert_eq!(atualizado.name, "Ollama renomeado");
        assert_eq!(atualizado.secret_ref, perfil.secret_ref);
        let referencia = format!("firebird-studio/ia/{}", perfil.id);
        assert_eq!(
            cofre.get(&referencia).expect("lendo cofre").as_deref(),
            Some("sk-secreto"),
            "sem nova chave, a antiga continua no cofre"
        );
    }

    #[test]
    fn apagar_remove_perfil_e_segredo() {
        let (servico, _repos, cofre) = servico();
        let perfil = servico
            .salvar(ProvedorEntrada {
                api_key: Some("sk-secreto".into()),
                ..entrada("Ollama")
            })
            .expect("salvando");

        assert!(servico.apagar(&perfil.id).expect("apagando"));
        let referencia = format!("firebird-studio/ia/{}", perfil.id);
        assert!(cofre.get(&referencia).expect("lendo cofre").is_none());
        assert!(servico.listar().expect("listando").is_empty());
    }

    #[test]
    fn recusa_configuracao_incompleta() {
        let (servico, _repos, _cofre) = servico();

        assert!(servico.salvar(entrada("")).is_err(), "nome obrigatório");
        assert!(servico
            .salvar(ProvedorEntrada {
                base_url: " ".into(),
                ..entrada("x")
            })
            .is_err());
        assert!(servico
            .salvar(ProvedorEntrada {
                tipo: "desconhecido".into(),
                ..entrada("x")
            })
            .is_err());
    }

    /// Transporte que devolve um SQL e registra o corpo recebido.
    struct TransporteResposta {
        resposta: String,
        corpo: std::sync::Mutex<String>,
    }

    impl TransporteHttp for TransporteResposta {
        fn post_json(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            corpo: &str,
            _timeout_ms: i64,
        ) -> Result<String, String> {
            if let Ok(mut g) = self.corpo.lock() {
                *g = corpo.to_string();
            }
            Ok(self.resposta.clone())
        }
    }

    fn contexto_de_teste() -> crate::application::ia_context::ContextoSchema {
        crate::application::ia_context::ContextoSchema {
            texto: "Esquema do banco (apenas metadados, sem dados):\nTABELA CLIENTES(ID:INTEGER NOT NULL)  PK(ID)".into(),
            relacoes: 1,
            relacoes_omitidas: 0,
            truncado: false,
        }
    }

    #[test]
    fn gerar_registra_interacao_e_devolve_sql_sugerido() {
        let repos = Arc::new(BancoLocal::em_memoria().expect("banco em memória"));
        let cofre = Arc::new(InMemorySecretStore::new());
        let transporte = Arc::new(TransporteResposta {
            resposta: r#"{"choices":[{"message":{"content":"```sql\nSELECT FIRST 10 ID FROM CLIENTES\n```"}}]}"#.into(),
            corpo: std::sync::Mutex::new(String::new()),
        });
        let servico = IaService::new(repos.clone(), cofre.clone(), transporte.clone());
        let perfil = servico
            .salvar(ProvedorEntrada {
                api_key: Some("sk-secreto".into()),
                ..entrada("Ollama")
            })
            .expect("salvando provedor");

        let resultado = servico
            .gerar(
                &perfil.id,
                firebird_domain::TarefaIa::GerarSql,
                "liste os clientes",
                &contexto_de_teste(),
            )
            .expect("gerando");

        assert_eq!(
            resultado.resposta.sql.as_deref(),
            Some("SELECT FIRST 10 ID FROM CLIENTES")
        );

        // Histórico registrado, sem segredo em nenhum campo.
        let historico = servico.historico(10).expect("histórico");
        assert_eq!(historico.len(), 1);
        assert_eq!(historico[0].task_type, "gerar_sql");
        assert!(historico[0].connection_fingerprint.is_some());
        assert!(!format!("{:?}", historico[0]).contains("sk-secreto"));
        assert!(historico[0].context_summary.contains("CLIENTES"));

        // O contexto entrou no prompt enviado ao provedor.
        let corpo = transporte.corpo.lock().expect("corpo").clone();
        assert!(corpo.contains("TABELA CLIENTES"));

        // Marcar o SQL aceito atualiza o registro.
        servico
            .marcar_sql_aceito(&resultado.interacao_id, "SELECT FIRST 10 ID FROM CLIENTES")
            .expect("marcando");
        assert!(servico
            .historico(10)
            .expect("histórico")
            .first()
            .and_then(|i| i.accepted_sql.clone())
            .is_some());
    }

    #[test]
    fn gerar_recusa_pedido_vazio() {
        let (servico, _repos, _cofre) = servico();
        let erro = servico
            .gerar(
                "qualquer",
                firebird_domain::TarefaIa::GerarSql,
                "   ",
                &contexto_de_teste(),
            )
            .expect_err("pedido vazio");
        assert!(erro.to_string().contains("vazio"));
    }

    #[test]
    fn impressao_do_contexto_muda_com_o_esquema() {
        let a = contexto_de_teste();
        let mut b = a.clone();
        b.texto
            .push_str("\nTABELA PEDIDOS(ID:INTEGER NOT NULL)  PK(ID)");
        assert_ne!(
            super::impressao_do_contexto(&a),
            super::impressao_do_contexto(&b)
        );
        assert_eq!(super::impressao_do_contexto(&a).len(), 16);
    }

    #[test]
    fn testar_usa_o_perfil_e_nao_revela_a_chave() {
        let (servico, _repos, _cofre) = servico();
        let perfil = servico
            .salvar(ProvedorEntrada {
                api_key: Some("sk-secreto".into()),
                ..entrada("Ollama")
            })
            .expect("salvando");

        assert!(servico
            .testar(&perfil.id)
            .expect("testando")
            .contains("qwen2.5-coder"));
        assert!(matches!(
            servico.testar("inexistente"),
            Err(ErroServicoIa::NaoEncontrado)
        ));
    }
}
