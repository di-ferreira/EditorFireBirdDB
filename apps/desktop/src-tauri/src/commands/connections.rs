//! Fronteira Tauri de conexões (Spec 03; Spec 10 §3).
//!
//! Cada comando tem duas camadas: a função `#[tauri::command]`, fina e sem
//! lógica (recebe o [`tauri::State`] e delega), e a função de aplicação, que
//! recebe `&AppState` e é testada **sem Tauri**. É essa segunda camada que
//! guarda as regras: exigir armazenamento, exigir cofre, converter DTO.
//!
//! Contrato com a UI (`apps/desktop/src/features/connections/api.ts`):
//! `connection_list_profiles`, `connection_save_profile`,
//! `connection_test_profile`, `connection_open`, `connection_close` e
//! `connection_get_status`. Erros são rejeições do `invoke` no formato
//! `{ code, message }` — nunca panic.

use tauri::State;

use crate::application::estado::AppState;
use crate::commands::dto::{
    CloseConnectionResult, ConnectionProfileDto, ConnectionProfileInputDto, ConnectionStatusInfo,
    OpenConnectionResult, TestConnectionResult,
};
use crate::commands::error::CommandError;

/// Senha recebida por `connection_open` — único ponto do backend que recebe
/// segredo da UI.
///
/// O `Debug` é manual e redigido: um `log!("{args:?}")`, um assert mal
/// colocado ou um panic não pode levar a senha (FR-016). O conteúdo vive em
/// [`SegredoOpcional::expor`], chamado uma única vez, pelo gerenciador.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct SegredoOpcional(Option<String>);

impl SegredoOpcional {
    /// Envolve uma senha vinda da UI.
    pub fn novo(segredo: Option<String>) -> Self {
        Self(segredo)
    }

    /// Senha como texto, para o gerenciador gravar no cofre.
    pub fn expor(&self) -> Option<&str> {
        self.0.as_deref()
    }

    /// `true` quando nenhum segredo foi informado.
    pub fn vazio(&self) -> bool {
        self.0.as_deref().is_none_or(str::is_empty)
    }
}

impl std::fmt::Debug for SegredoOpcional {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.vazio() {
            "SegredoOpcional(nenhum)"
        } else {
            "SegredoOpcional(***)"
        })
    }
}

/// Lista todos os perfis salvos.
#[tauri::command]
pub fn connection_list_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<ConnectionProfileDto>, CommandError> {
    listar_perfis(&state)
}

/// Cria ou atualiza um perfil. O payload **nunca** traz senha.
#[tauri::command]
pub fn connection_save_profile(
    profile: ConnectionProfileInputDto,
    state: State<'_, AppState>,
) -> Result<ConnectionProfileDto, CommandError> {
    salvar_perfil(&state, &profile)
}

/// Testa a conectividade **sem registrar o perfil** (Spec 03 §1).
#[tauri::command]
pub fn connection_test_profile(
    profile: ConnectionProfileInputDto,
    state: State<'_, AppState>,
) -> Result<TestConnectionResult, CommandError> {
    testar_perfil(&state, &profile)
}

/// Abre uma sessão: único comando que recebe a senha.
#[tauri::command]
pub fn connection_open(
    profile_id: String,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<OpenConnectionResult, CommandError> {
    abrir_conexao(&state, &profile_id, SegredoOpcional::novo(password))
}

/// Encerra a sessão do perfil (idempotente).
#[tauri::command]
pub fn connection_close(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<CloseConnectionResult, CommandError> {
    fechar_conexao(&state, &profile_id)
}

/// Situação atual da sessão do perfil.
#[tauri::command]
pub fn connection_get_status(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<ConnectionStatusInfo, CommandError> {
    consultar_status(&state, &profile_id)
}

/// Aplicação de `connection_list_profiles`.
pub fn listar_perfis(state: &AppState) -> Result<Vec<ConnectionProfileDto>, CommandError> {
    state.exigir_armazenamento()?;
    let perfis = state.manager().listar()?;
    Ok(perfis.iter().map(ConnectionProfileDto::from).collect())
}

/// Aplicação de `connection_save_profile`.
pub fn salvar_perfil(
    state: &AppState,
    entrada: &ConnectionProfileInputDto,
) -> Result<ConnectionProfileDto, CommandError> {
    state.exigir_armazenamento()?;
    let dados = entrada.para_dados()?;
    let perfil = state.manager().salvar(dados)?;
    Ok(ConnectionProfileDto::from(&perfil))
}

/// Aplicação de `connection_test_profile`.
///
/// Falha de conexão **não** vira erro de comando: volta
/// `TestConnectionResult { ok: false, message }`, para que a UI exiba o
/// motivo sem tratar tudo como falha do comando (Spec 03 §2).
pub fn testar_perfil(
    state: &AppState,
    entrada: &ConnectionProfileInputDto,
) -> Result<TestConnectionResult, CommandError> {
    let dados = entrada.para_dados()?;
    // O teste não persiste e, quando o perfil já existe, usa a senha do
    // cofre — por isso exige o cofre disponível.
    if !dados.id.trim().is_empty() {
        state.exigir_cofre()?;
    }
    let resultado = state.manager().testar(dados, None)?;
    Ok(TestConnectionResult::from(&resultado))
}

/// Aplicação de `connection_open`.
pub fn abrir_conexao(
    state: &AppState,
    profile_id: &str,
    senha: SegredoOpcional,
) -> Result<OpenConnectionResult, CommandError> {
    state.exigir_armazenamento()?;
    state.exigir_cofre()?;
    let sessao = state.manager().abrir(profile_id, senha.expor())?;
    Ok(OpenConnectionResult::from(&sessao))
}

/// Aplicação de `connection_close`.
pub fn fechar_conexao(
    state: &AppState,
    profile_id: &str,
) -> Result<CloseConnectionResult, CommandError> {
    state.exigir_armazenamento()?;
    let status = state.manager().fechar(profile_id)?;
    Ok(CloseConnectionResult {
        profile_id: profile_id.to_string(),
        status: super::dto::StatusDto::from(&status),
    })
}

/// Aplicação de `connection_get_status`.
pub fn consultar_status(
    state: &AppState,
    profile_id: &str,
) -> Result<ConnectionStatusInfo, CommandError> {
    state.exigir_armazenamento()?;
    let info = state.manager().status(profile_id)?;
    Ok(ConnectionStatusInfo::from(&info))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{biblioteca_cliente_falsa, ConectorFalso};
    use crate::commands::dto::StatusDto;
    use crate::commands::error::CommandErrorCode;
    use crate::security::CofreIndisponivel;
    use crate::storage::BancoLocal;
    use firebird_security::{secret_ref, InMemorySecretStore, SecretStore};
    use firebird_storage::ConnectionProfileRepository;
    use std::sync::Arc;

    const SENHA: &str = "Abc@123-secreta";

    fn estado(
        conector: Arc<ConectorFalso>,
    ) -> (AppState, Arc<BancoLocal>, Arc<InMemorySecretStore>) {
        let banco = Arc::new(BancoLocal::em_memoria().expect("banco em memória"));
        let cofre = Arc::new(InMemorySecretStore::new());
        let state = AppState::com_conector(banco.clone(), cofre.clone(), true, conector);
        (state, banco, cofre)
    }

    fn entrada(id: Option<&str>) -> ConnectionProfileInputDto {
        ConnectionProfileInputDto {
            id: id.map(str::to_string),
            name: "Produção".to_string(),
            host: "servidor.local".to_string(),
            port: 3050,
            database_path: "/dados/app.fdb".to_string(),
            username: "SYSDBA".to_string(),
            role_name: None,
            charset: "UTF8".to_string(),
            sql_dialect: Some(3),
            client_library_path: Some(biblioteca_cliente_falsa().display().to_string()),
            connect_timeout_ms: Some(5_000),
        }
    }

    fn perfil_criado(state: &AppState) -> ConnectionProfileDto {
        salvar_perfil(state, &entrada(None)).expect("salvando perfil")
    }

    #[test]
    fn salvar_e_listar_perfis_pelo_ciclo_do_comando() {
        let (state, banco, _cofre) = estado(ConectorFalso::ok());

        let dto = perfil_criado(&state);
        assert!(!dto.id.is_empty());
        assert_eq!(dto.port, 3050);
        assert_eq!(dto.sql_dialect, Some(3));
        assert_eq!(
            banco
                .get(&dto.id)
                .expect("buscando")
                .expect("perfil")
                .secret_ref,
            Some(secret_ref(&dto.id))
        );

        let listados = listar_perfis(&state).expect("listando");
        assert_eq!(listados.len(), 1);
        assert_eq!(listados[0].id, dto.id);
    }

    #[test]
    fn salvar_edicao_usa_o_mesmo_id() {
        let (state, _banco, _cofre) = estado(ConectorFalso::ok());
        let dto = perfil_criado(&state);

        let editado = salvar_perfil(
            &state,
            &ConnectionProfileInputDto {
                name: "Renomeado".to_string(),
                ..entrada(Some(&dto.id))
            },
        )
        .expect("editando");

        assert_eq!(editado.id, dto.id);
        assert_eq!(editado.name, "Renomeado");
        assert_eq!(listar_perfis(&state).expect("listando").len(), 1);
    }

    #[test]
    fn salvar_com_id_inexistente_devolve_erro_de_conexao() {
        let (state, banco, _cofre) = estado(ConectorFalso::ok());
        let erro = salvar_perfil(&state, &entrada(Some("fantasma"))).expect_err("id inexistente");
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(banco.list().expect("listando").is_empty());
    }

    #[test]
    fn salvar_com_porta_invalida_devolve_erro_sem_perseguir() {
        let (state, banco, _cofre) = estado(ConectorFalso::ok());
        let erro = salvar_perfil(
            &state,
            &ConnectionProfileInputDto {
                port: 0,
                ..entrada(None)
            },
        )
        .expect_err("porta zero");
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(erro.message.contains("port"), "{}", erro.message);
        assert!(banco.list().expect("listando").is_empty());
    }

    #[test]
    fn abrir_devolve_sessao_com_biblioteca_e_connected() {
        let conector = ConectorFalso::ok();
        let (state, _banco, cofre) = estado(conector.clone());

        let dto = perfil_criado(&state);
        let aberto = abrir_conexao(
            &state,
            &dto.id,
            SegredoOpcional::novo(Some(SENHA.to_string())),
        )
        .expect("abrindo conexão");

        assert_eq!(aberto.profile_id, dto.id);
        assert!(!aberto.session_id.is_empty());
        assert_eq!(aberto.status, StatusDto::Connected);
        assert_eq!(
            aberto.client_library_path.as_deref(),
            Some(biblioteca_cliente_falsa().to_str().expect("caminho utf-8"))
        );
        assert_eq!(
            cofre.get(&secret_ref(&dto.id)).expect("lendo cofre"),
            Some(SENHA.to_string()),
            "a senha foi para o cofre, não para o DTO"
        );
        assert_eq!(conector.recebidos()[0].password, SENHA);
    }

    #[test]
    fn abrir_com_perfil_inexistente_devolve_erro_tipado() {
        let (state, _banco, _cofre) = estado(ConectorFalso::ok());
        let erro = abrir_conexao(
            &state,
            "fantasma",
            SegredoOpcional::novo(Some(SENHA.to_string())),
        )
        .expect_err("perfil inexistente");
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(erro.message.contains("fantasma"));
    }

    #[test]
    fn abrir_sem_senha_e_sem_segredo_no_cofre_explica_o_que_fazer() {
        let (state, _banco, _cofre) = estado(ConectorFalso::ok());
        let dto = perfil_criado(&state);

        let erro =
            abrir_conexao(&state, &dto.id, SegredoOpcional::novo(None)).expect_err("sem senha");
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(
            erro.message.contains("informe-a ao conectar"),
            "mensagem acionável: {}",
            erro.message
        );
    }

    #[test]
    fn abrir_com_senha_redigida_nao_vaza_em_erro_do_driver() {
        let conector = ConectorFalso::falhando("login failed for password=Abc@123");
        let (state, _banco, _cofre) = estado(conector);

        let dto = perfil_criado(&state);
        let erro = abrir_conexao(
            &state,
            &dto.id,
            SegredoOpcional::novo(Some(SENHA.to_string())),
        )
        .expect_err("conexão falha");

        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(
            !erro.message.contains(SENHA) && !erro.message.contains("Abc@123"),
            "a senha não pode vazar: {}",
            erro.message
        );
        assert!(erro.message.contains("password=***"), "{}", erro.message);

        let status = consultar_status(&state, &dto.id).expect("status");
        assert_eq!(status.status, StatusDto::Error);
        assert!(!status.message.expect("mensagem").contains("Abc@123"));
    }

    #[test]
    fn cofre_indisponivel_falha_open_com_permission_denied_e_mensagem_acionavel() {
        let banco = Arc::new(BancoLocal::em_memoria().expect("banco em memória"));
        let state = AppState::com_conector(
            banco.clone(),
            Arc::new(CofreIndisponivel::novo(
                "nenhum serviço de segredo disponível nesta sessão",
            )),
            false,
            ConectorFalso::ok(),
        );
        let dto = perfil_criado(&state);

        let erro = abrir_conexao(
            &state,
            &dto.id,
            SegredoOpcional::novo(Some(SENHA.to_string())),
        )
        .expect_err("sem cofre");

        assert_eq!(erro.code, CommandErrorCode::PermissionDenied);
        assert!(!erro.message.contains(SENHA), "vazou: {}", erro.message);
    }

    #[test]
    fn close_e_idempotento_e_get_status_muda_para_disconnected() {
        let (state, _banco, _cofre) = estado(ConectorFalso::ok());
        let dto = perfil_criado(&state);
        abrir_conexao(
            &state,
            &dto.id,
            SegredoOpcional::novo(Some(SENHA.to_string())),
        )
        .expect("abrindo");

        let status = consultar_status(&state, &dto.id).expect("status");
        assert_eq!(status.status, StatusDto::Connected);
        assert!(status.session_id.is_some());

        let fechado = fechar_conexao(&state, &dto.id).expect("fechando");
        assert_eq!(fechado.profile_id, dto.id);
        assert_eq!(fechado.status, StatusDto::Disconnected);
        assert_eq!(
            fechar_conexao(&state, &dto.id)
                .expect("fechar de novo")
                .status,
            StatusDto::Disconnected
        );

        let status = consultar_status(&state, &dto.id).expect("status");
        assert_eq!(status.status, StatusDto::Disconnected);
        assert_eq!(status.session_id, None);
    }

    #[test]
    fn get_status_de_perfil_desconhecido_devolve_erro() {
        let (state, _banco, _cofre) = estado(ConectorFalso::ok());
        let erro = consultar_status(&state, "fantasma").expect_err("perfil inexistente");
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
    }

    #[test]
    fn test_perfil_nao_persiste_e_nao_guarda_senha() {
        let conector = ConectorFalso::ok();
        let (state, banco, cofre) = estado(conector.clone());

        let resultado = testar_perfil(&state, &entrada(None)).expect("testando");

        assert!(resultado.ok);
        assert_eq!(resultado.server_version.as_deref(), Some("4.0.1"));
        assert!(banco.list().expect("listando").is_empty(), "não persiste");
        assert!(cofre.is_empty(), "não grava segredo");
        assert_eq!(conector.recebidos().len(), 1);
    }

    #[test]
    fn test_perfil_com_falha_devolve_ok_falso_e_nao_rejeita() {
        let (state, _banco, _cofre) = estado(ConectorFalso::falhando(
            "connection refused (password=S3nh4!)",
        ));

        let resultado = testar_perfil(&state, &entrada(None)).expect("falha não rejeita");

        assert!(!resultado.ok);
        assert!(resultado.message.is_some());
        assert!(
            !resultado.message.expect("mensagem").contains("S3nh4!"),
            "senha não pode vazar"
        );
    }

    #[test]
    fn test_perfil_de_servidor_sem_libfbclient_devolve_ok_falso_com_orientacao() {
        // Sem biblioteca o erro precisa ser acionável, não um crash.
        let (state, _banco, _cofre) = estado(ConectorFalso::ok());
        let resultado = testar_perfil(
            &state,
            &ConnectionProfileInputDto {
                client_library_path: Some("/nao/existe/libfbclient.so.9".to_string()),
                ..entrada(None)
            },
        )
        .expect("falha de biblioteca não rejeita");

        assert!(!resultado.ok);
        assert!(resultado.message.expect("mensagem").contains("libfbclient"));
    }

    #[test]
    fn test_perfil_de_perfil_salvo_usa_a_senha_do_cofre() {
        let conector = ConectorFalso::ok();
        let (state, _banco, cofre) = estado(conector.clone());
        let dto = perfil_criado(&state);
        cofre.set(&secret_ref(&dto.id), SENHA).expect("gravando");

        let resultado = testar_perfil(&state, &entrada(Some(&dto.id))).expect("testando");

        assert!(resultado.ok);
        assert_eq!(conector.recebidos()[0].password, SENHA);
    }

    #[test]
    fn armazenamento_degradado_falha_os_comandos_de_perfil_com_orientacao() {
        let state = AppState::montar(Some(
            &std::env::temp_dir()
                .join("firebird-studio-degradado")
                .join("inexistente/studio.db"),
        ));
        let erro = listar_perfis(&state).expect_err("armazenamento degradado");
        assert_eq!(erro.code, CommandErrorCode::ConnectionError);
        assert!(
            erro.message.contains("perfis não serão salvos"),
            "{}",
            erro.message
        );
    }

    #[test]
    fn segredo_opcional_redige_no_debug() {
        let com = SegredoOpcional::novo(Some(SENHA.to_string()));
        let sem = SegredoOpcional::novo(None);

        assert!(!format!("{com:?}").contains(SENHA), "senha no Debug");
        assert_eq!(format!("{com:?}"), "SegredoOpcional(***)");
        assert_eq!(format!("{sem:?}"), "SegredoOpcional(nenhum)");
        assert_eq!(com.expor(), Some(SENHA));
        assert!(sem.vazio());
        assert!(SegredoOpcional::novo(Some(String::new())).vazio());
    }

    #[test]
    fn segredo_opcional_desserializa_de_option_string() {
        let com: SegredoOpcional = serde_json::from_str("\"Abc@123\"").expect("lendo");
        assert_eq!(com.expor(), Some("Abc@123"));
        let sem: SegredoOpcional = serde_json::from_str("null").expect("lendo");
        assert!(sem.vazio());
    }

    #[test]
    fn os_seis_comandos_de_conexao_estao_no_contrato_do_frontend() {
        // Trava de contrato: os nomes que a UI invoca (`api.ts`) precisam
        // estar na lista mantida junto ao `generate_handler!` de `lib.rs`.
        assert_eq!(
            crate::NOMES_DOS_COMANDOS_DE_CONEXAO,
            [
                "connection_list_profiles",
                "connection_save_profile",
                "connection_test_profile",
                "connection_open",
                "connection_close",
                "connection_get_status",
            ]
        );
    }

    #[test]
    fn conector_falso_nao_chega_ao_servidor() {
        // Guarda de teste: o duplo nunca é o conector real.
        let falso = ConectorFalso::ok();
        assert!(
            falso.ligando(),
            "o duplo de teste deve simular conexão bem-sucedida"
        );
    }
}
