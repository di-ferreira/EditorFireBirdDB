//! Interpretação da versão do servidor Firebird.

/// Versão do servidor Firebird: `major.minor[.patch]`.
///
/// Derivada de `RDB$GET_CONTEXT('SYSTEM', 'ENGINE_VERSION')` ou do banner do
/// isql (ex.: `WI-V2.5.9.27139 Firebird 2.5 WI-V`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: Option<u8>,
}

impl ServerVersion {
    /// Interpreta a versão a partir de uma string livre, sem regex.
    ///
    /// Remove os prefixos conhecidos (`WI-V`, `TDSV`, `OI-V`) e extrai a
    /// primeira sequência `major.minor[.patch]`; o quarto componente de build
    /// (ex.: `.27139`) é ignorado. Retorna [`None`] para entrada vazia ou que
    /// não contenha uma versão válida.
    pub fn parse(raw: &str) -> Option<ServerVersion> {
        let sem_prefixo = strip_known_prefixes(raw.trim());
        find_version(sem_prefixo)
    }

    /// Indica se o servidor expõe `RDB$RELATION_TYPE` no catálogo.
    ///
    /// O campo existe a partir do Firebird 2.1; esta implementação usa o gate
    /// `>= (2, 1)` por conservadorismo e pode ser revisada com evidência da
    /// prova de conceito (Task 4), caso o teste real mostre 2.0 suportando o
    /// campo.
    pub fn supports_relation_type(&self) -> bool {
        self.major > 2 || (self.major == 2 && self.minor >= 1)
    }
}

/// Remove, repetidamente, os prefixos de banner conhecidos do Firebird.
fn strip_known_prefixes(s: &str) -> &str {
    const PREFIXES: [&str; 3] = ["WI-V", "TDSV", "OI-V"];
    let mut restante = s;
    loop {
        match PREFIXES.iter().find_map(|p| restante.strip_prefix(p)) {
            Some(sem_prefixo) => restante = sem_prefixo,
            None => return restante,
        }
    }
}

/// Procura a primeira sequência `d+.d+(.d+)?` cujos componentes caibam em
/// [`u8`], começando apenas em limites de sequência de dígitos.
fn find_version(s: &str) -> Option<ServerVersion> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let inicio_de_sequencia = i == 0 || !bytes[i - 1].is_ascii_digit();
        if inicio_de_sequencia {
            if let Some(v) = parse_version_at(s, i) {
                return Some(v);
            }
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Tenta interpretar `major.minor[.patch]` começando exatamente em `start`.
fn parse_version_at(s: &str, start: usize) -> Option<ServerVersion> {
    let mut resto = &s[start..];
    let major = take_u8(&mut resto)?;
    resto = resto.strip_prefix('.')?;
    let minor = take_u8(&mut resto)?;
    let patch = if resto.starts_with('.') {
        resto = &resto[1..];
        Some(take_u8(&mut resto)?)
    } else {
        None
    };
    Some(ServerVersion {
        major,
        minor,
        patch,
    })
}

/// Consome a sequência de dígitos inicial de `s` e a converte em [`u8`].
fn take_u8(s: &mut &str) -> Option<u8> {
    let fim = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if fim == 0 {
        return None;
    }
    let (digits, cauda) = s.split_at(fim);
    *s = cauda;
    digits.parse::<u8>().ok()
}

#[cfg(test)]
mod tests {
    use super::ServerVersion;

    #[test]
    fn parse_banner_wi_v_interpreta_major_minor_patch() {
        let v = ServerVersion::parse("WI-V2.5.9.27139 Firebird 2.5 WI-V")
            .expect("banner do Firebird 2.5 deve ser interpretado");
        assert_eq!((v.major, v.minor, v.patch), (2, 5, Some(9)));
    }

    #[test]
    fn parse_banner_tdsv_interpreta_major_minor_patch() {
        let v = ServerVersion::parse("TDSV5.0.2.16063 Firebird 5.0")
            .expect("banner do Firebird 5.0 deve ser interpretado");
        assert_eq!((v.major, v.minor, v.patch), (5, 0, Some(2)));
    }

    #[test]
    fn parse_sem_patch_retorna_none_no_patch() {
        let v = ServerVersion::parse("2.5").expect("versão curta deve ser interpretada");
        assert_eq!((v.major, v.minor, v.patch), (2, 5, None));
    }

    #[test]
    fn parse_string_vazia_retorna_none() {
        assert_eq!(ServerVersion::parse(""), None);
    }

    #[test]
    fn relation_type_disponivel_desde_2_1() {
        assert!(ServerVersion {
            major: 2,
            minor: 1,
            patch: Some(0)
        }
        .supports_relation_type());
        assert!(ServerVersion {
            major: 3,
            minor: 0,
            patch: None
        }
        .supports_relation_type());
        assert!(ServerVersion {
            major: 5,
            minor: 0,
            patch: Some(2)
        }
        .supports_relation_type());
    }

    #[test]
    fn relation_type_indisponivel_antes_de_2_1() {
        assert!(!ServerVersion {
            major: 2,
            minor: 0,
            patch: None
        }
        .supports_relation_type());
        assert!(!ServerVersion {
            major: 1,
            minor: 5,
            patch: Some(0)
        }
        .supports_relation_type());
    }
}
