//! Consultas de catálogo `RDB$` com variantes por versão do servidor.
//!
//! A compatibilidade com Firebird 2.0–5.0 é **provada**, não presumida
//! (Spec 01, §1): consultas que citam colunas de versões novas só são usadas
//! quando [`ServerVersion::supports_relation_type`] confirma o suporte, com
//! variante fallback para o Firebird 2.0. Campos não suportados pelo servidor
//! viram ausência (`None`); nada é inventado (Spec 01, §4).

use firebird_domain::{DomainError, FieldInfo, RelationKind, RelationSummary, ServerVersion};

use crate::FirebirdAdapter;

/// Repositório de metadados do catálogo Firebird, por versão.
///
/// Estadoless: cada método recebe o [`FirebirdAdapter`] conectado.
pub struct MetadataRepository;

impl MetadataRepository {
    /// Cria o repositório (sem estado).
    pub fn new() -> Self {
        MetadataRepository
    }

    /// Lista relações de usuário (tabelas e views) do catálogo.
    ///
    /// Usa `RDB$RELATION_TYPE` (>= 2.1) quando suportado; para 2.0, cai para
    /// `RDB$VIEW_TYPE` — `NULL` = tabela base, não-`NULL` = view. Ambas as
    /// variantes excluem objetos de sistema e ordenam por nome.
    pub fn relations(
        &self,
        adapter: &mut dyn FirebirdAdapter,
        version: &ServerVersion,
    ) -> Result<Vec<RelationSummary>, DomainError> {
        let moderna = version.supports_relation_type();
        let sql = if moderna {
            SQL_RELATIONS_MODERNA
        } else {
            SQL_RELATIONS_FALLBACK
        };

        let linhas = adapter.query_rows(sql)?;
        let mut relacoes = Vec::with_capacity(linhas.len());
        for linha in &linhas {
            let nome = nome_de_catalogo(linha, "RDB$RELATIONS")?;
            let tipo = linha.get(1).and_then(Option::as_deref);
            let kind = if moderna {
                classificar_por_relation_type(tipo)
            } else {
                classificar_por_view_type(tipo)
            };
            relacoes.push(RelationSummary {
                name: limpar_char(nome),
                kind,
            });
        }
        Ok(relacoes)
    }

    /// Lista as colunas de uma tabela ou view, com tipo, domínio, nullability
    /// e default — derivados do catálogo, sem inventar valores ausentes.
    pub fn relation_fields(
        &self,
        adapter: &mut dyn FirebirdAdapter,
        relation: &str,
    ) -> Result<Vec<FieldInfo>, DomainError> {
        let sql = SQL_RELATION_FIELDS.replace("{}", &escapar_literal(relation));
        let linhas = adapter.query_rows(&sql)?;
        linhas.iter().map(|linha| campo_de_linha(linha)).collect()
    }
}

impl Default for MetadataRepository {
    fn default() -> Self {
        Self::new()
    }
}

/// Variante moderna (>= 2.1): `RDB$RELATION_TYPE` distingue tabela/view/outros.
///
/// O filtro de objetos de sistema usa `(RDB$SYSTEM_FLAG = 0 OR ... IS NULL)`
/// e não `IS NOT DISTINCT FROM 0`, que só existe no Firebird 5.0+ e quebraria
/// 2.0–4.0 — usuário tem `RDB$SYSTEM_FLAG` `NULL`, não `0`.
const SQL_RELATIONS_MODERNA: &str = "SELECT RDB$RELATION_NAME, RDB$RELATION_TYPE \
     FROM RDB$RELATIONS \
     WHERE (RDB$SYSTEM_FLAG = 0 OR RDB$SYSTEM_FLAG IS NULL) \
     ORDER BY RDB$RELATION_NAME";

/// Variante fallback (2.0): `RDB$VIEW_TYPE` — `NULL` = tabela, não-`NULL` = view.
/// Mesmo filtro de sistema seguro da variante moderna (ver [`SQL_RELATIONS_MODERNA`]).
const SQL_RELATIONS_FALLBACK: &str = "SELECT RDB$RELATION_NAME, RDB$VIEW_TYPE \
     FROM RDB$RELATIONS \
     WHERE (RDB$SYSTEM_FLAG = 0 OR RDB$SYSTEM_FLAG IS NULL) \
     ORDER BY RDB$RELATION_NAME";

/// Colunas de [`SQL_RELATION_FIELDS`], na ordem do `SELECT`.
const SQL_RELATION_FIELDS: &str =
    "SELECT RF.RDB$FIELD_NAME, F.RDB$FIELD_TYPE, F.RDB$FIELD_SUB_TYPE, \
     RF.RDB$NULL_FLAG, F.RDB$NULL_FLAG, \
     RF.RDB$DEFAULT_SOURCE, F.RDB$DEFAULT_SOURCE, \
     F.RDB$DOMAIN_NAME, RF.RDB$FIELD_POSITION \
     FROM RDB$RELATION_FIELDS RF \
     JOIN RDB$FIELDS F ON F.RDB$FIELD_NAME = RF.RDB$FIELD_SOURCE \
     WHERE RF.RDB$RELATION_NAME = '{}' \
     ORDER BY RF.RDB$FIELD_POSITION";

/// Colunas esperadas em cada linha de [`SQL_RELATION_FIELDS`].
const COLUNAS_RELATION_FIELDS: usize = 9;

/// Classifica pela variante moderna: 0 = TABLE, 1 = VIEW, resto (e `NULL`) =
/// [`RelationKind::Other`] — ausência nunca vira afirmação inventada.
fn classificar_por_relation_type(tipo: Option<&str>) -> RelationKind {
    match tipo {
        Some("0") => RelationKind::Table,
        Some("1") => RelationKind::View,
        _ => RelationKind::Other,
    }
}

/// Classifica pela variante 2.0: `RDB$VIEW_TYPE` `NULL` = tabela base;
/// não-`NULL` (0 = CONSTRAINED, 1 = RELATION) = view.
fn classificar_por_view_type(tipo: Option<&str>) -> RelationKind {
    match tipo {
        None => RelationKind::Table,
        Some(_) => RelationKind::View,
    }
}

/// Remove o preenchimento de `CHAR` à direita (não trim total: espaços à
/// esquerda são significativos em nomes case-sensitive entre aspas).
fn limpar_char(texto: &str) -> String {
    texto.trim_end().to_string()
}

/// Escapa um valor para literal string SQL (Firebird): só as aspas simples
/// são duplicadas (`'` → `''`), o que também fecha tentativas de injeção.
/// Aspas duplas são caracteres comuns dentro de literal entre aspas simples
/// e passam intactas — quem delimita o valor é sempre `'`, nunca `"`.
fn escapar_literal(texto: &str) -> String {
    texto.replace('\'', "''")
}

/// Extrai o nome (coluna 0) de uma linha de catálogo, exigindo linha e nome
/// presentes; NULL onde só pode haver identificador é dado malformado.
fn nome_de_catalogo<'a>(linha: &'a [Option<String>], tabela: &str) -> Result<&'a str, DomainError> {
    linha
        .first()
        .and_then(Option::as_deref)
        .ok_or_else(|| catalogo_erro(format!("nome ausente em linha de {tabela}: {linha:?}")))
}

/// Monta um [`DomainError::Catalog`] com o contexto da consulta problemática.
fn catalogo_erro(message: String) -> DomainError {
    DomainError::Catalog { message }
}

/// Converte uma coluna numérica textual do catálogo (vem como `String` pelo
/// adapter genérico) em `i32`, com erro claro em vez de default inventado.
fn inteiro_de_catalogo(valor: Option<&str>, campo: &str) -> Result<i32, DomainError> {
    valor
        .map(str::trim)
        .filter(|texto| !texto.is_empty())
        .ok_or_else(|| catalogo_erro(format!("coluna {campo} ausente")))?
        .parse::<i32>()
        .map_err(|e| catalogo_erro(format!("coluna {campo} inválida: {e}")))
}

/// Converte uma linha de [`SQL_RELATION_FIELDS`] em [`FieldInfo`].
///
/// `NULL` (o [`Option`] vindo do adapter) é ausência: `nullable` só é `false`
/// se a coluna ou o domínio marcam NOT NULL; `domain`/`default_value` ficam
/// [`None`] quando o catálogo não os traz — nada é inventado (Spec 01, §4).
fn campo_de_linha(linha: &[Option<String>]) -> Result<FieldInfo, DomainError> {
    if linha.len() < COLUNAS_RELATION_FIELDS {
        return Err(catalogo_erro(format!(
            "linha malformada em RDB$RELATION_FIELDS: {linha:?}"
        )));
    }
    let valor = |indice: usize| linha[indice].as_deref();

    let nome = valor(0)
        .ok_or_else(|| catalogo_erro(format!("RDB$FIELD_NAME ausente em linha: {linha:?}")))?;
    let field_type = inteiro_de_catalogo(valor(1), "RDB$FIELD_TYPE")?;
    let sub_type = match valor(2) {
        None => None,
        Some(texto) => Some(inteiro_de_catalogo(Some(texto), "RDB$FIELD_SUB_TYPE")?),
    };
    let nullavel = valor(3).is_none() && valor(4).is_none();
    let default_source = valor(5).or_else(|| valor(6)).map(limpar_char);
    let position = inteiro_de_catalogo(valor(8), "RDB$FIELD_POSITION")?;

    Ok(FieldInfo {
        name: limpar_char(nome),
        type_name: type_name_de_catalogo(field_type, sub_type)?,
        domain: valor(7).map(limpar_char),
        nullable: nullavel,
        default_value: default_source,
        position,
    })
}

/// Deriva o nome legível do tipo a partir de `RDB$FIELD_TYPE`/`RDB$FIELD_SUB_TYPE`.
///
/// Mapa mínimo do spike (Firebird 2.0–5.0): SMALLINT (7), INTEGER (8),
/// FLOAT (10), DATE (12), TIME (13), CHAR (14), BIGINT (16), BOOLEAN (23),
/// DOUBLE PRECISION (27), TIMESTAMP (35), VARCHAR (37) e BLOB (261, com
/// `SUB_TYPE 1` = texto). NUMERIC/DECIMAL vêm de SMALLINT/INTEGER/BIGINT com
/// `RDB$FIELD_SUB_TYPE` 1/2. Observações para a Fase 1, sem gate por versão
/// aqui: BOOLEAN só existe a partir do 3.0, e tipos novos (INT128, DECFLOAT,
/// `TIME/TIMESTAMP WITH TIME ZONE` etc.) estão fora do mapa — tipo fora do
/// mapa gera erro, nunca nome inventado.
fn type_name_de_catalogo(field_type: i32, sub_type: Option<i32>) -> Result<String, DomainError> {
    let nome = match field_type {
        7 => tipo_numerico("SMALLINT", sub_type),
        8 => tipo_numerico("INTEGER", sub_type),
        16 => tipo_numerico("BIGINT", sub_type),
        10 => "FLOAT".to_string(),
        12 => "DATE".to_string(),
        13 => "TIME".to_string(),
        14 => "CHAR".to_string(),
        23 => "BOOLEAN".to_string(),
        27 => "DOUBLE PRECISION".to_string(),
        35 => "TIMESTAMP".to_string(),
        37 => "VARCHAR".to_string(),
        261 => match sub_type {
            Some(1) => "BLOB SUB_TYPE TEXT".to_string(),
            _ => "BLOB".to_string(),
        },
        fora_do_mapa => {
            return Err(catalogo_erro(format!(
                "RDB$FIELD_TYPE {fora_do_mapa} fora do mapa do spike; \
                 nada foi inventado (refinar na Fase 1)"
            )))
        }
    };
    Ok(nome)
}

/// Aplica NUMERIC/DECIMAL sobre a base inteira conforme `RDB$FIELD_SUB_TYPE`
/// (0 ou `NULL` = base simples; 1 = NUMERIC; 2 = DECIMAL).
fn tipo_numerico(base: &str, sub_type: Option<i32>) -> String {
    match sub_type {
        Some(1) => "NUMERIC".to_string(),
        Some(2) => "DECIMAL".to_string(),
        _ => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeAdapter;

    /// Versão sintética sem patch, para os testes.
    fn versao(major: u8, minor: u8) -> ServerVersion {
        ServerVersion {
            major,
            minor,
            patch: None,
        }
    }

    #[test]
    fn relations_v25_usa_relation_type_e_limpa_padding() {
        let linhas = vec![
            vec![
                Some("CLIENTES            ".to_string()),
                Some("0".to_string()),
            ],
            vec![
                Some("V_CLIENTES          ".to_string()),
                Some("1".to_string()),
            ],
            vec![
                Some("PKG_TESTE           ".to_string()),
                Some("7".to_string()),
            ],
        ];
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_TYPE", linhas)]);
        let repo = MetadataRepository::new();

        let relacoes = repo
            .relations(&mut fake, &versao(2, 5))
            .expect("2.5 deve listar relações");

        assert_eq!(
            relacoes,
            vec![
                RelationSummary {
                    name: "CLIENTES".to_string(),
                    kind: RelationKind::Table,
                },
                RelationSummary {
                    name: "V_CLIENTES".to_string(),
                    kind: RelationKind::View,
                },
                RelationSummary {
                    name: "PKG_TESTE".to_string(),
                    kind: RelationKind::Other,
                },
            ],
            "nomes com preenchimento CHAR devem sair limpos e tipados pelo RDB$RELATION_TYPE"
        );

        let sqls = fake.executed_sqls();
        assert_eq!(sqls.len(), 1);
        let sql = &sqls[0];
        assert!(
            sql.contains("RDB$RELATION_TYPE"),
            "variante moderna deve usar RDB$RELATION_TYPE: {sql}"
        );
        assert!(
            !sql.contains("RDB$VIEW_TYPE"),
            "variante moderna não deve citar RDB$VIEW_TYPE: {sql}"
        );
        assert!(
            sql.contains("RDB$SYSTEM_FLAG"),
            "deve excluir objetos de sistema: {sql}"
        );
        assert!(
            sql.contains("ORDER BY RDB$RELATION_NAME"),
            "deve ordenar por nome: {sql}"
        );
    }

    #[test]
    fn relations_v20_usa_fallback_view_type() {
        let linhas = vec![
            vec![Some("CLIENTES".to_string()), None],
            vec![Some("V_CLIENTES".to_string()), Some("1".to_string())],
            vec![Some("PKG_TESTE".to_string()), Some("0".to_string())],
        ];
        let mut fake = FakeAdapter::new(vec![("RDB$VIEW_TYPE", linhas)]);
        let repo = MetadataRepository::new();

        let relacoes = repo
            .relations(&mut fake, &versao(2, 0))
            .expect("2.0 deve listar relações pela variante fallback");

        assert_eq!(
            relacoes[0].kind,
            RelationKind::Table,
            "VIEW_TYPE NULL = tabela"
        );
        assert_eq!(relacoes[1].kind, RelationKind::View, "VIEW_TYPE 1 = view");
        assert_eq!(
            relacoes[2].kind,
            RelationKind::View,
            "VIEW_TYPE 0 (CONSTRAINED) também é view"
        );

        let sqls = fake.executed_sqls();
        let sql = &sqls[0];
        assert!(
            sql.contains("RDB$VIEW_TYPE"),
            "fallback do 2.0 deve usar RDB$VIEW_TYPE: {sql}"
        );
        assert!(
            !sql.contains("RDB$RELATION_TYPE"),
            "Firebird 2.0 não tem RDB$RELATION_TYPE: {sql}"
        );
    }

    #[test]
    fn relations_moderna_com_relation_type_null_e_other() {
        let linhas = vec![vec![Some("ESTRANHA".to_string()), None]];
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_TYPE", linhas)]);
        let repo = MetadataRepository::new();

        let relacoes = repo
            .relations(&mut fake, &versao(3, 0))
            .expect("deve listar");

        assert_eq!(
            relacoes[0].kind,
            RelationKind::Other,
            "NULL não pode virar Table/View por invenção"
        );
    }

    #[test]
    fn relations_propaga_erro_do_adapter() {
        let mut fake = FakeAdapter::new(vec![]);
        let repo = MetadataRepository::new();

        let erro = repo
            .relations(&mut fake, &versao(2, 5))
            .expect_err("SQL sem regra deve propagar erro");

        assert!(
            matches!(erro, DomainError::Catalog { .. }),
            "erro do adapter deve chegar intacto: {erro:?}"
        );
    }

    #[test]
    fn relation_fields_mapeia_colunas_e_limpa_padding() {
        let linha = vec![
            Some("ID                      ".to_string()), // RDB$FIELD_NAME (CHAR)
            Some("8".to_string()),                        // RDB$FIELD_TYPE = INTEGER
            None,                                         // RDB$FIELD_SUB_TYPE
            Some("1".to_string()),                        // RF.RDB$NULL_FLAG = NOT NULL
            None,                                         // F.RDB$NULL_FLAG
            None,                                         // RF.RDB$DEFAULT_SOURCE
            None,                                         // F.RDB$DEFAULT_SOURCE
            None,                                         // F.RDB$DOMAIN_NAME
            Some("0".to_string()),                        // RF.RDB$FIELD_POSITION
        ];
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_FIELDS", vec![linha])]);
        let repo = MetadataRepository::new();

        let campos = repo
            .relation_fields(&mut fake, "CLIENTES")
            .expect("deve listar colunas");

        assert_eq!(
            campos,
            vec![FieldInfo {
                name: "ID".to_string(),
                type_name: "INTEGER".to_string(),
                domain: None,
                nullable: false,
                default_value: None,
                position: 0,
            }]
        );

        let sqls = fake.executed_sqls();
        let sql = &sqls[0];
        assert!(
            sql.contains("RDB$RELATION_FIELDS"),
            "deve consultar as colunas da relação: {sql}"
        );
        assert!(
            sql.contains("= 'CLIENTES'"),
            "deve filtrar pelo nome da relação: {sql}"
        );
        assert!(
            sql.contains("ORDER BY RF.RDB$FIELD_POSITION"),
            "deve ordenar por posição: {sql}"
        );
    }

    #[test]
    fn relation_fields_extrai_domain_default_e_numeric() {
        let linha = vec![
            Some("VALOR                   ".to_string()),
            Some("8".to_string()), // INTEGER como base
            Some("1".to_string()), // RDB$FIELD_SUB_TYPE = NUMERIC
            None,                  // RF.RDB$NULL_FLAG → nullable
            None,                  // F.RDB$NULL_FLAG
            Some("DEFAULT 0                           ".to_string()), // RF.RDB$DEFAULT_SOURCE
            None,                  // F.RDB$DEFAULT_SOURCE
            Some("DINHEIRO               ".to_string()), // F.RDB$DOMAIN_NAME
            Some("3".to_string()), // RF.RDB$FIELD_POSITION
        ];
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_FIELDS", vec![linha])]);
        let repo = MetadataRepository::new();

        let campos = repo
            .relation_fields(&mut fake, "CONTAS")
            .expect("deve listar colunas");

        assert_eq!(
            campos,
            vec![FieldInfo {
                name: "VALOR".to_string(),
                type_name: "NUMERIC".to_string(),
                domain: Some("DINHEIRO".to_string()),
                nullable: true,
                default_value: Some("DEFAULT 0".to_string()),
                position: 3,
            }]
        );
    }

    #[test]
    fn relation_fields_respeita_not_null_e_default_do_dominio() {
        let linha = vec![
            Some("TIPO".to_string()),
            Some("14".to_string()),          // CHAR
            None,                            // RDB$FIELD_SUB_TYPE
            None,                            // RF.RDB$NULL_FLAG (sem override na coluna)
            Some("1".to_string()),           // F.RDB$NULL_FLAG → domínio NOT NULL
            None,                            // RF.RDB$DEFAULT_SOURCE
            Some("DEFAULT 'A'".to_string()), // F.RDB$DEFAULT_SOURCE (override do domínio)
            Some("T_TIPO".to_string()),
            Some("1".to_string()),
        ];
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_FIELDS", vec![linha])]);
        let repo = MetadataRepository::new();

        let campos = repo
            .relation_fields(&mut fake, "MOVIMENTOS")
            .expect("deve listar colunas");

        assert_eq!(
            campos,
            vec![FieldInfo {
                name: "TIPO".to_string(),
                type_name: "CHAR".to_string(),
                domain: Some("T_TIPO".to_string()),
                nullable: false,
                default_value: Some("DEFAULT 'A'".to_string()),
                position: 1,
            }],
            "NOT NULL e default do domínio contam quando a coluna não sobrescreve"
        );
    }

    #[test]
    fn relation_fields_deriva_type_name_de_field_type() {
        // 23 = BOOLEAN (só existe a partir do 3.0; spike não gateia o mapa),
        // 261+SUB_TYPE 1 = BLOB SUB_TYPE TEXT, 37 = VARCHAR.
        let linhas = vec![
            vec![
                Some("ATIVO".to_string()),
                Some("23".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                Some("0".to_string()),
            ],
            vec![
                Some("DOC".to_string()),
                Some("261".to_string()),
                Some("1".to_string()),
                None,
                None,
                None,
                None,
                None,
                Some("1".to_string()),
            ],
            vec![
                Some("NOME".to_string()),
                Some("37".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                Some("2".to_string()),
            ],
        ];
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_FIELDS", linhas)]);
        let repo = MetadataRepository::new();

        let campos = repo
            .relation_fields(&mut fake, "ANEXOS")
            .expect("deve listar colunas");

        assert_eq!(campos[0].type_name, "BOOLEAN");
        assert_eq!(campos[1].type_name, "BLOB SUB_TYPE TEXT");
        assert_eq!(campos[2].type_name, "VARCHAR");
    }

    #[test]
    fn relation_fields_erro_para_tipo_fora_do_mapa() {
        let linha = vec![
            Some("X".to_string()),
            Some("999".to_string()), // tipo inexistente no mapa do spike
            None,
            None,
            None,
            None,
            None,
            None,
            Some("0".to_string()),
        ];
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_FIELDS", vec![linha])]);
        let repo = MetadataRepository::new();

        let erro = repo
            .relation_fields(&mut fake, "T")
            .expect_err("tipo fora do mapa deve falhar, nunca inventar nome");

        assert!(
            matches!(erro, DomainError::Catalog { .. }),
            "deve virar DomainError::Catalog: {erro:?}"
        );
    }

    #[test]
    fn relation_fields_erro_para_linha_malformada() {
        let linha = vec![
            Some("ID".to_string()),
            Some("8".to_string()),
            Some("0".to_string()),
        ];
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_FIELDS", vec![linha])]);
        let repo = MetadataRepository::new();

        let erro = repo
            .relation_fields(&mut fake, "T")
            .expect_err("linha com colunas a menos deve falhar");

        assert!(
            matches!(erro, DomainError::Catalog { .. }),
            "deve virar DomainError::Catalog: {erro:?}"
        );
    }

    #[test]
    fn relation_fields_escapa_aspas_simples_no_nome() {
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_FIELDS", vec![])]);
        let repo = MetadataRepository::new();

        repo.relation_fields(&mut fake, "O'BRIEN")
            .expect("listagem vazia é Ok vazio");

        let sqls = fake.executed_sqls();
        let sql = &sqls[0];
        assert!(
            sql.contains("= 'O''BRIEN'"),
            "aspas simples do literal devem ser duplicadas (segurança contra injeção): {sql}"
        );
    }

    #[test]
    fn relation_fields_preserva_aspas_duplas_no_nome() {
        let mut fake = FakeAdapter::new(vec![("RDB$RELATION_FIELDS", vec![])]);
        let repo = MetadataRepository::new();

        repo.relation_fields(&mut fake, "WEIRD \"NOME\"")
            .expect("listagem vazia é Ok vazio");

        let sqls = fake.executed_sqls();
        let sql = &sqls[0];
        assert!(
            sql.contains("= 'WEIRD \"NOME\"'"),
            "aspas duplas são comuns dentro de literal string e passam intactas: {sql}"
        );
    }
}
