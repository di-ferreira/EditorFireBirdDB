//! Build script do Firebird Studio Linux (Tauri 2).
//!
//! `tauri_build::build()` valida `tauri.conf.json`, gera as capabilities em
//! `gen/schemas/` e registra o manifesto de comandos do app.
fn main() {
    tauri_build::build();
}
