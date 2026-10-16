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
    /// Todos os SQLs passados a [`FirebirdAdapter::query_rows`] ou
    /// [`FirebirdAdapter::query_rows_params`], em ordem.
    sqls_executados: Vec<String>,
    /// Params de cada chamada parametrizada ([`FirebirdAdapter::query_rows_params`]
    /// e [`FirebirdAdapter::execute`]), na mesma ordem das chamadas.
    params_executados: Vec<Vec<Option<String>>>,
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
            params_executados: Vec::new(),
        }
    }

    /// SQLs executados até aqui, na ordem — para asserir qual variante de
    /// catálogo (`RDB$RELATION_TYPE` vs `RDB$VIEW_TYPE` etc.) foi usada.
    /// Conta também os SQLs passados a
    /// [`FirebirdAdapter::query_rows_params`].
    pub fn executed_sqls(&self) -> &[String] {
        &self.sqls_executados
    }

    /// Parâmetros posicionais de cada chamada parametrizada
    /// ([`FirebirdAdapter::query_rows_params`] e [`FirebirdAdapter::execute`]),
    /// na ordem das chamadas — para asserir que o parâmetro certo foi enviado
    /// (ex.: `?` do SELECT).
    pub fn executed_params(&self) -> &[Vec<Option<String>>] {
        &self.params_executados
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
        self.buscar_regra(sql)
    }

    fn query_rows_params(
        &mut self,
        sql: &str,
        params: &[Option<&str>],
    ) -> Result<Vec<Vec<Option<String>>>, DomainError> {
        self.registrar_e_buscar(sql, params)
    }

    fn execute(&mut self, sql: &str, params: &[Option<&str>]) -> Result<usize, DomainError> {
        // O fake não simula contagem de linhas afetadas: regra casa → Ok(0).
        self.registrar_e_buscar(sql, params).map(|_| 0)
    }

    fn query_rows_with_columns(
        &mut self,
        sql: &str,
        params: &[Option<&str>],
    ) -> Result<(Vec<String>, Vec<Vec<Option<String>>>), DomainError> {
        let linhas = self.registrar_e_buscar(sql, params)?;
        Ok((Vec::new(), linhas))
    }
}

impl FakeAdapter {
    /// Registra SQL e params e resolve o SQL contra as regras por marcador
    /// (primeiro que casa).
    fn registrar_e_buscar(
        &mut self,
        sql: &str,
        params: &[Option<&str>],
    ) -> Result<Vec<Vec<Option<String>>>, DomainError> {
        self.sqls_executados.push(sql.to_string());
        self.params_executados
            .push(params.iter().map(|p| p.map(ToString::to_string)).collect());
        self.buscar_regra(sql)
    }

    /// Resolve o SQL contra as regras por marcador (primeiro que casa).
    fn buscar_regra(&mut self, sql: &str) -> Result<Vec<Vec<Option<String>>>, DomainError> {
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
    fn query_rows_params_devolve_linhas_do_marcador_como_query_rows() {
        let linhas = vec![vec![Some("O'BRIEN".to_string())]];
        let mut fake = FakeAdapter::new(vec![("WHERE NOME =", linhas.clone())]);

        let obtidas = fake
            .query_rows_params(
                "SELECT NOME FROM CLIENTES WHERE NOME = ?",
                &[Some("O'BRIEN")],
            )
            .expect("marcador presente deve devolver as linhas");

        assert_eq!(obtidas, linhas);
        assert_eq!(
            fake.executed_sqls().len(),
            1,
            "o SQL parametrizado deve ser registrado como executado"
        );
    }

    #[test]
    fn query_rows_params_registra_os_params_enviados() {
        let mut fake = FakeAdapter::new(vec![("CLIENTES", vec![])]);

        let _ = fake.query_rows_params(
            "INSERT INTO CLIENTES (NOME, FLAG) VALUES (?, ?)",
            &[Some("João"), None],
        );

        assert_eq!(
            fake.executed_params(),
            &[vec![Some("João".to_string()), None]],
            "os params (com None = NULL) devem ficar registrados para asserção"
        );
    }

    #[test]
    fn query_rows_params_sem_regra_retorna_erro_de_catalogo() {
        let mut fake = FakeAdapter::new(vec![]);

        let erro = fake
            .query_rows_params("SELECT 1 FROM RDB$DATABASE WHERE ? = ?", &[Some("a"), None])
            .expect_err("SQL sem regra deve falhar alto, não devolver vazio");

        assert!(
            matches!(erro, DomainError::Catalog { .. }),
            "deve virar DomainError::Catalog: {erro:?}"
        );
    }

    #[test]
    fn execute_registra_sql_e_params_e_devolve_ok_sem_linhas() {
        let mut fake = FakeAdapter::new(vec![("INSERT INTO CLIENTES", vec![])]);

        let afetadas = fake
            .execute(
                "INSERT INTO CLIENTES (NOME, FLAG) VALUES (?, ?)",
                &[Some("João"), None],
            )
            .expect("regra presente deve devolver Ok");

        assert_eq!(afetadas, 0, "o fake não simula contagem de linhas");
        assert_eq!(fake.executed_sqls().len(), 1);
        assert_eq!(
            fake.executed_params(),
            &[vec![Some("João".to_string()), None]],
            "os params do execute devem ficar registrados"
        );
    }

    #[test]
    fn execute_sem_regra_retorna_erro_de_catalogo() {
        let mut fake = FakeAdapter::new(vec![]);

        let erro = fake
            .execute("CREATE TABLE X (Y INTEGER)", &[])
            .expect_err("SQL sem regra deve falhar alto");

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
