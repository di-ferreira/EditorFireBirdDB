//! Serviço do assistente de IA — Passo 1 da Fase 3.
//!
//! Junta as três peças mantendo a separação de responsabilidades:
//! * **SQLite local** guarda o perfil (sem chave);
//! * **cofre do sistema** guarda a chave da API (`secret_ref` aponta para lá);
//! * **provedor** faz a chamada HTTP, com transporte injetado (testes sem rede).

use std::sync::Arc;

use firebird_domain::{ErroIa, ProvedorIaConfig, TipoProvedorIa};
use firebird_security::SecretStore;
use firebird_storage::{agora_iso8601, AiProviderProfile};

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
