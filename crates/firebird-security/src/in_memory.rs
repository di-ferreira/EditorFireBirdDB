//! Cofre de teste/development em memória.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::error::SecurityError;
use crate::secret_store::SecretStore;

/// Cofre de segredos mantido em RAM, para testes e desenvolvimento.
///
/// Nunca use em produção: o processo perde tudo ao fechar e a senha fica na
/// memória do processo. O `Debug` e o `Display` são manuais e redigidos, de
/// modo que nem um `unwrap()` em teste exponha o segredo (FR-016).
pub struct InMemorySecretStore {
    entradas: Mutex<BTreeMap<String, String>>,
}

impl InMemorySecretStore {
    /// Cria um cofre vazio.
    pub fn new() -> Self {
        Self {
            entradas: Mutex::new(BTreeMap::new()),
        }
    }

    /// Número de segredos guardados (útil em asserções de teste).
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// `true` quando nenhum segredo está guardado.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Trava o mapa ignorando envenenamento: um panic em outra thread não
    /// pode transformar leitura de cofre de teste em panic aqui.
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, String>> {
        self.entradas.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for InMemorySecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for InMemorySecretStore {
    fn set(&self, ref_: &str, secret: &str) -> Result<(), SecurityError> {
        self.lock().insert(ref_.to_string(), secret.to_string());
        Ok(())
    }

    fn get(&self, ref_: &str) -> Result<Option<String>, SecurityError> {
        Ok(self.lock().get(ref_).cloned())
    }

    fn delete(&self, ref_: &str) -> Result<(), SecurityError> {
        self.lock().remove(ref_);
        Ok(())
    }
}

impl std::fmt::Debug for InMemorySecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemorySecretStore")
            .field("entradas", &self.len())
            .finish()
    }
}

impl std::fmt::Display for InMemorySecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InMemorySecretStore({} segredos)", self.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const REF_: &str = "firebird-studio/profile/perfil-1";

    #[test]
    fn set_agora_get_devolve_o_segredo() {
        let store = InMemorySecretStore::new();
        store.set(REF_, "Abc@123").expect("set falhou");
        assert_eq!(store.get(REF_).expect("get falhou"), Some("Abc@123".into()));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn get_de_referencia_inexistente_devolve_none() {
        let store = InMemorySecretStore::new();
        assert_eq!(store.get("firebird-studio/profile/nao-existe"), Ok(None));
        assert!(store.is_empty());
    }

    #[test]
    fn set_sobrescreve_o_segredo_anterior() {
        let store = InMemorySecretStore::new();
        store.set(REF_, "antiga").expect("set falhou");
        store.set(REF_, "nova").expect("set falhou");
        assert_eq!(store.get(REF_).expect("get falhou"), Some("nova".into()));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn delete_remove_o_segredo() {
        let store = InMemorySecretStore::new();
        store.set(REF_, "Abc@123").expect("set falhou");
        store.delete(REF_).expect("delete falhou");
        assert_eq!(store.get(REF_).expect("get falhou"), None);
        assert!(store.is_empty());
    }

    #[test]
    fn delete_de_referencia_inexistente_e_idempotente() {
        let store = InMemorySecretStore::new();
        assert!(store.delete(REF_).is_ok());
        assert!(store.delete(REF_).is_ok());
    }

    #[test]
    fn debug_nao_exibe_o_segredo_guardado() {
        let store = InMemorySecretStore::new();
        store.set(REF_, "Abc@123").expect("set falhou");
        let texto = format!("{store:?}");
        assert!(
            !texto.contains("Abc@123"),
            "segredo vazou no Debug: {texto}"
        );
        assert!(
            !texto.contains(REF_),
            "a referência também não deve vazar: {texto}"
        );
        assert!(texto.contains("InMemorySecretStore"), "texto: {texto}");
    }

    #[test]
    fn display_nao_exibe_o_segredo_guardado() {
        let store = InMemorySecretStore::new();
        store.set(REF_, "Abc@123").expect("set falhou");
        let texto = store.to_string();
        assert!(
            !texto.contains("Abc@123"),
            "segredo vazou no Display: {texto}"
        );
        assert!(texto.contains('1'), "o Display mostra a contagem: {texto}");
    }

    #[test]
    fn store_e_compartilhavel_entre_threads() {
        let store = Arc::new(InMemorySecretStore::new());
        let inicio = std::time::Instant::now();
        let mut handles = Vec::new();
        for i in 0..8 {
            let store = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                let ref_ = format!("firebird-studio/profile/perfil-{i}");
                store.set(&ref_, &format!("senha-{i}")).expect("set falhou");
                store.get(&ref_).expect("get falhou")
            }));
        }
        for (i, handle) in handles.into_iter().enumerate() {
            assert_eq!(
                handle.join().expect("thread panic"),
                Some(format!("senha-{i}"))
            );
        }
        assert_eq!(store.len(), 8);
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(5),
            "a serialização por Mutex não pode travar as threads"
        );
    }
}
