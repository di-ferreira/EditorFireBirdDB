//! Descoberta e validação da biblioteca cliente nativa (`libfbclient.so`).
//!
//! A descoberta é somente leitura: varre diretórios conhecidos (Spec 01, §3),
//! nunca recursivamente, nunca substitui bibliotecas do sistema nem cria
//! symlinks. Symlinks que apontam para o mesmo arquivo real são deduplicados
//! pelo caminho resolvido.

use std::path::{Path, PathBuf};

use firebird_domain::{DomainError, ServerVersion};

/// Prefixo dos nomes de arquivo candidatos a `libfbclient.so` versionada.
const PREFIXO_LIBFBCLIENT: &str = "libfbclient.so";

/// Bytes lidos do início do arquivo para validar o cabeçalho ELF
/// (magic + `e_machine` no offset 18, little endian).
const TAMANHO_CABECALHO_ELF: usize = 20;

/// Diretórios varridos por [`ClientLibrary::discover`], sem recursão.
const DIRETORIOS_DE_BUSCA: [&str; 5] = [
    "/usr/lib",
    "/usr/lib64",
    "/usr/lib/x86_64-linux-gnu",
    "/opt/firebird/lib",
    "/usr/local/firebird/lib",
];

/// Uma `libfbclient.so` encontrada no sistema, com a versão deduzida do nome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientLibrary {
    /// Caminho do arquivo (o nome encontrado, não o symlink resolvido).
    pub path: PathBuf,
    /// Versão extraída do nome do arquivo; [`None`] quando o nome não a traz
    /// (ex.: o symlink `libfbclient.so` sem sufixo).
    pub version: Option<ServerVersion>,
}

impl ClientLibrary {
    /// Varre os diretórios conhecidos por `libfbclient.so*` e retorna as
    /// bibliotecas encontradas, deduplicadas pelo caminho resolvido
    /// (um symlink e seu alvo contam uma única vez; prevalece a entrada cujo
    /// nome traz a versão).
    pub fn discover() -> Vec<ClientLibrary> {
        discover_in(&DIRETORIOS_DE_BUSCA)
    }

    /// Valida que o arquivo é ELF e que sua arquitetura (`e_machine`) bate
    /// com a do processo atual, lendo apenas os 20 primeiros bytes.
    pub fn validate_arch(&self) -> Result<(), DomainError> {
        let mut cabecalho = [0u8; TAMANHO_CABECALHO_ELF];
        let mut arquivo = std::fs::File::open(&self.path)
            .map_err(|e| crate::erro_driver(format!("abrir '{}': {e}", self.path.display())))?;
        use std::io::Read;
        let lidos = arquivo
            .read(&mut cabecalho)
            .map_err(|e| crate::erro_driver(format!("ler '{}': {e}", self.path.display())))?;
        check_elf(&cabecalho[..lidos], &self.path)
    }
}

/// Varre `dirs` (sem recursão) por `libfbclient.so*`, em ordem determinística:
/// diretórios na ordem dada, entradas por nome. Deduplica pelo caminho
/// resolvido, preferindo a entrada com versão no nome.
fn discover_in<P: AsRef<Path>>(dirs: &[P]) -> Vec<ClientLibrary> {
    let mut achadas: Vec<(PathBuf, ClientLibrary)> = Vec::new();
    for dir in dirs {
        let Ok(entradas) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut entradas: Vec<_> = entradas.filter_map(Result::ok).collect();
        entradas.sort_by_key(|e| e.file_name());
        for entrada in entradas {
            let Some(nome) = entrada.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !e_candidata(&nome) {
                continue;
            }
            let path = entrada.path();
            // file_type() usa lstat (não segue symlink): aceita arquivo real ou
            // symlink, mesmo com alvo ilegível —.validate_arch()/connect() dão
            // o erro acionável depois (Spec 01, §3). Exclui apenas diretórios.
            let Ok(tipo) = entrada.file_type() else {
                continue;
            };
            if tipo.is_dir() {
                continue;
            }
            let resolvido = path.canonicalize().unwrap_or_else(|_| path.clone());
            let biblioteca = ClientLibrary {
                path,
                version: version_from_name(&nome),
            };
            match achadas.iter_mut().find(|(r, _)| *r == resolvido) {
                Some((_, existente)) => {
                    if existente.version.is_none() {
                        *existente = biblioteca;
                    }
                }
                None => achadas.push((resolvido, biblioteca)),
            }
        }
    }
    achadas
        .into_iter()
        .map(|(_, biblioteca)| biblioteca)
        .collect()
}

/// Indica se o nome de arquivo é `libfbclient.so` exato ou começa com
/// `libfbclient.so.` (ex.: `libfbclient.so.2.5.9`).
fn e_candidata(nome: &str) -> bool {
    nome == PREFIXO_LIBFBCLIENT || nome.starts_with(&format!("{PREFIXO_LIBFBCLIENT}."))
}

/// Extrai a versão do nome `libfbclient.so.X[.Y[.Z]]`; [`None`] para o nome
/// sem sufixo de versão ou com sufixo não interpretável.
fn version_from_name(nome: &str) -> Option<ServerVersion> {
    let resto = nome.strip_prefix(PREFIXO_LIBFBCLIENT)?.strip_prefix('.')?;
    if resto.is_empty() {
        return None;
    }
    ServerVersion::parse(resto)
}

/// Valida magic ELF e `e_machine` (offset 18, little endian) contra a
/// arquitetura do processo atual.
fn check_elf(cabecalho: &[u8], path: &Path) -> Result<(), DomainError> {
    if cabecalho.len() < TAMANHO_CABECALHO_ELF {
        return Err(crate::erro_driver(format!(
            "'{}' não é uma biblioteca ELF válida: apenas {} byte(s) lidos",
            path.display(),
            cabecalho.len()
        )));
    }
    if &cabecalho[..4] != b"\x7fELF" {
        return Err(crate::erro_driver(format!(
            "'{}' não é uma biblioteca ELF válida: magic inválido",
            path.display()
        )));
    }
    let e_machine = u16::from_le_bytes([cabecalho[18], cabecalho[19]]);
    let Some(esperado) = e_machine_da_arquitetura_atual() else {
        return Err(crate::erro_driver(format!(
            "arquitetura do processo '{}' não suportada na validação de ELF",
            std::env::consts::ARCH
        )));
    };
    if e_machine != esperado {
        return Err(crate::erro_driver(format!(
            "'{}' tem e_machine={e_machine}, incompatível com o processo {} (esperado e_machine={esperado})",
            path.display(),
            std::env::consts::ARCH
        )));
    }
    Ok(())
}

/// Valor ELF `e_machine` da arquitetura do processo atual, ou [`None`] se
/// desconhecida.
fn e_machine_da_arquitetura_atual() -> Option<u16> {
    match std::env::consts::ARCH {
        "x86_64" => Some(62),
        "aarch64" => Some(183),
        "x86" => Some(3),
        "arm" => Some(40),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diretório temporário único por teste (sem crates extras).
    fn dir_temp(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "firebird-adapter-library-{nome}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar diretório temporário de teste");
        dir
    }

    /// Cabeçalho ELF sintético de 20 bytes com o `e_machine` dado.
    fn elf_header_com_e_machine(e_machine: u16) -> [u8; TAMANHO_CABECALHO_ELF] {
        let mut bytes = [0u8; TAMANHO_CABECALHO_ELF];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2; // EI_CLASS: 64 bits
        bytes[5] = 1; // EI_DATA: little endian
        bytes[6] = 1; // EI_VERSION
        let machine = e_machine.to_le_bytes();
        bytes[18] = machine[0];
        bytes[19] = machine[1];
        bytes
    }

    #[test]
    fn version_do_nome_com_versao_completa() {
        let v = version_from_name("libfbclient.so.2.5.9").expect("2.5.9 deve ser interpretada");
        assert_eq!((v.major, v.minor, v.patch), (2, 5, Some(9)));
    }

    #[test]
    fn version_do_nome_sem_versao_retorna_none() {
        assert_eq!(version_from_name("libfbclient.so"), None);
    }

    #[test]
    fn version_do_nome_com_versao_tres_componentes() {
        let v = version_from_name("libfbclient.so.3.0.11").expect("3.0.11 deve ser interpretada");
        assert_eq!((v.major, v.minor, v.patch), (3, 0, Some(11)));
    }

    #[test]
    fn version_do_nome_sem_patch() {
        let v = version_from_name("libfbclient.so.2.5").expect("2.5 deve ser interpretada");
        assert_eq!((v.major, v.minor, v.patch), (2, 5, None));
    }

    #[test]
    fn discover_nao_duplica_symlink_e_ignora_irrelevantes() {
        let dir = dir_temp("symlink");
        std::fs::write(dir.join("libfbclient.so.2.5.9"), b"dummy").expect("escrever alvo");
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("libfbclient.so.2.5.9"), dir.join("libfbclient.so"))
            .expect("criar symlink");
        std::fs::create_dir(dir.join("libfbclient.so.4.0.0")).expect("criar subdiretório");
        std::fs::write(dir.join("liboutra.so.1.0"), b"dummy").expect("escrever irrelevante");

        let achadas = discover_in(&[dir.as_path()]);

        assert_eq!(
            achadas.len(),
            1,
            "symlink e alvo devem contar uma única vez: {achadas:?}"
        );
        let v = achadas[0]
            .version
            .expect("a entrada deve ter a versão do nome");
        assert_eq!((v.major, v.minor, v.patch), (2, 5, Some(9)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_diretorio_inexistente_retorna_vazio() {
        let achadas = discover_in(&["/diretorio/que/nao/existe"]);
        assert!(
            achadas.is_empty(),
            "diretório inexistente não pode derrubar a descoberta: {achadas:?}"
        );
    }

    #[test]
    fn validate_arch_aceita_elf_da_arquitetura_atual() {
        let dir = dir_temp("arch-ok");
        let path = dir.join("libfbclient.so.2.5.9");
        std::fs::write(&path, elf_header_com_e_machine(62)).expect("escrever fixture");

        let resultado = ClientLibrary {
            path,
            version: None,
        }
        .validate_arch();

        if std::env::consts::ARCH == "x86_64" {
            assert!(
                resultado.is_ok(),
                "e_machine=62 deve ser aceito em x86_64: {resultado:?}"
            );
        } else {
            assert!(resultado.is_err());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_arch_rejeita_arquitetura_incompativel() {
        let dir = dir_temp("arch-errado");
        let path = dir.join("libfbclient.so.3.0.11");
        std::fs::write(&path, elf_header_com_e_machine(40)).expect("escrever fixture");

        let resultado = ClientLibrary {
            path,
            version: None,
        }
        .validate_arch();

        assert!(resultado.is_err(), "e_machine=40 não deve ser aceito aqui");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_arch_rejeita_magic_invalido() {
        let dir = dir_temp("magic-errado");
        let path = dir.join("libfbclient.so.2.5.9");
        let mut bytes = elf_header_com_e_machine(62);
        bytes[..4].copy_from_slice(b"\x7fELG");
        std::fs::write(&path, bytes).expect("escrever fixture");

        let resultado = ClientLibrary {
            path,
            version: None,
        }
        .validate_arch();

        assert!(resultado.is_err(), "magic inválido deve ser rejeitado");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_arch_rejeita_arquivo_curto() {
        let dir = dir_temp("curto");
        let path = dir.join("libfbclient.so.2.5.9");
        std::fs::write(&path, b"\x7fELF").expect("escrever fixture");

        let resultado = ClientLibrary {
            path,
            version: None,
        }
        .validate_arch();

        assert!(
            resultado.is_err(),
            "arquivo menor que 20 bytes deve ser rejeitado"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
