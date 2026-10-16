//! Classificação de risco de SQL (Spec 07 §revisão, Passo 4 da Fase 3).
//!
//! A classificação é **conservadora de propósito**: qualquer coisa que não seja
//! reconhecida como leitura cai no nível mais alto. O objetivo é fazer o app
//! pedir confirmação antes do usuário executar, nunca Edgar-izar permission
//! implícita — e nunca executar SQL de IA sozinho.

/// Nível de risco de uma instrução SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NivelRisco {
    /// Apenas leitura de dados (`SELECT`).
    Leitura,
    /// Muda estado da sessão (`COMMIT`, `ROLLBACK`, `SET TRANSACTION`).
    Transacao,
    /// Muda dados (`INSERT`, `UPDATE`, `DELETE`, `MERGE`).
    Escrita,
    /// Muda objetos do banco (`CREATE`, `ALTER`, `DROP`, `GRANT`…).
    Estrutural,
    /// Não reconhecido ou potencialmente destrutivo.
    Destrutivo,
}

impl NivelRisco {
    /// Identificador estável (para DTO/UI).
    pub fn as_str(self) -> &'static str {
        match self {
            NivelRisco::Leitura => "leitura",
            NivelRisco::Transacao => "transacao",
            NivelRisco::Escrita => "escrita",
            NivelRisco::Estrutural => "estrutural",
            NivelRisco::Destrutivo => "destrutivo",
        }
    }

    /// Rótulo curto para a UI.
    pub fn rotulo(self) -> &'static str {
        match self {
            NivelRisco::Leitura => "Leitura",
            NivelRisco::Transacao => "Transação",
            NivelRisco::Escrita => "Escrita",
            NivelRisco::Estrutural => "Estrutural",
            NivelRisco::Destrutivo => "Destrutivo",
        }
    }

    /// `true` quando a execução precisa de confirmação explícita do usuário.
    pub fn exige_confirmacao(self) -> bool {
        self >= NivelRisco::Escrita
    }
}

/// Resultado da avaliação de um texto SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvaliacaoRisco {
    /// Nível mais alto entre as instruções encontradas.
    pub nivel: NivelRisco,
    /// Motivos (em português) que explicaram a classificação.
    pub motivos: Vec<String>,
    /// `true` quando há mais de uma instrução no texto.
    pub multiplas_instrucoes: bool,
}

impl AvaliacaoRisco {
    /// `true` quando a UI deve pedir confirmação antes de executar.
    pub fn exige_confirmacao(&self) -> bool {
        self.nivel.exige_confirmacao()
    }
}

/// Avalia o risco de um texto SQL.
///
/// Comentários e literais são removidos antes da análise para que
/// `SELECT 1 -- DELETE FROM x` continue classificado como leitura.
pub fn avaliar_sql(sql: &str) -> AvaliacaoRisco {
    let limpo = remove_comentarios_e_literais(sql);
    let instrucoes: Vec<&str> = limpo
        .split(';')
        .map(|i| i.trim())
        .filter(|i| !i.is_empty())
        .collect();

    if instrucoes.is_empty() {
        return AvaliacaoRisco {
            nivel: NivelRisco::Leitura,
            motivos: vec!["nenhuma instrução reconhecida".to_string()],
            multiplas_instrucoes: false,
        };
    }

    let mut nivel = NivelRisco::Leitura;
    let mut motivos: Vec<String> = Vec::new();

    let total_instrucoes = instrucoes.len();
    for instrucao in instrucoes {
        let (n, razao) = classificar(instrucao);
        if n > nivel {
            nivel = n;
        }
        if let Some(r) = razao {
            motivos.push(r);
        }
    }

    if motivos.is_empty() {
        motivos.push(format!("instrução classificada como {}", nivel.rotulo()));
    }
    motivos.push(format!("{total_instrucoes} instrução(ões) analisada(s)"));

    AvaliacaoRisco {
        nivel,
        motivos,
        multiplas_instrucoes: total_instrucoes > 1,
    }
}

/// Classifica uma instrução já limpa.
fn classificar(instrucao: &str) -> (NivelRisco, Option<String>) {
    let maiuscula = instrucao.to_uppercase();
    let primeira = maiuscula
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
        .to_string();

    let nivel = match primeira.as_str() {
        "SELECT" | "WITH" => {
            // CTE que esconde escrita (ex.: WITH x AS (DELETE …)) continua escrita.
            if contem(&maiuscula, "DELETE")
                || contem(&maiuscula, "UPDATE")
                || contem(&maiuscula, "INSERT")
            {
                NivelRisco::Escrita
            } else {
                NivelRisco::Leitura
            }
        }
        "COMMIT" | "ROLLBACK" | "SET" | "SAVEPOINT" => NivelRisco::Transacao,
        "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "PUT" => NivelRisco::Escrita,
        "CREATE" | "ALTER" | "DROP" | "RECREATE" | "COMMENT" | "GRANT" | "REVOKE" => {
            NivelRisco::Estrutural
        }
        "EXECUTE" => NivelRisco::Destrutivo,
        _ => NivelRisco::Destrutivo,
    };

    let motivo = Some(match nivel {
        NivelRisco::Leitura => format!("começa com `{primeira}` — apenas leitura"),
        NivelRisco::Transacao => {
            format!("começa com `{primeira}` — mexe só no estado da transação")
        }
        NivelRisco::Escrita => format!("começa com `{primeira}` — altera dados"),
        NivelRisco::Estrutural => format!("começa com `{primeira}` — altera objetos do banco"),
        NivelRisco::Destrutivo => {
            if primeira.is_empty() {
                "instrução não reconhecida".to_string()
            } else {
                format!("`{primeira}` não reconhecida — tratado como destrutivo por precaução")
            }
        }
    });

    (nivel, motivo)
}

/// `true` quando `palavra` aparece como token independente.
///
/// Tokens são normalizados (sem pontuação) para que `(DELETE` conte como
/// `DELETE` — é assim que escrita costuma aparecer escondida numa CTE.
fn contem(texto: &str, palavra: &str) -> bool {
    texto
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
        .any(|t| t == palavra)
}

/// Remove comentários (`--`, `/* */`) e literais (`'...'`) para não classificar
/// pelo conteúdo de texto.
fn remove_comentarios_e_literais(sql: &str) -> String {
    let mut saida = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut em_string = false;

    while let Some(c) = chars.next() {
        if em_string {
            if c == '\'' {
                em_string = false;
                saida.push(' ');
            }
            continue;
        }
        match c {
            '\'' => {
                em_string = true;
                saida.push(' ');
            }
            '-' if chars.peek() == Some(&'-') => {
                for proximo in chars.by_ref() {
                    if proximo == '\n' {
                        saida.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut anterior = ' ';
                for proximo in chars.by_ref() {
                    if anterior == '*' && proximo == '/' {
                        break;
                    }
                    anterior = proximo;
                }
                saida.push(' ');
            }
            outro => saida.push(outro),
        }
    }
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_e_leitura() {
        let a = avaliar_sql("SELECT FIRST 10 * FROM CLIENTES");
        assert_eq!(a.nivel, NivelRisco::Leitura);
        assert!(!a.exige_confirmacao());
    }

    #[test]
    fn insert_update_delete_exigem_confirmacao() {
        for sql in [
            "INSERT INTO CLIENTES (NOME) VALUES ('A')",
            "UPDATE CLIENTES SET NOME = 'B'",
            "DELETE FROM CLIENTES",
        ] {
            let a = avaliar_sql(sql);
            assert_eq!(a.nivel, NivelRisco::Escrita, "{sql}");
            assert!(a.exige_confirmacao(), "{sql}");
        }
    }

    #[test]
    fn ddl_e_estrutural() {
        for sql in [
            "CREATE TABLE T (A INT)",
            "DROP TABLE T",
            "ALTER TABLE T ADD B INT",
        ] {
            assert_eq!(avaliar_sql(sql).nivel, NivelRisco::Estrutural, "{sql}");
        }
    }

    #[test]
    fn desconhecido_cai_em_destrutivo() {
        let a = avaliar_sql("FOO BAR BAZ");
        assert_eq!(a.nivel, NivelRisco::Destrutivo);
        assert!(a.exige_confirmacao());
        assert!(
            a.motivos
                .iter()
                .any(|m| m.contains("tratado como destrutivo")),
            "motivos: {:?}",
            a.motivos
        );
    }

    #[test]
    fn execute_e_destrutivo_por_precaucao() {
        let a = avaliar_sql("EXECUTE BLOCK AS BEGIN END");
        assert_eq!(a.nivel, NivelRisco::Destrutivo);
        assert!(a.exige_confirmacao());
    }

    #[test]
    fn comentario_com_delete_nao_eleva_o_risco() {
        let a = avaliar_sql("SELECT 1 FROM RDB$DATABASE -- DELETE FROM CLIENTES");
        assert_eq!(a.nivel, NivelRisco::Leitura);
    }

    #[test]
    fn literal_com_delete_nao_eleva_o_risco() {
        let a = avaliar_sql("SELECT 'DELETE FROM X' FROM RDB$DATABASE");
        assert_eq!(a.nivel, NivelRisco::Leitura);
    }

    #[test]
    fn cte_com_escrita_vira_escrita() {
        let a = avaliar_sql("WITH X AS (DELETE FROM CLIENTES RETURNING ID) SELECT * FROM X");
        assert_eq!(a.nivel, NivelRisco::Escrita);
        assert!(a.exige_confirmacao());
    }

    #[test]
    fn varias_instrucoes_pegam_o_maior_risco() {
        let a = avaliar_sql("SELECT 1 FROM RDB$DATABASE; DELETE FROM CLIENTES");
        assert_eq!(a.nivel, NivelRisco::Escrita);
        assert!(a.multiplas_instrucoes);
    }

    #[test]
    fn transacao_nao_exige_confirmacao() {
        let a = avaliar_sql("COMMIT");
        assert_eq!(a.nivel, NivelRisco::Transacao);
        assert!(!a.exige_confirmacao());
    }

    #[test]
    fn texto_vazio_e_leitura() {
        let a = avaliar_sql("   ");
        assert_eq!(a.nivel, NivelRisco::Leitura);
        assert!(!a.exige_confirmacao());
    }

    #[test]
    fn rotulos_e_flags_sao_estaveis() {
        assert_eq!(NivelRisco::Leitura.as_str(), "leitura");
        assert_eq!(NivelRisco::Escrita.rotulo(), "Escrita");
        assert!(NivelRisco::Estrutural.exige_confirmacao());
        assert!(!NivelRisco::Transacao.exige_confirmacao());
    }
}
