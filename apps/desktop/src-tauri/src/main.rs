//! Binário do Firebird Studio Linux.
//!
//! Toda a inicialização (Tauri builder, janela e comandos) vive em
//! [`firebird_studio_desktop::run`]; este binário apenas o aciona.

// Em release, suprime o console no Windows (a plataforma-alvo é Linux; a
// atribuição é mantida por compatibilidade com o template do Tauri).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    firebird_studio_desktop::run();
}
