//! Contexto do schema e templates versionados — Passo 2 da Fase 3 (Spec 07).
//!
//! O contexto é montado **só de metadados** (nomes de relações, colunas, tipos e
//! chaves). Nunca entra valor de dado: o assistente precisa saber *a forma* do
//! banco, não o conteúdo das tabelas.
//!
//! O texto é limitado por três tetos (tabelas, colunas por tabela e caracteres
//! totais) e o corte é sinalizado com `truncado` — o modelo recebe um aviso
//! explícito em vez de um esquema silenciosamente incompleto.

use std::sync::Arc;

use firebird_adapter::{FirebirdAdapter, MetadataRepository};
use firebird_domain::{ErroIa, TarefaIa};

/// Tetos do contexto (proteção de prompt e de memória).
pub const MAX_TABELAS: usize = 60;
/// Colunas descritas por relação.
pub const MAX_COLUNAS_POR_TABELA: usize = 40;
/// Tamanho máximo do contexto em caracteres.
pub const MAX_CARACTERES: usize = 12_000;

/// Contexto do schema pronto para o prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextoSchema {
    /// Texto do contexto (linhas `TABELA (TIPO, PK...)`).
    pub texto: String,
    /// Quantas relações entraram.
    pub relacoes: usize,
    /// Quantas relações foram deixadas de fora pelo teto.
    pub relacoes_omitidas: usize,
    /// `true` quando algum teto foi atingido.
    pub truncado: bool,
}

/// Monta o contexto de um perfil conectado.
#[derive(Debug, Clone, Default)]
pub struct ContextoBuilder {
    /// Teto de relações (override em teste).
    pub max_tabelas: usize,
    /// Teto de colunas por relação.
    pub max_colunas: usize,
    /// Teto de caracteres.
    pub max_caracteres: usize,
    /// Inclui objetos de sistema do catálogo.
    pub incluir_sistema: bool,
}

impl ContextoBuilder {
    /// Teto de relações.
    pub fn max_tabelas(&self) -> usize {
        if self.max_tabelas == 0 {
            MAX_TABELAS
        } else {
            self.max_tabelas
        }
    }

    /// Teto de colunas por relação.
    pub fn max_colunas(&self) -> usize {
        if self.max_colunas == 0 {
            MAX_COLUNAS_POR_TABELA
        } else {
            self.max_colunas
        }
    }

    /// Teto de caracteres do contexto.
    pub fn max_caracteres(&self) -> usize {
        if self.max_caracteres == 0 {
            MAX_CARACTERES
        } else {
            self.max_caracteres
        }
    }

    /// Monta o contexto a partir do catálogo do perfil.
    pub fn construir(
        &self,
        adapter: &mut dyn FirebirdAdapter,
        versao: &firebird_domain::ServerVersion,
    ) -> Result<ContextoSchema, ErroIa> {
        let repo = MetadataRepository::new();
        let mut relacoes = repo
            .relations_com_filtro(adapter, versao, None, self.incluir_sistema)
            .map_err(|e| ErroIa::Indisponivel(e.redacted()))?;
        relacoes.sort_by(|a, b| a.name.cmp(&b.name));

        let teto = self.max_tabelas();
        let total = relacoes.len();
        let selecionadas: Vec<_> = relacoes.into_iter().take(teto).collect();
        let omitidas = total.saturating_sub(selecionadas.len());

        let mut linhas: Vec<String> = Vec::with_capacity(selecionadas.len());
        let mut truncado = omitidas > 0;
        let mut usado = 0usize;

        for relacao in selecionadas {
            let campos = repo
                .relation_fields(adapter, &relacao.name)
                .map_err(|e| ErroIa::Indisponivel(e.redacted()))?;
            let pk = repo
                .chave_primaria(adapter, &relacao.name)
                .unwrap_or_default();

            let mut descricao = String::new();
            for campo in campos.iter().take(self.max_colunas()) {
                if !descricao.is_empty() {
                    descricao.push_str(", ");
                }
                descricao.push_str(&campo.name);
                descricao.push(':');
                descricao.push_str(&campo.type_name);
                if !campo.nullable {
                    descricao.push_str(" NOT NULL");
                }
            }
            if campos.len() > self.max_colunas() {
                descricao.push_str(", …");
                truncado = true;
            }

            let mut linha = format!(
                "{} {}({})",
                rotulo_do_tipo(&relacao.kind),
                relacao.name,
                descricao
            );
            if !pk.is_empty() {
                linha.push_str(&format!("  PK({})", pk.join(",")));
            }
            if campos.len() > self.max_colunas() {
                linha.push_str(&format!("  [{} colunas]", campos.len()));
            }

            if usado + linha.len() + 1 > self.max_caracteres() {
                truncado = true;
                break;
            }
            usado += linha.len() + 1;
            linhas.push(linha);
        }

        let mut texto = String::from("Esquema do banco (apenas metadados, sem dados):\n");
        texto.push_str(&linhas.join("\n"));
        if truncado {
            texto.push_str(
                "\n… contexto truncado por limite de tamanho: o esquema acima está incompleto.",
            );
        }

        Ok(ContextoSchema {
            relacoes: linhas.len(),
            relacoes_omitidas: omitidas,
            texto,
            truncado,
        })
    }
}

/// Rótulo curto do tipo da relação (usado no contexto).
fn rotulo_do_tipo(kind: &firebird_domain::RelationKind) -> &'static str {
    use firebird_domain::RelationKind as K;
    match kind {
        K::Table => "TABELA",
        K::View => "VIEW",
        K::Other => "OBJETO",
    }
}

/// Um template versionado de prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateIa {
    /// Tarefa atendida.
    pub tarefa: TarefaIa,
    /// Versão do template (muda quando o prompt muda).
    pub versao: u32,
    /// Instrução específica da tarefa.
    pub instrucao: &'static str,
}

/// Versão atual dos templates — incrementada a cada mudança de prompt.
pub const VERSAO_TEMPLATES: u32 = 1;

/// Templates disponíveis, na ordem exibida na UI.
pub fn templates() -> &'static [TemplateIa] {
    &[
        TemplateIa {
            tarefa: TarefaIa::GerarSql,
            versao: VERSAO_TEMPLATES,
            instrucao: "Gere o SQL Firebird que responde ao pedido. Use apenas tabelas e colunas \
                        listadas no esquema; se faltar informação, diga o que falta.",
        },
        TemplateIa {
            tarefa: TarefaIa::ExplicarSql,
            versao: VERSAO_TEMPLATES,
            instrucao: "Explique o SQL enviado, objeto por objeto, em português do Brasil.",
        },
        TemplateIa {
            tarefa: TarefaIa::CorrigirSql,
            versao: VERSAO_TEMPLATES,
            instrucao: "Corrija o SQL enviado e devolva apenas o SQL Firebird corrigido, sem \
                        comentários fora do bloco ```sql.",
        },
        TemplateIa {
            tarefa: TarefaIa::RevisarSql,
            versao: VERSAO_TEMPLATES,
            instrucao: "Revise o SQL enviado e liste riscos concretos (tempo, locks, FULL TABLE \
                        SCAN, perda de dados), um por item.",
        },
    ]
}

/// Template de uma tarefa (ou `None` se a tarefa não tiver template).
pub fn template_de(tarefa: TarefaIa) -> Option<&'static TemplateIa> {
    templates().iter().find(|t| t.tarefa == tarefa)
}

/// Monta o prompt final (instrução + contexto + pergunta), já redigido.
pub fn montar_prompt(template: &TemplateIa, contexto: &ContextoSchema, entrada: &str) -> String {
    format!(
        "{}\n\n{}\n\n{}\n\n{}",
        template.instrucao, contexto.texto, entrada, REGRA_SEGURANCA
    )
}

/// Bloco fixo: lembrete de que o modelo não deve inventar objetos nem executar.
pub const REGRA_SEGURANCA: &str =
    "Não invente tabelas ou colunas que não estejam no esquema. Responda em português do Brasil.";

/// Provedor de contexto compartilhado entre comandos (cache por sessão opcional).
pub type ContextoCompartilhado = Arc<ContextoBuilder>;

#[cfg(test)]
mod tests {
    use super::*;
    use firebird_adapter::FakeAdapter;

    /// Catálogo sintético: 2 relações com colunas e PK.
    fn catalogo() -> Vec<(&'static str, Vec<Vec<Option<String>>>)> {
        vec![
            (
                "RDB$RELATIONS",
                vec![
                    vec![
                        Some("CLIENTES".to_string()),
                        Some("0".to_string()),
                        Some("128".to_string()),
                        Some("12".to_string()),
                    ],
                    vec![
                        Some("PEDIDOS".to_string()),
                        Some("0".to_string()),
                        Some("128".to_string()),
                        Some("3".to_string()),
                    ],
                ],
            ),
            (
                "RDB$RELATION_FIELDS",
                vec![
                    // FIELD_NAME, FIELD_TYPE, SUB_TYPE, RF.NULL_FLAG, F.NULL_FLAG,
                    // RF.DEFAULT_SOURCE, F.DEFAULT_SOURCE, FIELD_SOURCE, POSITION
                    vec![
                        Some("ID".to_string()),
                        Some("8".to_string()),
                        None,
                        Some("1".to_string()),
                        Some("1".to_string()),
                        None,
                        None,
                        Some("RDB$1".to_string()),
                        Some("0".to_string()),
                    ],
                    vec![
                        Some("NOME".to_string()),
                        Some("37".to_string()),
                        None,
                        None,
                        None,
                        None,
                        None,
                        Some("RDB$2".to_string()),
                        Some("1".to_string()),
                    ],
                ],
            ),
            ("RDB$INDEX_SEGMENTS", vec![vec![Some("ID".to_string())]]),
        ]
    }

    fn adapter() -> FakeAdapter {
        FakeAdapter::new(catalogo())
    }

    #[test]
    fn contexto_traz_tabela_colunas_e_pk() {
        let mut fake = adapter();
        let versao = firebird_domain::ServerVersion {
            major: 4,
            minor: 0,
            patch: None,
        };

        let contexto = ContextoBuilder::default()
            .construir(&mut fake, &versao)
            .expect("contexto");

        assert!(contexto.texto.contains("CLIENTES"), "{}", contexto.texto);
        assert!(
            contexto.texto.contains("ID:INTEGER NOT NULL"),
            "{}",
            contexto.texto
        );
        assert!(contexto.texto.contains("NOME:VARCHAR"));
        assert!(contexto.texto.contains("PK(ID)"));
        assert!(!contexto.truncado);
        assert!(!contexto.texto.to_lowercase().contains("masterkey"));
    }

    #[test]
    fn contexto_respeita_teto_de_caracteres() {
        let mut fake = adapter();
        let versao = firebird_domain::ServerVersion {
            major: 4,
            minor: 0,
            patch: None,
        };

        let contexto = ContextoBuilder {
            max_caracteres: 60,
            ..Default::default()
        }
        .construir(&mut fake, &versao)
        .expect("contexto");

        assert!(contexto.truncado);
        assert!(contexto.texto.contains("truncado"));
        assert!(contexto.texto.len() < 400);
    }

    #[test]
    fn contexto_omite_relacoes_alem_do_teto() {
        let mut fake = adapter();
        let versao = firebird_domain::ServerVersion {
            major: 4,
            minor: 0,
            patch: None,
        };

        let contexto = ContextoBuilder {
            max_tabelas: 1,
            ..Default::default()
        }
        .construir(&mut fake, &versao)
        .expect("contexto");

        assert_eq!(contexto.relacoes, 1);
        assert_eq!(contexto.relacoes_omitidas, 1);
        assert!(contexto.truncado);
    }

    #[test]
    fn contexto_ignora_teto_zero_como_padrao() {
        let builder = ContextoBuilder::default();
        assert_eq!(builder.max_tabelas(), MAX_TABELAS);
        assert_eq!(builder.max_colunas(), MAX_COLUNAS_POR_TABELA);
        assert_eq!(builder.max_caracteres(), MAX_CARACTERES);
    }

    #[test]
    fn templates_sao_versionados_e_completos() {
        let todos = templates();
        assert_eq!(todos.len(), 4);
        for t in todos {
            assert_eq!(t.versao, VERSAO_TEMPLATES);
            assert!(!t.instrucao.trim().is_empty());
        }
        assert_eq!(
            template_de(TarefaIa::CorrigirSql).expect("template").tarefa,
            TarefaIa::CorrigirSql
        );
        assert!(template_de(TarefaIa::GerarSql).is_some());
    }

    #[test]
    fn prompt_tem_instrucao_contexto_entrada_e_regra() {
        let contexto = ContextoSchema {
            texto: "Esquema do banco (apenas metadados, sem dados):\nTABELA T(A:INTEGER NOT NULL)  PK(A)".into(),
            relacoes: 1,
            relacoes_omitidas: 0,
            truncado: false,
        };
        let template = template_de(TarefaIa::GerarSql).expect("template");
        let prompt = montar_prompt(template, &contexto, "liste os clientes");

        assert!(prompt.contains("Gere o SQL Firebird"));
        assert!(prompt.contains("TABELA T(A:INTEGER NOT NULL)"));
        assert!(prompt.contains("liste os clientes"));
        assert!(prompt.contains("Não invente tabelas"));
    }
}
