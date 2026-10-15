//! Checklists de integração por versão do Firebird (Fase 0 — PoC, Task 4).
//!
//! Os testes vivos ficam em `tests/poc_checklist.rs` e rodam apenas contra um
//! servidor real quando as env vars `FB_POC_HOST/PORT/DATABASE/USER/PASSWORD/
//! LIB_PATH` estão definidas (ambiente provido por `scripts/firebird-env.sh`
//! ou pelo servidor local). Sem env, são ignorados com aviso — nunca falham o
//! `cargo test` local.
//!
//! Este módulo concentra a **pré-checagem de schema**, uma regra de segurança
//! do harness: o checklist pode ser apontado para um banco que não é
//! descartável, então jamais derruba `CLIENTES`/`V_CLIENTES` que não foram
//! criados por esta execução. A lógica mora aqui (e não no teste) para ser
//! verificável **sem servidor**, via [`FakeAdapter`].

use firebird_adapter::MetadataRepository;
use firebird_domain::{RelationSummary, ServerVersion};

/// Nomes de objeto do schema sintético do checklist.
pub const OBJETOS_SINTETICOS: [&str; 2] = ["CLIENTES", "V_CLIENTES"];

/// Decide se o schema sintético pode ser criado com segurança.
///
/// Retorna `Some(mensagem)` — com o texto que explica a recusa — quando
/// `CLIENTES` ou `V_CLIENTES` **já existem** no banco de destino; `None` quando
/// o banco está livre e o setup pode prosseguir.
///
/// A função não distingue se o objeto é tabela ou view de propósito: qualquer
/// dos dois nomes significa "schema de alguém" e precisa de recusa.
pub fn recusa_se_schema_pre_existe(relacoes: &[RelationSummary]) -> Option<String> {
    let conflitos: Vec<&str> = relacoes
        .iter()
        .map(|r| r.name.as_str())
        .filter(|nome| OBJETOS_SINTETICOS.contains(nome))
        .collect();

    if conflitos.is_empty() {
        return None;
    }

    Some(format!(
        "{} já {} no banco de destino; recusa criar ou derrubar schema que não é \
         desta execução. Se for resíduo de uma execução anterior do harness, remova \
         manualmente (DROP VIEW V_CLIENTES; DROP TABLE CLIENTES;) e rode de novo.",
        conflitos.join("/"),
        if conflitos.len() == 1 {
            "existe"
        } else {
            "existem"
        },
    ))
}

/// Aplica [`recusa_se_schema_pre_existe`] sobre o catálogo real do servidor.
pub fn pre_checagem(
    adapter: &mut dyn firebird_adapter::FirebirdAdapter,
    versao: Option<&ServerVersion>,
) -> Result<Option<String>, firebird_domain::DomainError> {
    let versao = match versao {
        Some(versao) => versao,
        None => {
            return Ok(Some(
                "sem server_version (item 1), a pré-checagem não pode garantir que o banco \
                 está livre — recusa criar"
                    .to_string(),
            ))
        }
    };

    let relacoes = MetadataRepository::new().relations(adapter, versao)?;
    Ok(recusa_se_schema_pre_existe(&relacoes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use firebird_adapter::FakeAdapter;
    use firebird_domain::RelationKind;

    fn relacao(nome: &str) -> RelationSummary {
        RelationSummary {
            name: nome.to_string(),
            kind: RelationKind::Table,
        }
    }

    #[test]
    fn banco_livre_permite_o_setup() {
        let relacoes = vec![relacao("PEDIDOS"), relacao("V_PEDIDOS")];
        assert_eq!(recusa_se_schema_pre_existe(&relacoes), None);
    }

    #[test]
    fn banco_vazio_permite_o_setup() {
        assert_eq!(recusa_se_schema_pre_existe(&[]), None);
    }

    #[test]
    fn tabela_sintetica_pre_existente_e_recusada() {
        let recusa = recusa_se_schema_pre_existe(&[relacao("PEDIDOS"), relacao("CLIENTES")])
            .expect("deveria recusar");

        assert!(
            recusa.contains("CLIENTES"),
            "mensagem cita o objeto: {recusa}"
        );
        assert!(
            recusa.contains("existe"),
            "mensagem explica a recusa: {recusa}"
        );
        assert!(
            recusa.contains("desta execução"),
            "mensagem delimita o escopo: {recusa}"
        );
    }

    #[test]
    fn objeto_sintetico_e_recusado_pelo_nome_mesmo_com_kind_diferente() {
        let recusa =
            recusa_se_schema_pre_existe(&[relacao("V_CLIENTES")]).expect("deveria recusar");

        assert!(recusa.contains("V_CLIENTES"));
        assert!(recusa.contains("existe"));
        // A mensagem precisa orientar a remediação: sem server_version/pré-checagem
        // quebrada, o operador ficaria sem caminho para destravar o harness.
        assert!(recusa.contains("DROP TABLE CLIENTES"), "{recusa}");
    }

    #[test]
    fn ambos_pre_existentes_sao_listados_e_conjugado() {
        let recusa =
            recusa_se_schema_pre_existe(&[relacao("CLIENTES"), relacao("V_CLIENTES")]).unwrap();

        assert!(
            recusa.starts_with("CLIENTES/V_CLIENTES já existem"),
            "{recusa}"
        );
    }

    #[test]
    fn nomes_similares_nao_disparam_a_recusa() {
        // Só os objetos exatos bloqueiam: um banco do cliente com `CLIENTES_X`
        // ou `V_CLIENTES_2` continua utilizável pelo harness.
        let relacoes = vec![
            relacao("CLIENTES_X"),
            relacao("V_CLIENTES_2"),
            relacao("meu_clientes"),
        ];
        assert_eq!(recusa_se_schema_pre_existe(&relacoes), None);
    }

    #[test]
    fn pre_checagem_sem_versao_recusa() {
        let mut fake = FakeAdapter::new(vec![]);
        let recusa = pre_checagem(&mut fake, None)
            .expect("sem rede")
            .expect("deveria recusar");
        assert!(recusa.contains("recusa criar"), "{recusa}");
    }

    #[test]
    fn pre_checagem_usa_o_catalogo_real_via_adapter() {
        let versao = ServerVersion::parse("WI-V3.0.10.33268 Firebird 3.0 WI-V").unwrap();
        let mut fake = FakeAdapter::new(vec![(
            "RDB$RELATION_TYPE",
            vec![
                vec![Some("CLIENTES".to_string()), Some("0".to_string())],
                vec![Some("V_CLIENTES".to_string()), Some("1".to_string())],
            ],
        )]);

        let recusa = pre_checagem(&mut fake, Some(&versao)).expect("sem rede");

        assert!(
            recusa.is_some_and(|r| r.contains("CLIENTES")),
            "deveria recusar"
        );
        assert!(
            fake.executed_sqls()
                .iter()
                .any(|sql| sql.contains("RDB$RELATION_TYPE")),
            "a pré-checagem deve ler o catálogo pela variante moderna"
        );
    }

    #[test]
    fn pre_checagem_permite_quando_catalogo_nao_tem_os_objetos() {
        let versao = ServerVersion::parse("WI-V3.0.10.33268 Firebird 3.0 WI-V").unwrap();
        let mut fake = FakeAdapter::new(vec![(
            "RDB$RELATION_TYPE",
            vec![vec![Some("PEDIDOS".to_string()), Some("0".to_string())]],
        )]);

        let recusa = pre_checagem(&mut fake, Some(&versao)).expect("sem rede");

        assert!(recusa.is_none());
    }
}
