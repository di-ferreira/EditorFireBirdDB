//! Migrações versionadas do banco local (Spec 09, §2; ADR-003).
//!
//! Cada migração é um arquivo `.sql` incluído em tempo de compilação com
//! [`include_str!`], numerado por versão crescente. [`Db::migrate`](crate::Db::migrate)
//! cria `schema_migrations`, aplica somente as versões pendentes, cada uma em
//! transação própria, e é idempotente.

/// Uma migração de schema versionada.
pub struct Migration {
    /// Versão aplicada (monótona, ordem de aplicação).
    pub version: i64,
    /// Nome lógico da migração, para diagnóstico (nunca contém dado do usuário).
    pub nome: &'static str,
    /// SQL da migração.
    pub sql: &'static str,
}

/// Todas as migrações do crate, em ordem de versão.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    nome: "0001_inicial",
    sql: include_str!("../migrations/0001_inicial.sql"),
}];

#[cfg(test)]
mod tests {
    use super::MIGRATIONS;
    use crate::Db;

    fn tabela_existe(db: &Db, nome: &str) -> bool {
        let mut stmt = db
            .conn_for_tests()
            .prepare("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .expect("preparando consulta de sqlite_master");
        let total: i64 = stmt
            .query_row([nome], |r| r.get(0))
            .expect("consultando sqlite_master");
        total > 0
    }

    #[test]
    fn migrate_cria_todas_as_tabelas_da_spec_09() {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        db.migrate().expect("aplicando migração inicial");

        for tabela in [
            "connection_profiles",
            "query_history",
            "saved_scripts",
            "ai_provider_profiles",
            "schema_descriptions",
            "ai_interactions",
            "app_settings",
            "schema_migrations",
        ] {
            assert!(
                tabela_existe(&db, tabela),
                "a tabela {tabela} da Spec 09 deveria existir após migrate()"
            );
        }
    }

    #[test]
    fn migrate_e_idempotente_quando_executada_duas_vezes() {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        db.migrate().expect("primeira aplicação");
        db.migrate().expect("segunda aplicação não pode falhar");
        db.migrate().expect("terceira aplicação não pode falhar");
    }

    #[test]
    fn migrate_registra_a_versao_aplicada() {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        db.migrate().expect("aplicando migração");

        let versao: i64 = db
            .conn_for_tests()
            .query_row("SELECT max(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .expect("lendo versão aplicada");
        assert_eq!(versao, 1, "a versão 1 deve estar registrada");
    }

    #[test]
    fn migrations_tem_versoes_unicas_e_crescentes() {
        let mut anterior = 0;
        for m in MIGRATIONS {
            assert!(
                m.version > anterior,
                "versões devem crescer e ser únicas: {} depois de {anterior}",
                m.version
            );
            anterior = m.version;
        }
    }

    #[test]
    fn migracao_inicial_nao_declara_coluna_de_senha_ou_token() {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        db.migrate().expect("aplicando migração");

        for tabela in [
            "connection_profiles",
            "query_history",
            "saved_scripts",
            "ai_provider_profiles",
            "schema_descriptions",
            "ai_interactions",
            "app_settings",
        ] {
            let mut stmt = db
                .conn_for_tests()
                .prepare(&format!("PRAGMA table_info({tabela})"))
                .expect("lendo colunas da tabela");
            let colunas = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .expect("listando colunas");
            for coluna in colunas {
                let coluna = coluna.expect("nome da coluna");
                let min = coluna.to_ascii_lowercase();
                // `secret_ref` é a única coluna permitida: guarda a referência
                // do cofre, nunca o segredo (ADR-003).
                if min == "secret_ref" {
                    continue;
                }
                for proibida in ["password", "passwd", "senha", "secret", "api_key", "token"] {
                    assert!(
                        !min.contains(proibida),
                        "a tabela {tabela} não pode ter coluna de segredo: {coluna}"
                    );
                }
            }
        }
    }

    #[test]
    fn perfis_referenciam_o_cofre_apenas_por_secret_ref() {
        let db = Db::open_in_memory().expect("abrindo banco em memória");
        db.migrate().expect("aplicando migração");

        let mut stmt = db
            .conn_for_tests()
            .prepare("PRAGMA table_info(connection_profiles)")
            .expect("lendo colunas de connection_profiles");
        let colunas: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .expect("listando colunas")
            .collect::<Result<_, _>>()
            .expect("colunas válidas");

        assert!(
            colunas.iter().any(|c| c == "secret_ref"),
            "deve existir secret_ref, que aponta para o cofre: {colunas:?}"
        );
        assert!(
            !colunas.iter().any(|c| c == "password" || c == "senha"),
            "não pode existir coluna de senha: {colunas:?}"
        );
    }
}
