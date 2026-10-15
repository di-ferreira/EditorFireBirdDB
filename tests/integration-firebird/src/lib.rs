//! Checklists de integração por versão do Firebird (Fase 0 — PoC, Task 4).
//!
//! Os testes vivem em `tests/poc_checklist.rs` e rodam apenas contra um
//! servidor real quando as env vars `FB_POC_HOST/PORT/DATABASE/USER/PASSWORD/
//! LIB_PATH` estão definidas (ambiente provido por `scripts/firebird-env.sh`
//! ou pelo servidor local). Sem env, são ignorados com aviso — nunca falham o
//! `cargo test` local.
