# Spec 14 — Plano de Entrega

**Documento de origem:** Seção 16 da especificação técnica

## Fase 0 — Prova de conceito técnica

- Criar app mínimo Tauri 2 + React + Rust.
- Carregar `libfbclient.so` de forma controlada ou testar a alternativa selecionada.
- Conectar a ambientes de Firebird 2.0, 2.1, 2.5 e a pelo menos uma versão moderna.
- Consultar versão do servidor, `RDB$RELATIONS`, `RDB$RELATION_FIELDS` e executar SELECT.
- Validar Unicode, dialeto, parâmetros, tipos numéricos e mensagens de erro.
- Escrever ADR com a decisão de driver e as combinações homologadas (ADR-002).

**Saída obrigatória:** relatório de prova de conceito, inclusive falhas e limitações. Se
2.0 não funcionar nas condições Linux alvo, não ocultar o problema: identificar
biblioteca/sistema exigidos e decidir se o compromisso é por conexão remota, sistema
suportado ou imagem compatível.

## Fase 1 — Fundação

- Estrutura de repositório e pipeline CI.
- Tipos de domínio e erros.
- Persistência SQLite e migrações.
- Cofre de credenciais.
- Gerenciador de conexões e status de sessão.

## Fase 2 — DB Explorer e SQL Editor

- Catálogo básico, árvore sob demanda e refresh.
- Editor CodeMirror, abas e atalhos.
- Execução, resultados paginados e tratamento de erros.
- Histórico e scripts salvos.

## Fase 3 — IA contextual

- Interface de provedores local/remoto.
- Teste de configuração e gerenciamento seguro de secrets.
- Context builder, templates versionados, geração, explicação e correção.
- Revisão de SQL, classificação conservadora de risco e consentimento de envio remoto.

## Fase 4 — Estabilização e distribuição

- Testes de integração na matriz Firebird.
- Testes E2E e acessibilidade.
- Pacote `.deb` como entrega principal inicial; avaliar AppImage/RPM após validação de
  dependências.
- Publicar guia de instalação, matriz de compatibilidade, SBOM, changelog e política de
  segurança.

## Fase 5 — Pós-MVP

- Exportação avançada, editor de dados e geração de DDL.
- Backup/restore e estatísticas.
- Editor de procedures/triggers e análise de dependências.
- Modelador ERD e comparação de estruturas.
- Depuração e ferramentas administrativas avançadas somente após estudo específico.

## Próxima ação recomendada (Seção 20)

Antes de construir toda a interface, implementar um spike isolado com Rust que carregue a
biblioteca cliente, conecte a uma base Firebird 2.0 real ou ambiente reprodutível, leia
metadados e execute uma consulta parametrizada. Repetir para 2.1, 2.5 e uma versão
moderna. O resultado do spike determina se o adaptador será baseado em `libfbclient.so`,
em uma implementação alternativa ou em mais de uma estratégia.
