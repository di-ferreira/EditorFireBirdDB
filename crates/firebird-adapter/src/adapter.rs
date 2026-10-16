//! Conexão Firebird via `rsfbclient` (native, `libfbclient.so` carregada em
//! runtime) atrás do trait [`FirebirdAdapter`].
//!
//! O campo `conn` é do tipo real exposto pelo rsfbclient 0.29,
//! `Connection<NativeFbClient<DynLoad>>` (o `ManualTc` de versões antigas do
//! driver não existe mais); `DynLoad { charset, lib_path }` é montado pelo
//! builder `with_dyn_load(...).charset(...)` na conexão.

use std::path::PathBuf;

use firebird_domain::{DomainError, ServerVersion};
use rsfbclient::{Connection, Queryable, Row};
use rsfbclient_native::{DynLoad, NativeFbClient};

/// Parâmetros de conexão com um servidor Firebird.
///
/// Os campos `Debug` e `Display` são manuais e **redigem a senha** (FR-016):
/// nunca vaza em log, panic ou DTO.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnParams {
    /// Hostname ou endereço IP do servidor.
    pub host: String,
    /// Porta TCP do servidor (padrão do Firebird: 3050).
    pub port: u16,
    /// Nome/caminho do banco de dados no servidor.
    pub database: String,
    /// Usuário de autenticação.
    pub user: String,
    /// Senha de autenticação — nunca exibida em texto cru.
    pub password: String,
    /// Charset da conexão (ex.: `UTF8`, padrão recomendado).
    pub charset: String,
    /// Caminho da `libfbclient.so` a carregar (seleção manual ou descoberta).
    pub client_library_path: PathBuf,
}

impl std::fmt::Debug for ConnParams {
    /// Imprime os campos com a senha redigida como `***`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnParams")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &"***")
            .field("charset", &self.charset)
            .field("client_library_path", &self.client_library_path)
            .finish()
    }
}

impl std::fmt::Display for ConnParams {
    /// Versão legível, com a senha redigida como `***`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ConnParams {{ host: {}, port: {}, database: {}, user: {}, password: ***, charset: {}, client_library_path: {} }}",
            self.host,
            self.port,
            self.database,
            self.user,
            self.charset,
            self.client_library_path.display()
        )
    }
}

/// Abstração comum dos drivers Firebird candidatos (ADR-002).
///
/// Versão spike simplificada da Fase 0; será estendida na Fase 1.
pub trait FirebirdAdapter {
    /// Versão do servidor, via `RDB$GET_CONTEXT('SYSTEM', 'ENGINE_VERSION')`.
    fn server_version(&mut self) -> Result<ServerVersion, DomainError>;

    /// Executa `sql` e devolve as linhas como texto (colunas na ordem do
    /// `SELECT`; [`None`] para NULL).
    fn query_rows(&mut self, sql: &str) -> Result<Vec<Vec<Option<String>>>, DomainError>;

    /// Executa `sql` com parâmetros posicionais `?` e devolve as linhas como
    /// texto ([`None`] para NULL no resultado).
    ///
    /// Cada parâmetro é texto ([`Some`]) ou NULL ([`None`]); a conversão para
    /// o tipo da coluna é do servidor (parametrização real, sem concatenação
    /// de string — fechamento de injeção na Fase 1).
    fn query_rows_params(
        &mut self,
        sql: &str,
        params: &[Option<&str>],
    ) -> Result<Vec<Vec<Option<String>>>, DomainError>;

    /// Executa `sql` (DML/DDL, sem cursor de saída) e devolve o número de
    /// linhas afetadas ([`None`]/0 quando o servidor não informa).
    ///
    /// Existe por necessidade do harness de integração (Task 4): o
    /// `rsfbclient` 0.29 (backend nativo) classifica **todo** statement como
    /// SELECT — o parse de `isc_info_sql_stmt_type` lê o terminator
    /// `isc_info_end` (= 1) como tipo — e então tenta `isc_dsql_fetch` após
    /// DDL/DML; com o cliente 2.5 isso é SIGSEGV e com 3.0+ vira o erro
    /// `-504 (Cursor is not open)`. `Execute::execute` do rsfbclient não faz
    /// fetch e contorna o bug; evidência (backtrace) no relatório da Task 4.
    fn execute(&mut self, sql: &str, params: &[Option<&str>]) -> Result<usize, DomainError>;

    /// Executa `sql` com parâmetros e devolve **nomes das colunas** (quando a
    /// primeira linha trouxer colunas) + linhas como texto ([`None`] para NULL).
    ///
    /// Quando o `SELECT` devolver **zero linhas**, a lista de nomes pode vir
    /// vazia — o chamador deve então usar rótulos neutros (`COL0`, ...).
    #[allow(clippy::type_complexity)]
    fn query_rows_with_columns(
        &mut self,
        sql: &str,
        params: &[Option<&str>],
    ) -> Result<(Vec<String>, Vec<Vec<Option<String>>>), DomainError>;
}

/// Adapter concreto sobre o `rsfbclient` nativo com carregamento dinâmico da
/// `libfbclient.so` escolhida em [`ConnParams::client_library_path`].
pub struct RsFbClientAdapter {
    conn: Connection<NativeFbClient<DynLoad>>,
}

impl RsFbClientAdapter {
    /// Abre a conexão carregando a biblioteca indicada em `params`.
    ///
    /// O builder do rsfbclient monta `DynLoad { charset, lib_path }` com o
    /// caminho escolhido; erros do driver são mapeados para
    /// [`DomainError::Driver`] com mensagem redigida (FR-016).
    pub fn connect(params: &ConnParams) -> Result<Self, DomainError> {
        let charset: rsfbclient::Charset = params.charset.parse().map_err(|e| {
            crate::erro_driver(format!("charset inválido '{}': {e}", params.charset))
        })?;

        let mut builder = rsfbclient::builder_native()
            .with_dyn_load(params.client_library_path.to_string_lossy().into_owned())
            .with_remote();
        builder
            .host(&params.host)
            .port(params.port)
            .user(&params.user)
            .pass(&params.password)
            .db_name(&params.database)
            .charset(charset);

        let conn = builder
            .connect()
            .map_err(|e| crate::erro_driver(format!("{e}")))?;

        Ok(Self { conn })
    }
}

/// Consulta usada para obter a versão do engine (disponível desde 2.0).
const SQL_ENGINE_VERSION: &str =
    "SELECT RDB$GET_CONTEXT('SYSTEM', 'ENGINE_VERSION') FROM RDB$DATABASE";

impl FirebirdAdapter for RsFbClientAdapter {
    fn server_version(&mut self) -> Result<ServerVersion, DomainError> {
        let linhas = self.query_rows(SQL_ENGINE_VERSION)?;
        let raw = linhas
            .first()
            .and_then(|linha| linha.first())
            .cloned()
            .flatten()
            .unwrap_or_default();
        ServerVersion::parse(&raw).ok_or(DomainError::ParseVersion { raw })
    }

    fn query_rows(&mut self, sql: &str) -> Result<Vec<Vec<Option<String>>>, DomainError> {
        self.query_rows_params(sql, &[])
    }

    fn query_rows_params(
        &mut self,
        sql: &str,
        params: &[Option<&str>],
    ) -> Result<Vec<Vec<Option<String>>>, DomainError> {
        let rows: Vec<Row> = self
            .conn
            .query(sql, params_em_rsfb(params))
            .map_err(|e| crate::erro_driver(format!("{e}")))?;
        linhas_como_texto(rows)
    }

    fn query_rows_with_columns(
        &mut self,
        sql: &str,
        params: &[Option<&str>],
    ) -> Result<(Vec<String>, Vec<Vec<Option<String>>>), DomainError> {
        let rows: Vec<Row> = self
            .conn
            .query(sql, params_em_rsfb(params))
            .map_err(|e| crate::erro_driver(format!("{e}")))?;
        let nomes = rows
            .first()
            .map(|row| row.cols.iter().map(|col| col.name.to_string()).collect())
            .unwrap_or_default();
        let linhas = rows
            .into_iter()
            .map(|row| {
                (0..row.cols.len())
                    .map(|i| row.get::<Option<String>>(i))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::erro_driver(format!("{e}")))?;
        Ok((nomes, linhas))
    }

    fn execute(&mut self, sql: &str, params: &[Option<&str>]) -> Result<usize, DomainError> {
        // Caminho sem fetch (`Execute::execute`): obrigatório para DML/DDL
        // enquanto o bug de classificação do rsfbclient 0.29 nativo existir
        // (ver doc da trait).
        use rsfbclient::Execute;
        self.conn
            .execute(sql, params_em_rsfb(params))
            .map_err(|e| crate::erro_driver(format!("{e}")))
    }
}

/// Converte parâmetros de domínio (`Some(texto)`/`None` = NULL) para o tipo
/// do rsfbclient: `Text` (o driver envia como SQL_TEXT e o servidor converte
/// para o tipo contextual da coluna/`?`) e `Null`.
fn params_em_rsfb(params: &[Option<&str>]) -> Vec<rsfbclient::SqlType> {
    params
        .iter()
        .map(|param| match param {
            Some(texto) => rsfbclient::SqlType::Text((*texto).to_string()),
            None => rsfbclient::SqlType::Null,
        })
        .collect()
}

/// Converte linhas brutas do rsfbclient em texto ([`None`] para NULL).
fn linhas_como_texto(rows: Vec<Row>) -> Result<Vec<Vec<Option<String>>>, DomainError> {
    rows.into_iter()
        .map(|row| {
            (0..row.cols.len())
                .map(|i| row.get::<Option<String>>(i))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::erro_driver(format!("{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parâmetros de exemplo com senha sintética (nunca dado real).
    fn params_de_exemplo() -> ConnParams {
        ConnParams {
            host: "localhost".to_string(),
            port: 3050,
            database: "/dados/app.fdb".to_string(),
            user: "SYSDBA".to_string(),
            password: "segredo".to_string(),
            charset: "UTF8".to_string(),
            client_library_path: PathBuf::from("/usr/lib/libfbclient.so.3.0.11"),
        }
    }

    #[test]
    fn debug_de_conn_params_nao_exibe_a_senha() {
        let texto = format!("{:?}", params_de_exemplo());
        assert!(
            !texto.contains("segredo"),
            "a senha não pode aparecer no Debug: {texto}"
        );
        assert!(
            texto.contains("password: \"***\""),
            "a senha deve aparecer redigida como ***: {texto}"
        );
        assert!(
            texto.contains("ConnParams"),
            "o Debug deve nomear o tipo: {texto}"
        );
    }

    #[test]
    fn display_de_conn_params_nao_exibe_a_senha() {
        let texto = format!("{}", params_de_exemplo());
        assert!(
            !texto.contains("segredo"),
            "a senha não pode aparecer no Display: {texto}"
        );
        assert!(
            texto.contains("password: ***"),
            "a senha deve aparecer redigida como ***: {texto}"
        );
    }

    #[test]
    fn erro_de_driver_e_redigido_na_construcao() {
        let erro = crate::erro_driver("user=SYSDBA password=segredo123 negado".to_string());
        let texto = format!("{erro}");
        assert!(
            !texto.contains("segredo123"),
            "a senha não pode aparecer no erro: {texto}"
        );
        assert!(
            texto.contains("password=***"),
            "a senha deve sair redigida: {texto}"
        );
    }

    #[test]
    fn connect_com_biblioteca_inexistente_falha_sem_vazar_senha() {
        let dir =
            std::env::temp_dir().join(format!("firebird-adapter-connect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar diretório temporário");

        let params = ConnParams {
            client_library_path: dir.join("libfbclient-nao-existe.so"),
            ..params_de_exemplo()
        };

        let erro = RsFbClientAdapter::connect(&params)
            .err()
            .expect("biblioteca inexistente deve falhar sem rede");

        assert!(
            matches!(erro, DomainError::Driver { .. }),
            "erro de carregamento deve virar DomainError::Driver: {erro:?}"
        );
        let texto = format!("{erro}");
        assert!(
            !texto.contains("segredo"),
            "a senha não pode aparecer no erro: {texto}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn connect_com_charset_invalido_retorna_erro_de_driver() {
        let params = ConnParams {
            charset: "nao-existe".to_string(),
            ..params_de_exemplo()
        };

        let erro = RsFbClientAdapter::connect(&params)
            .err()
            .expect("charset inválido deve falhar antes de qualquer conexão");

        assert!(
            matches!(erro, DomainError::Driver { .. }),
            "charset inválido deve virar DomainError::Driver: {erro:?}"
        );
        let texto = format!("{erro}");
        assert!(
            texto.contains("nao-existe"),
            "o erro deve nomear o charset rejeitado: {texto}"
        );
    }
}
