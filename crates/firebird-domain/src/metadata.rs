//! Modelos puros de metadados do catálogo Firebird.
//!
//! Campos ausentes no servidor são sempre [`None`]; nada é inventado.

/// Natureza de uma relação do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    /// Tabela física.
    Table,
    /// View.
    View,
    /// Outros tipos (sinônimos, prováveis processos externos etc.).
    Other,
}

/// Resumo de uma relação listada no explorador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationSummary {
    pub name: String,
    pub kind: RelationKind,
}

/// Metadados de uma coluna de tabela ou view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    pub name: String,
    /// Nome do tipo declarado (ex.: `INTEGER`, `VARCHAR`).
    pub type_name: String,
    /// Domínio da coluna (`RDB$FIELD_SOURCE`): explícito (`CREATE TABLE t
    /// (x DOMINIO)`) ou interno (`RDB$NNN`) gerado para coluna com tipo
    /// inline — `RDB$FIELDS.RDB$DOMAIN_NAME` não existe em 2.5/3.0
    /// (evidência da Task 4); ausente = [`None`].
    pub domain: Option<String>,
    pub nullable: bool,
    /// Expressão default literal, quando existir; ausente = [`None`].
    pub default_value: Option<String>,
    /// Posição da coluna na relação (base 0, como em `RDB$FIELD_POSITION`).
    pub position: i32,
}

#[cfg(test)]
mod tests {
    use super::{FieldInfo, RelationKind, RelationSummary};

    #[test]
    fn field_info_preserva_ausencia_de_metadados_com_none() {
        let campo = FieldInfo {
            name: "ID".to_string(),
            type_name: "INTEGER".to_string(),
            domain: None,
            nullable: false,
            default_value: None,
            position: 0,
        };
        assert_eq!(campo.domain, None);
        assert_eq!(campo.default_value, None);
    }

    #[test]
    fn relation_summary_identifica_tabela_e_view() {
        let tabela = RelationSummary {
            name: "CLIENTES".to_string(),
            kind: RelationKind::Table,
        };
        let view = RelationSummary {
            name: "V_CLIENTES".to_string(),
            kind: RelationKind::View,
        };
        assert_eq!(tabela.kind, RelationKind::Table);
        assert_eq!(view.kind, RelationKind::View);
    }
}
