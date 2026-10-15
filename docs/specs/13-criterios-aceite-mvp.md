# Spec 13 — Requisitos Rastreáveis e Critérios de Aceite do MVP

**Documento de origem:** Seções 14 e 15 da especificação técnica

`P0` = obrigatório para o MVP; `P1` = desejável se não comprometer as funções P0.

## 1. Matriz de requisitos funcionais

| ID | Requisito | Prioridade | Spec | Critério resumido |
|---|---|---|---|---|
| FR-001 | Gerenciar perfis | P0 | [03](03-gerenciador-conexoes.md) | Criar, editar, testar e excluir perfil sem senha em texto puro |
| FR-002 | Conectar ao Firebird | P0 | [03](03-gerenciador-conexoes.md) | Abrir sessão e expor versão/capacidades detectadas |
| FR-003 | DB Explorer | P0 | [04](04-db-explorer.md) | Listar tabelas/views e expandir campos |
| FR-004 | Metadados avançados | P0 | [04](04-db-explorer.md) | Mostrar PK/FK/índices/domínios quando suportados |
| FR-005 | SQL Editor | P0 | [05](05-sql-editor.md) | Editar, selecionar e executar instrução |
| FR-006 | Resultados | P0 | [06](06-grade-resultados.md) | Exibir colunas/linhas/NULL/erros/duração com paginação |
| FR-007 | Histórico SQL | P1 | [05](05-sql-editor.md) | Pesquisar e reabrir histórico local, com opção de desativar |
| FR-008 | Provedor local de IA | P0 | [07](07-assistente-ia.md) | Testar conexão e solicitar geração contextual |
| FR-009 | Provedor remoto de IA | P0 | [07](07-assistente-ia.md) | Configurar provedor e exigir política explícita de envio |
| FR-010 | Contexto do esquema | P0 | [07](07-assistente-ia.md) | Enviar apenas metadados relevantes selecionados |
| FR-011 | Pré-visualização de SQL | P0 | [07](07-assistente-ia.md) | SQL gerado nunca é executado automaticamente |
| FR-012 | Explicar SQL | P1 | [07](07-assistente-ia.md) | Explicação separada do código SQL |
| FR-013 | Corrigir SQL | P1 | [07](07-assistente-ia.md) | Usar erro retornado pelo servidor, mediante ação do usuário |
| FR-014 | Exportar resultados | P1 | [06](06-grade-resultados.md) | Exportar resultado limitado para CSV |
| FR-015 | Compatibilidade versionada | P0 | [01](01-compatibilidade-firebird.md) | Matriz homologada e adaptação de recursos por servidor |
| FR-016 | Logs seguros | P0 | [11](11-requisitos-nao-funcionais.md) | Redigir credenciais e tokens em logs/erros |

## 2. Critérios de aceite do MVP

O MVP pode ser considerado utilizável quando todos estes cenários forem verdadeiros:

1. O usuário instala o pacote em uma distribuição Linux homologada.
2. O usuário configura um perfil e seleciona uma biblioteca cliente Firebird válida.
3. A aplicação conecta a uma versão Firebird presente na matriz homologada e informa
   versão/capacidades.
4. O usuário navega por tabelas, campos, views e relacionamentos declarados como suportados.
5. O usuário executa um SELECT ou seleção SQL e recebe resultados sem bloquear a interface.
6. Erros de conexão/SQL são mostrados com contexto útil, sem credenciais.
7. O usuário configura um modelo local e gera SQL usando metadados da conexão ativa.
8. O usuário configura um provedor remoto e vê claramente o contexto que será enviado.
9. O SQL gerado aparece para revisão e precisa de ação explícita para ser inserido/executado.
10. Uma resposta da IA que use tabela ou campo inexistente gera aviso de referência não
    confirmada.
11. A aplicação continua utilizável quando o provedor de IA está indisponível.
12. Os testes automatizados e o relatório de compatibilidade do MVP estão disponíveis no
    repositório.
