//! Modelos puros de catálogo do Firebird Studio Linux (Passo 1, Fase 2).
//!
//! Ao contrário de [`crate::metadata::FieldInfo`], estes modelos descrevem
//! entidades de mais alto nível do explorador (banco, índices, constraints,
//! domínios, generators, procedures, parâmetros, triggers e categorias do
//! explorador). Campos ausentes no servidor são sempre [`Option`] — nada é
//! inventado (Spec 01, §4).

/// Informações básicas do banco (`RDB$DATABASE`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InfoBanco {
    /// Caminho/nome do banco no servidor, quando reportado pelo adapter.
    pub database_path: Option<String>,
    /// Tamanho da página em bytes (ex.: 4096, 8192, 16384).
    pub page_size: Option<i32>,
    /// Versão do ODS (major).
    pub ods_major: Option<i32>,
    /// Revisão do ODS (minor).
    pub ods_minor: Option<i32>,
    /// Charset default do banco, quando exposto.
    pub charset: Option<String>,
}

/// Tipo de constraint do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintTipo {
    /// Chave primária.
    PrimaryKey,
    /// Chave única (unique).
    Unique,
    /// Chave estrangeira (foreign key).
    ForeignKey,
    /// Restrição de checagem.
    Check,
    /// NOT NULL (quando materializado como constraint).
    NotNull,
}

impl ConstraintTipo {
    /// Código curto usado em relatórios/UI (`PK`, `UK`, `FK`, ...).
    pub fn as_str(&self) -> &'static str {
        match self {
            ConstraintTipo::PrimaryKey => "PK",
            ConstraintTipo::Unique => "UK",
            ConstraintTipo::ForeignKey => "FK",
            ConstraintTipo::Check => "CHECK",
            ConstraintTipo::NotNull => "NOT NULL",
        }
    }
}

/// Metadados de uma constraint de relação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintInfo {
    pub name: String,
    pub relation_name: String,
    pub constraint_type: ConstraintTipo,
    /// Relação referenciada, para chaves estrangeiras.
    pub referenced_relation: Option<String>,
    /// Índice associado à constraint, quando existir.
    pub index_name: Option<String>,
    /// Marca objetos de sistema do catálogo Firebird.
    pub is_system: bool,
}

/// Metadados de um índice do catálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndiceInfo {
    pub name: String,
    pub relation_name: String,
    pub is_unique: bool,
    /// Colunas (segmentos) do índice, quando lidas do catálogo.
    pub columns: Vec<String>,
    /// Marca objetos de sistema do catálogo Firebird.
    pub is_system: bool,
}

/// Metadados de um domínio Firebird (`RDB$FIELDS` com nome de domínio).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DominioInfo {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    /// Marca objetos de sistema do catálogo Firebird.
    pub is_system: bool,
}

/// Metadados de um generator/sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratorInfo {
    pub name: String,
    pub value: Option<i64>,
    /// Marca objetos de sistema do catálogo Firebird.
    pub is_system: bool,
}

/// Metadados de um procedimento armazenado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProceduraInfo {
    pub name: String,
    pub procedure_type: Option<i32>,
    pub entrypoint: Option<String>,
    /// Marca objetos de sistema do catálogo Firebird.
    pub is_system: bool,
}

/// Metadados de um parâmetro de procedimento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParametroInfo {
    pub procedure_name: String,
    pub name: String,
    pub position: i32,
    /// `IN`, `OUT` ou `INOUT`.
    pub mode: String,
    pub type_name: String,
}

/// Metadados de um trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerInfo {
    pub name: String,
    pub relation_name: Option<String>,
    pub trigger_type: Option<i32>,
    pub is_active: bool,
    pub sequence: Option<i32>,
    /// Marca objetos de sistema do catálogo Firebird.
    pub is_system: bool,
}

/// As sete categorias navegáveis do explorador de banco (Spec 04 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CategoriaCatalogo {
    Tabelas,
    Views,
    Indices,
    Dominios,
    Generators,
    Procedures,
    Triggers,
}

impl CategoriaCatalogo {
    /// Identificador textual canônico (mesmo da fronteira `CatalogCategoryDto`).
    pub fn as_str(&self) -> &'static str {
        match self {
            CategoriaCatalogo::Tabelas => "tabelas",
            CategoriaCatalogo::Views => "views",
            CategoriaCatalogo::Indices => "indices",
            CategoriaCatalogo::Dominios => "dominios",
            CategoriaCatalogo::Generators => "generators",
            CategoriaCatalogo::Procedures => "procedures",
            CategoriaCatalogo::Triggers => "triggers",
        }
    }

    /// Parsing tolerante (minúsculo, exatamente os valores da fronteira).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "tabelas" | "tables" => Some(CategoriaCatalogo::Tabelas),
            "views" => Some(CategoriaCatalogo::Views),
            "indices" | "índices" | "indexes" => Some(CategoriaCatalogo::Indices),
            "dominios" | "domínios" | "domains" => Some(CategoriaCatalogo::Dominios),
            "generators" => Some(CategoriaCatalogo::Generators),
            "procedures" => Some(CategoriaCatalogo::Procedures),
            "triggers" => Some(CategoriaCatalogo::Triggers),
            _ => None,
        }
    }

    /// Lista estável das sete categorias, na ordem de exibição do explorador.
    pub fn todas() -> [CategoriaCatalogo; 7] {
        [
            CategoriaCatalogo::Tabelas,
            CategoriaCatalogo::Views,
            CategoriaCatalogo::Indices,
            CategoriaCatalogo::Dominios,
            CategoriaCatalogo::Generators,
            CategoriaCatalogo::Procedures,
            CategoriaCatalogo::Triggers,
        ]
    }
}
