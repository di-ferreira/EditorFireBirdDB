//! Binário do Firebird Studio Linux.
//!
//! Toda a inicialização (Tauri builder, janela e comandos) vive em
//! [`firebird_studio_desktop::run`]; este binário apenas o aciona.
//!
//! Plataforma-alvo: Linux (Spec 01 §1) — sem atributos de subsistema
//! específicos de outra plataforma.

fn main() {
    firebird_studio_desktop::run();
}
