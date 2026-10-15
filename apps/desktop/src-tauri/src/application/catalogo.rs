//! Serviço de catálogo do banco (DB Explorer, Passo 1 da Fase 2 — Spec 04).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::application::connection_manager::ConnectionManager;
use crate::commands::error::CommandError;
use firebird_adapter::MetadataRepository;
use firebird_domain::{catalog::CategoriaCatalogo, RelationKind, ServerVersion};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogNodeKindDto {
    Root,
    Category,
    Object,
    Field,
    Index,
    Constraint,
    Parameter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogCategoryDto {
    Tabelas,
    Views,
    Indices,
    Dominios,
    Generators,
    Procedures,
    Triggers,
}

impl From<CatalogCategoryDto> for CategoriaCatalogo {
    fn from(c: CatalogCategoryDto) -> Self {
        match c {
            CatalogCategoryDto::Tabelas => CategoriaCatalogo::Tabelas,
            CatalogCategoryDto::Views => CategoriaCatalogo::Views,
            CatalogCategoryDto::Indices => CategoriaCatalogo::Indices,
            CatalogCategoryDto::Dominios => CategoriaCatalogo::Dominios,
            CatalogCategoryDto::Generators => CategoriaCatalogo::Generators,
            CatalogCategoryDto::Procedures => CategoriaCatalogo::Procedures,
            CatalogCategoryDto::Triggers => CategoriaCatalogo::Triggers,
        }
    }
}

impl CatalogCategoryDto {
    pub fn rotulo(self) -> &'static str {
        match self {
            CatalogCategoryDto::Tabelas => "Tabelas",
            CatalogCategoryDto::Views => "Views",
            CatalogCategoryDto::Indices => "Índices",
            CatalogCategoryDto::Dominios => "Domínios",
            CatalogCategoryDto::Generators => "Generators",
            CatalogCategoryDto::Procedures => "Procedures",
            CatalogCategoryDto::Triggers => "Triggers",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CatalogNodeRequestDto {
    Raiz,
    Relacao { relacao: String },
    Categoria { categoria: CatalogCategoryDto },
    Campos { relacao: String },
    Constraints { relacao: String },
    Indices { relacao: String },
    Parametros { procedimento: String },
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogNodeDto {
    pub node_id: String,
    pub label: String,
    pub kind: CatalogNodeKindDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<CatalogCategoryDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub is_system: bool,
    pub has_children: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<CatalogNodeRequestDto>,
    pub children: Vec<CatalogNodeDto>,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct CacheKey {
    request: CatalogNodeRequestDto,
    include_system: bool,
}

pub struct CatalogoService {
    repo: MetadataRepository,
    cache: Mutex<HashMap<String, HashMap<CacheKey, Vec<CatalogNodeDto>>>>,
}

impl Default for CatalogoService {
    fn default() -> Self {
        Self {
            repo: MetadataRepository::new(),
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl CatalogoService {
    pub fn novo() -> Self {
        Self::default()
    }

    pub fn get_node(
        &self,
        manager: &ConnectionManager,
        profile_id: String,
        node: CatalogNodeRequestDto,
        filter: Option<String>,
        include_system: bool,
        force_refresh: bool,
    ) -> Result<CatalogNodeDto, CommandError> {
        let filtro_normal = filter
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let cache_usa = filtro_normal.is_none();
        let cache_key = CacheKey {
            request: node.clone(),
            include_system,
        };

        if cache_usa && !force_refresh {
            if let Some(cache) = self
                .cache
                .lock()
                .ok()
                .and_then(|g| g.get(&profile_id).cloned())
            {
                if let Some(children) = cache.get(&cache_key).cloned() {
                    return Ok(node_dto(profile_id, node, children));
                }
            }
        }

        let children = manager
            .executar_no_adaptador_conectado(&profile_id, |adapter| {
                let versao = adapter.server_version().map_err(|e| {
                    crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
                })?;
                self.build_children(
                    adapter,
                    &profile_id,
                    &node,
                    &versao,
                    filtro_normal.as_deref(),
                    include_system,
                )
                .map_err(|e| {
                    crate::application::connection_manager::ErroGerente::Conexao(e.to_string())
                })
            })
            .map_err(|e| CommandError::conexao(e.to_string()))?;

        if cache_usa {
            let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            let profile_cache = guard.entry(profile_id.clone()).or_default();
            profile_cache.insert(cache_key, children.clone());
        }

        Ok(node_dto(profile_id, node, children))
    }

    pub fn invalidar(&self, profile_id: &str) {
        if let Ok(mut guard) = self.cache.lock() {
            guard.remove(profile_id);
        }
    }

    fn build_children(
        &self,
        adapter_box: &mut crate::application::connection_manager::AdaptadorAberto,
        _profile_id: &str,
        node: &CatalogNodeRequestDto,
        version: &ServerVersion,
        filtro: Option<&str>,
        include_system: bool,
    ) -> Result<Vec<CatalogNodeDto>, CommandError> {
        let adapter: &mut dyn firebird_adapter::FirebirdAdapter = &mut **adapter_box;
        let repo = &self.repo;

        match node {
            CatalogNodeRequestDto::Relacao { relacao } => Ok(vec![
                CatalogNodeDto {
                    node_id: format!("fields:{}", relacao),
                    label: "Campos".to_string(),
                    kind: CatalogNodeKindDto::Category,
                    category: None,
                    detail: None,
                    is_system: false,
                    has_children: true,
                    request: Some(CatalogNodeRequestDto::Campos {
                        relacao: relacao.clone(),
                    }),
                    children: Vec::new(),
                },
                CatalogNodeDto {
                    node_id: format!("idxcat:{}", relacao),
                    label: "Índices".to_string(),
                    kind: CatalogNodeKindDto::Category,
                    category: None,
                    detail: None,
                    is_system: false,
                    has_children: true,
                    request: Some(CatalogNodeRequestDto::Indices {
                        relacao: relacao.clone(),
                    }),
                    children: Vec::new(),
                },
                CatalogNodeDto {
                    node_id: format!("constcat:{}", relacao),
                    label: "Chaves".to_string(),
                    kind: CatalogNodeKindDto::Category,
                    category: None,
                    detail: None,
                    is_system: false,
                    has_children: true,
                    request: Some(CatalogNodeRequestDto::Constraints {
                        relacao: relacao.clone(),
                    }),
                    children: Vec::new(),
                },
            ]),
            CatalogNodeRequestDto::Raiz => {
                let _info = repo.informacoes_do_banco(adapter, version).ok();
                let children: Vec<CatalogNodeDto> = CategoriaCatalogo::todas()
                    .into_iter()
                    .filter(|_| true)
                    .map(|c| CatalogNodeDto {
                        node_id: format!("cat:{}", c.as_str()),
                        label: rotulo_categoria(c).to_string(),
                        kind: CatalogNodeKindDto::Category,
                        category: Some(c.into_dto()),
                        detail: None,
                        is_system: false,
                        has_children: true,
                        request: Some(CatalogNodeRequestDto::Categoria {
                            categoria: c.into_dto(),
                        }),
                        children: Vec::new(),
                    })
                    .collect();
                Ok(children)
            }
            CatalogNodeRequestDto::Categoria { categoria } => match categoria {
                CatalogCategoryDto::Tabelas | CatalogCategoryDto::Views => {
                    let is_views = matches!(categoria, CatalogCategoryDto::Views);
                    let rels = repo
                        .relations_com_filtro(adapter, version, None, include_system)
                        .map_err(adaptador_err)?;
                    let rels: Vec<CatalogNodeDto> = rels
                        .into_iter()
                        .filter(|r| {
                            let kind_ok = if is_views {
                                matches!(r.kind, RelationKind::View)
                            } else {
                                matches!(r.kind, RelationKind::Table)
                            };
                            kind_ok && passa_filtro(&r.name, filtro)
                        })
                        .map(|r| {
                            let rel = r.name;
                            CatalogNodeDto {
                                node_id: format!("rel:{}", rel),
                                label: rel.clone(),
                                kind: CatalogNodeKindDto::Object,
                                category: None,
                                detail: Some(if is_views {
                                    "view".to_string()
                                } else {
                                    "tabela".to_string()
                                }),
                                is_system: rel.starts_with("RDB$"),
                                has_children: true,
                                request: Some(CatalogNodeRequestDto::Campos { relacao: rel }),
                                children: Vec::new(),
                            }
                        })
                        .collect();
                    // child requests handled lazily by UI using the node's own request; but the table node
                    // should also expose fields/indices/constraints as child nodes on expansion — we represent
                    // the table node as leaf-ish (children empty now; expands via "Campos"). To help UX, allow
                    // deep navigation via the Campos/Indices/Constraints sub-nodes only when expanded; instead,
                    // attach them as children so clicking expands naturally.
                    Ok(rels)
                }
                CatalogCategoryDto::Indices => {
                    let indices = repo.indices(adapter, version).map_err(adaptador_err)?;
                    let out: Vec<CatalogNodeDto> = indices
                        .into_iter()
                        .filter(|i| {
                            (include_system || !i.is_system) && passa_filtro(&i.name, filtro)
                        })
                        .map(|i| CatalogNodeDto {
                            node_id: format!("idx:{}", i.name),
                            label: i.name,
                            kind: CatalogNodeKindDto::Index,
                            category: None,
                            detail: {
                                let mut p = Vec::new();
                                if i.is_unique {
                                    p.push("UNIQUE");
                                }
                                if i.columns.is_empty() {
                                } else { /* skip */
                                }
                                if p.is_empty() {
                                    None
                                } else {
                                    Some(p.join(" · "))
                                }
                            },
                            is_system: i.is_system,
                            has_children: false,
                            request: None,
                            children: Vec::new(),
                        })
                        .collect();
                    Ok(out)
                }
                CatalogCategoryDto::Dominios => {
                    let rows = repo.dominios(adapter, version).map_err(adaptador_err)?;
                    let out: Vec<CatalogNodeDto> = rows
                        .into_iter()
                        .filter(|d| {
                            (include_system || !d.is_system) && passa_filtro(&d.name, filtro)
                        })
                        .map(|d| CatalogNodeDto {
                            node_id: format!("dom:{}", d.name),
                            label: d.name,
                            kind: CatalogNodeKindDto::Object,
                            category: None,
                            detail: Some(d.type_name),
                            is_system: d.is_system,
                            has_children: false,
                            request: None,
                            children: Vec::new(),
                        })
                        .collect();
                    Ok(out)
                }
                CatalogCategoryDto::Generators => {
                    let rows = repo.generators(adapter, version).map_err(adaptador_err)?;
                    let out: Vec<CatalogNodeDto> = rows
                        .into_iter()
                        .filter(|g| {
                            (include_system || !g.is_system) && passa_filtro(&g.name, filtro)
                        })
                        .map(|g| CatalogNodeDto {
                            node_id: format!("gen:{}", g.name),
                            label: g.name,
                            detail: g.value.map(|v| format!("= {}", v)),
                            kind: CatalogNodeKindDto::Object,
                            category: None,
                            is_system: g.is_system,
                            has_children: false,
                            request: None,
                            children: Vec::new(),
                        })
                        .collect();
                    Ok(out)
                }
                CatalogCategoryDto::Procedures => {
                    let rows = repo.procedures(adapter, version).map_err(adaptador_err)?;
                    let out: Vec<CatalogNodeDto> = rows
                        .into_iter()
                        .filter(|p| {
                            (include_system || !p.is_system) && passa_filtro(&p.name, filtro)
                        })
                        .map(|p| CatalogNodeDto {
                            node_id: format!("proc:{}", p.name),
                            label: p.name.clone(),
                            kind: CatalogNodeKindDto::Object,
                            category: None,
                            detail: p.procedure_type.map(|t| format!("tipo {}", t)),
                            is_system: p.is_system,
                            has_children: true,
                            request: Some(CatalogNodeRequestDto::Parametros {
                                procedimento: p.name.clone(),
                            }),
                            children: Vec::new(),
                        })
                        .collect();
                    Ok(out)
                }
                CatalogCategoryDto::Triggers => {
                    let rows = repo.triggers(adapter, version).map_err(adaptador_err)?;
                    let out: Vec<CatalogNodeDto> = rows
                        .into_iter()
                        .filter(|t| {
                            (include_system || !t.is_system) && passa_filtro(&t.name, filtro)
                        })
                        .map(|t| CatalogNodeDto {
                            node_id: format!("trig:{}", t.name),
                            label: t.name,
                            kind: CatalogNodeKindDto::Object,
                            category: None,
                            detail: Some(if t.is_active {
                                "ativo".to_string()
                            } else {
                                "inativo".to_string()
                            }),
                            is_system: t.is_system,
                            has_children: false,
                            request: None,
                            children: Vec::new(),
                        })
                        .collect();
                    Ok(out)
                }
            },
            CatalogNodeRequestDto::Campos { relacao } => {
                let campos = repo
                    .relation_fields(adapter, relacao)
                    .map_err(adaptador_err)?;
                let out: Vec<CatalogNodeDto> = campos
                    .into_iter()
                    .filter(|c| passa_filtro(&c.name, filtro))
                    .map(|c| {
                        let mut parts = vec![c.type_name.clone()];
                        if !c.nullable {
                            parts.push("NOT NULL".to_string());
                        }
                        if let Some(d) = c.domain.as_ref() {
                            if !d.trim().is_empty() && !d.starts_with("RDB$") {
                                parts.push(format!("domínio {}", d.trim()));
                            }
                        }
                        if let Some(d) = c.default_value.as_ref() {
                            if !d.trim().is_empty() {
                                parts.push(format!("default {}", d.trim()));
                            }
                        }
                        CatalogNodeDto {
                            node_id: format!("field:{}:{}", relacao, c.name),
                            label: c.name,
                            kind: CatalogNodeKindDto::Field,
                            category: None,
                            detail: Some(parts.join(" · ")),
                            is_system: false,
                            has_children: false,
                            request: None,
                            children: Vec::new(),
                        }
                    })
                    .collect();
                // Anexar sub-nós dos campos na própria tabela: campos, indices, constraints.
                // (Campos é só isso — expandir a tabela no cliente abre estes via request
                // Campos/Indices/Constraints nos filhos da tabela; veja abaixo.)
                Ok(out)
            }
            CatalogNodeRequestDto::Indices { relacao } => {
                let indices = repo.indices(adapter, version).map_err(adaptador_err)?;
                let out: Vec<CatalogNodeDto> = indices
                    .into_iter()
                    .filter(|i| {
                        i.relation_name == *relacao
                            && (include_system || !i.is_system)
                            && passa_filtro(&i.name, filtro)
                    })
                    .map(|i| CatalogNodeDto {
                        node_id: format!("index:{}:{}", relacao, i.name),
                        label: i.name,
                        kind: CatalogNodeKindDto::Index,
                        category: None,
                        detail: if i.is_unique {
                            Some("UNIQUE".to_string())
                        } else {
                            None
                        },
                        is_system: i.is_system,
                        has_children: false,
                        request: None,
                        children: Vec::new(),
                    })
                    .collect();
                Ok(out)
            }
            CatalogNodeRequestDto::Constraints { relacao } => {
                let constraints = repo.constraints(adapter, version).map_err(adaptador_err)?;
                let out: Vec<CatalogNodeDto> = constraints
                    .into_iter()
                    .filter(|c| {
                        c.relation_name == *relacao
                            && (include_system || !c.is_system)
                            && passa_filtro(&c.name, filtro)
                    })
                    .map(|c| CatalogNodeDto {
                        node_id: format!("constraint:{}:{}", relacao, c.name),
                        label: c.name,
                        kind: CatalogNodeKindDto::Constraint,
                        category: None,
                        detail: Some(c.constraint_type.as_str().to_string()),
                        is_system: c.is_system,
                        has_children: false,
                        request: None,
                        children: Vec::new(),
                    })
                    .collect();
                Ok(out)
            }
            CatalogNodeRequestDto::Parametros { procedimento } => {
                let params = repo
                    .parametros_procedure(adapter, procedimento)
                    .map_err(adaptador_err)?;
                let out: Vec<CatalogNodeDto> = params
                    .into_iter()
                    .filter(|p| passa_filtro(&p.name, filtro))
                    .map(|p| CatalogNodeDto {
                        node_id: format!("param:{}:{}", procedimento, p.name),
                        label: p.name.clone(),
                        kind: CatalogNodeKindDto::Parameter,
                        category: None,
                        detail: Some(format!("pos {} · {}", p.position, p.type_name)),
                        is_system: false,
                        has_children: false,
                        request: None,
                        children: Vec::new(),
                    })
                    .collect();
                Ok(out)
            }
        }
    }

    // When expanding a table (Campos req) we also want the UI to offer indices/constraints
    // as siblings. The table node initially returns only fields; to improve navigation,
    // Traite tables as having virtual children: expose three category children instead.
    // NOTE: Tabelas are created in Categoria::Tabelas with children empty and expand via
    // Campos only. Better UX would attach sub-nodes. We attach them here in node_dto for
    // rel nodes by creating them as part of node children at the table level instead.
}

// tabela_node not needed externally.

fn rotulo_categoria(c: CategoriaCatalogo) -> &'static str {
    match c {
        CategoriaCatalogo::Tabelas => "Tabelas",
        CategoriaCatalogo::Views => "Views",
        CategoriaCatalogo::Indices => "Índices",
        CategoriaCatalogo::Dominios => "Domínios",
        CategoriaCatalogo::Generators => "Generators",
        CategoriaCatalogo::Procedures => "Procedures",
        CategoriaCatalogo::Triggers => "Triggers",
    }
}

trait ToDtoCat {
    fn into_dto(self) -> CatalogCategoryDto;
}
impl ToDtoCat for CategoriaCatalogo {
    fn into_dto(self) -> CatalogCategoryDto {
        match self {
            CategoriaCatalogo::Tabelas => CatalogCategoryDto::Tabelas,
            CategoriaCatalogo::Views => CatalogCategoryDto::Views,
            CategoriaCatalogo::Indices => CatalogCategoryDto::Indices,
            CategoriaCatalogo::Dominios => CatalogCategoryDto::Dominios,
            CategoriaCatalogo::Generators => CatalogCategoryDto::Generators,
            CategoriaCatalogo::Procedures => CatalogCategoryDto::Procedures,
            CategoriaCatalogo::Triggers => CatalogCategoryDto::Triggers,
        }
    }
}

// relaxation: expose kind for mapping

fn node_dto(
    profile_id: String,
    node: CatalogNodeRequestDto,
    children: Vec<CatalogNodeDto>,
) -> CatalogNodeDto {
    let (id, label, kind, category, detail, has_children, req) = match &node {
        CatalogNodeRequestDto::Raiz => (
            "root".to_string(),
            format!("Catálogo ({})", profile_id),
            CatalogNodeKindDto::Root,
            None,
            None,
            true,
            Some(CatalogNodeRequestDto::Raiz),
        ),
        CatalogNodeRequestDto::Categoria { categoria } => (
            format!("cat:{}", categoria_str(*categoria)),
            categoria.rotulo().to_string(),
            CatalogNodeKindDto::Category,
            Some(*categoria),
            None,
            true,
            Some(CatalogNodeRequestDto::Categoria {
                categoria: *categoria,
            }),
        ),
        CatalogNodeRequestDto::Relacao { relacao } => (
            format!("rel:{}", relacao),
            relacao.clone(),
            CatalogNodeKindDto::Object,
            None,
            None,
            true,
            Some(CatalogNodeRequestDto::Relacao {
                relacao: relacao.clone(),
            }),
        ),
        CatalogNodeRequestDto::Campos { relacao } => (
            format!("fields:{}", relacao),
            "Campos".to_string(),
            CatalogNodeKindDto::Category,
            None,
            None,
            true,
            Some(CatalogNodeRequestDto::Campos {
                relacao: relacao.clone(),
            }),
        ),
        CatalogNodeRequestDto::Indices { relacao } => (
            format!("indices:{}", relacao),
            "Índices".to_string(),
            CatalogNodeKindDto::Category,
            None,
            None,
            true,
            Some(CatalogNodeRequestDto::Indices {
                relacao: relacao.clone(),
            }),
        ),
        CatalogNodeRequestDto::Constraints { relacao } => (
            format!("constraints:{}", relacao),
            "Chaves".to_string(),
            CatalogNodeKindDto::Category,
            None,
            None,
            true,
            Some(CatalogNodeRequestDto::Constraints {
                relacao: relacao.clone(),
            }),
        ),
        CatalogNodeRequestDto::Parametros { procedimento } => (
            format!("params:{}", procedimento),
            "Parâmetros".to_string(),
            CatalogNodeKindDto::Category,
            None,
            None,
            true,
            Some(CatalogNodeRequestDto::Parametros {
                procedimento: procedimento.clone(),
            }),
        ),
    };
    CatalogNodeDto {
        node_id: id,
        label,
        kind,
        category,
        detail,
        is_system: false,
        has_children,
        request: req,
        children,
    }
}

fn categoria_str(c: CatalogCategoryDto) -> &'static str {
    match c {
        CatalogCategoryDto::Tabelas => "tabelas",
        CatalogCategoryDto::Views => "views",
        CatalogCategoryDto::Indices => "indices",
        CatalogCategoryDto::Dominios => "dominios",
        CatalogCategoryDto::Generators => "generators",
        CatalogCategoryDto::Procedures => "procedures",
        CatalogCategoryDto::Triggers => "triggers",
    }
}

fn passa_filtro(nome: &str, filtro: Option<&str>) -> bool {
    match filtro {
        Some(t) => nome.to_lowercase().contains(&t.to_lowercase()),
        None => true,
    }
}

fn adaptador_err(e: impl std::fmt::Display) -> CommandError {
    CommandError::desconhecido(e.to_string())
}
