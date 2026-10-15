//! Adapter falso para testes de catálogo, sem servidor e sem rede.
//!
//! É um item **público** de propósito (não `#[cfg(test)]`): será reutilizado
//! pelos testes de integração por versão da Fase 0 (Task 4). Cada regra liga
//! um marcador de SQL às linhas sintéticas devolvidas quando o SQL contém o
//! marcador, e todo SQL executado fica registrado em
//! [`FakeAdapter::executed_sqls`] para asserção da variante de catálogo.

use firebird_domain::{DomainError, ServerVersion};

use crate::FirebirdAdapter;

/// Adapter de teste determinístico: responde por marcador de SQL e grava o
/// histórico de consultas.
pub struct FakeAdapter {
    /// Regras `(marcador, linhas)`, na ordem de prioridade.
    regras: Vec<(String, Vec<Vec<Option<String>>>)>,
    /// Todos os SQLs passados a [`FirebirdAdapter::query_rows`], em ordem.
    sqls_executados: Vec<String>,
}

impl FakeAdapter {
    /// Cria o fake com regras `(marcador, linhas)`: o primeiro SQL que contém
    /// `marcador` devolve `linhas` (clonadas). SQL sem regra casa vira
    /// [`DomainError::Catalog`] — assim, teste que esquece fixture falha alto.
    pub fn new(regras: Vec<(&str, Vec<Vec<Option<String>>>)>) -> Self {
        Self {
            regras: regras
                .into_iter()
                .map(|(marcador, linhas)| (marcador.to_string(), linhas))
                .collect(),
            sqls_executados: Vec::new(),
        }
    }

    /// SQLs executados até aqui, na ordem — para asserir qual variante de
    /// catálogo (`RDB$RELATION_TYPE` vs `RDB$VIEW_TYPE` etc.) foi usada.
    pub fn executed_sqls(&self) -> &[String] {
        &self.sqls_executados
    }
}

impl FirebirdAdapter for FakeAdapter {
    fn server_version(&mut self) -> Result<ServerVersion, DomainError> {
        // O repositório recebe a versão explícita nos testes; o fake não a
        // descobre para não simular o que ainda não foi provado contra servidor.
        Err(DomainError::Driver {
            message: "FakeAdapter não expõe server_version; injete a versão no repositório"
                .to_string(),
        })
    }

    fn query_rows(&mut self, sql: &str) -> Result<Vec<Vec<Option<String>>>, DomainError> {
        self.sqls_executados.push(sql.to_string());
        match self
            .regras
            .iter()
            .find(|(marcador, _)| sql.contains(marcador))
        {
            Some((_, linhas)) => Ok(linhas.clone()),
            None => Err(DomainError::Catalog {
                message: format!("FakeAdapter: nenhuma regra para SQL contendo '{sql}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Linhas sintéticas de exemplo (nunca dado real).
    fn linhas_sinteticas() -> Vec<Vec<Option<String>>> {
        vec![vec![Some("CLIENTES".to_string()), Some("0".to_string())]]
    }

    #[test]
    fn devolve_linhas_do_primeiro_marcador_que_casa() {
        let linhas = linhas_sinteticas();
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_TYPE", linhas.clone())]);

        let obtidas = fake
            .query_rows("SELECT RDB$RELATION_NAME, RDB$RELATION_TYPE FROM RDB$RELATIONS")
            .expect("marcador presente deve devolver as linhas");

        assert_eq!(obtidas, linhas);
    }

    #[test]
    fn registra_sql_executado_para_assercao_de_variante() {
        let mut fake = FakeAdapter::new(vec![("QUALQUER", vec![])]);

        let _ = fake.query_rows("SELECT 1 FROM RDB$DATABASE");

        assert_eq!(fake.executed_sqls().len(), 1);
        assert!(
            fake.executed_sqls()[0].contains("RDB$DATABASE"),
            "o SQL executado deve ficar registrado: {:?}",
            fake.executed_sqls()
        );
    }

    #[test]
    fn sql_sem_regra_retorna_erro_de_catalogo() {
        let mut fake = FakeAdapter::new(vec![]);

        let erro = fake
            .query_rows("SELECT 1")
            .expect_err("SQL sem regra deve falhar alto, não devolver vazio");

        assert!(
            matches!(erro, DomainError::Catalog { .. }),
            "deve virar DomainError::Catalog: {erro:?}"
        );
    }

    #[test]
    fn server_version_retorna_erro_explicito() {
        let mut fake = FakeAdapter::new(vec![]);

        let erro = fake.server_version().expect_err("fake não tem versão");

        assert!(
            matches!(erro, DomainError::Driver { .. }),
            "a versão deve ser injetada no repositório: {erro:?}"
        );
    }
}
