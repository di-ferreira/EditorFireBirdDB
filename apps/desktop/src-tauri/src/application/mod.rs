//! Camada de aplicação: orquestração de casos de uso, sem Tauri.
//!
//! Os serviços daqui recebem suas dependências por trait (repositório de
//! perfis, cofre de segredos, conector Firebird), o que permite testá-los sem
//! servidor, sem `libfbclient` e sem estado global. Os comandos em
//! [`crate::commands`] são casca fina sobre estes serviços.

pub mod connection_manager;
pub mod estado;

#[cfg(test)]
pub mod fakes;
