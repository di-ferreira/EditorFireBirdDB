//! Dublês de teste do adaptador Firebird (sem servidor, sem rede, sem cofre).
//!
//! Vivem em `#[cfg(test)]` e são compartilhados pelos testes da camada de
//! aplicação e dos comandos Tauri: o [`ConectorFalso`] devolve um
//! [`AdaptadorDeTeste`] sintético ou um erro do driver, e registra os
//! [`ConnParams`] recebidos — cujo `Debug` já redige a senha (FR-016).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use firebird_adapter::{ConnParams, FirebirdAdapter};
use firebird_domain::{DomainError, ServerVersion};

use crate::application::connection_manager::{AdaptadorAberto, Connector};

/// Adaptador que responde tudo localmente, sem tocar o servidor.
pub struct AdaptadorDeTeste {
    versao: ServerVersion,
}

impl AdaptadorDeTeste {
    /// Adaptador que informa `versao` em `server_version`.
    pub fn com_versao(versao: ServerVersion) -> Self {
        Self { versao }
    }
}

impl FirebirdAdapter for AdaptadorDeTeste {
    fn server_version(&mut self) -> Result<ServerVersion, DomainError> {
        Ok(self.versao)
    }

    fn query_rows(&mut self, _sql: &str) -> Result<Vec<Vec<Option<String>>>, DomainError> {
        Ok(vec![vec![Some("LINHA".to_string())]])
    }

    fn query_rows_params(
        &mut self,
        _sql: &str,
        _params: &[Option<&str>],
    ) -> Result<Vec<Vec<Option<String>>>, DomainError> {
        Ok(Vec::new())
    }

    fn execute(&mut self, _sql: &str, _params: &[Option<&str>]) -> Result<usize, DomainError> {
        Ok(0)
    }
}

/// Conector programável: sucesso com versão fixa, ou falha com a mensagem
/// informada (útil para provar redação de segredo no caminho de erro).
#[derive(Debug)]
pub struct ConectorFalso {
    falha: Mutex<Option<String>>,
    recebidos: Mutex<Vec<ConnParams>>,
    versao: Mutex<ServerVersion>,
}

impl ConectorFalso {
    /// Conector que sempre conecta, informando Firebird 4.0.1.
    pub fn ok() -> Arc<Self> {
        Arc::new(Self {
            falha: Mutex::new(None),
            recebidos: Mutex::new(Vec::new()),
            versao: Mutex::new(ServerVersion {
                major: 4,
                minor: 0,
                patch: Some(1),
            }),
        })
    }

    /// Conector que sempre falha com `mensagem` (do driver).
    pub fn falhando(mensagem: &str) -> Arc<Self> {
        Arc::new(Self {
            falha: Mutex::new(Some(mensagem.to_string())),
            recebidos: Mutex::new(Vec::new()),
            versao: Mutex::new(ServerVersion {
                major: 3,
                minor: 0,
                patch: None,
            }),
        })
    }

    /// Todos os parâmetros recebidos, na ordem das chamadas.
    pub fn recebidos(&self) -> Vec<ConnParams> {
        lock(&self.recebidos).clone()
    }

    /// `true` quando este conector deve conectar.
    pub fn ligando(&self) -> bool {
        lock(&self.falha).is_none()
    }
}

/// Trava um `Mutex` de duplo ignorando envenenamento.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Connector for ConectorFalso {
    fn connect(&self, params: &ConnParams) -> Result<AdaptadorAberto, DomainError> {
        lock(&self.recebidos).push(params.clone());
        match lock(&self.falha).clone() {
            Some(mensagem) => Err(DomainError::Driver { message: mensagem }),
            None => {
                let versao = *lock(&self.versao);
                Ok(Box::new(AdaptadorDeTeste::com_versao(versao)))
            }
        }
    }
}

/// Caminho de uma `libfbclient.so` sintética com cabeçalho ELF válido.
///
/// Os testes de conexão precisam passar pela **validação real** de biblioteca
/// (magic ELF + `e_machine` da arquitetura do processo) sem depender do
/// Firebird instalado na máquina de CI.
pub fn biblioteca_cliente_falsa() -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("firebird-studio-lib-falsa-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("criando diretório temporário");
    let caminho = dir.join("libfbclient.so.3.0.11");
    if !caminho.exists() {
        let mut cabecalho = [0u8; 20];
        cabecalho[..4].copy_from_slice(b"\x7fELF");
        cabecalho[4] = 2; // ELFCLASS64
        cabecalho[18..20].copy_from_slice(&e_machine_atual().to_le_bytes());
        std::fs::write(&caminho, cabecalho).expect("escrevendo biblioteca sintética");
    }
    caminho
}

/// `e_machine` ELF correspondente à arquitetura do processo de teste.
fn e_machine_atual() -> u16 {
    match std::env::consts::ARCH {
        "aarch64" => 183,
        "x86" => 3,
        "arm" => 40,
        _ => 62,
    }
}
