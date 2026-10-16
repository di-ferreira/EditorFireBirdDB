# Plano — Fase 3: IA contextual (decomposição em passos)

**Origem:** `docs/specs/14-plano-entrega.md` (Fase 3) e `docs/specs/07-assistente-ia.md`,
`docs/specs/08-segredos-e-privacidade.md`.

A Fase 3 é dividida em **4 passos** entregáveis. Cada passo fecha com verificação padrão
(`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --all --check`, `pnpm build`) e um commit único.

## Passo 1 — Fundação do provedor de IA (interface, config e segredos)

- Domínio (`firebird-domain::ai`): `TipoProvedorIa`, `ProvedorIaConfig`, `PedidoIa`,
  `RespostaIa` e `ErroIa` (sem segredo em `Debug`/`Display`).
- Persistência: repositório da tabela `ai_provider_profiles` (Spec 09), com
  `secret_ref` — **nenhuma chave de API em coluna, log, erro ou DTO**.
- Cofre: a chave do provedor é gravada no `SecretStore` do sistema; o perfil guarda
  só o `secret_ref`.
- Aplicação: trait `ProvedorIa` (`testar`, `gerar`) e implementação
  `ProvedorOpenAiCompativel` (cobre Ollama, LM Studio e APIs OpenAI-compatíveis),
  atrás de um `TransporteHttp` injetável — assim os testes rodam **sem rede**.
- Comandos Tauri: `ia_provider_list`, `ia_provider_save`, `ia_provider_delete`,
  `ia_provider_test`.
- UI `features/ai-assistant`: aba de provedores (listar, criar, testar) com estados e
  pt-BR.

**Fora do escopo do passo:** contexto do banco, templates, revisão de SQL, risco e
consentimento de envio remoto (passos 2–4).

## Passo 2 — Contexto do banco e templates versionados

- `ContextoSchema`: tabelas/views/colunas/PK/FK do perfil ativo, com limite de
  tamanho e **sem valores de dados**.
- Templates versionados (gerar, explicar, corrigir, revisar) em `docs/specs` +
  código, com `versao` explícita e prompt redigido.
- Comando de pré-visualização do contexto antes de qualquer envio.

## Passo 3 — Geração, explicação e correção + histórico

- `ia_generate`, `ia_explain`, `ia_fix` reaproveitando o Passo 1.
- Registro em `ai_interactions` (prompt, contexto resumido, resposta, SQL aceito).
- UI: painel de conversa por tarefa, com diff do SQL sugerido e **nunca execução
  automática**.

## Passo 4 — Revisão de SQL, classificação de risco e consentimento

- Classificador conservador (`SELECT` → seguro; DML/DDL → exige confirmação).
- Consentimento explícito antes de enviar contexto a provedor **remoto**, com o
  botão `remote_context_confirmation` do perfil respeitado.
- Explicação de risco no SQL inserido no editor, sempre como sugestão.

## Regras que valem em todos os passos

1. SQL gerado por IA **nunca** é executado automaticamente.
2. Nenhum segredo em log, erro, DTO, histórico ou `ai_interactions`.
3. Sem chamada de rede em teste: o `TransporteHttp` é sempre dobrado nos testes.
