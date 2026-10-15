# Spec 04 — DB Explorer

**Documento de origem:** Seção 6.2 da especificação técnica
**Requisitos:** FR-003 (DB Explorer, P0), FR-004 (Metadados avançados, P0)

## 1. Objetos mínimos

- Banco e informações básicas da conexão.
- Tabelas e views.
- Campos e seus tipos efetivos, incluindo domínio, nulabilidade, default e posição.
- Chaves primárias, estrangeiras e únicas.
- Índices.
- Domínios.
- Procedures e parâmetros, quando suportados no catálogo da versão.
- Triggers e generators/sequences.
- Dependências de objetos, quando disponíveis.

## 2. Comportamento

- Busca por nome com debounce.
- Expansão sob demanda, sem carregar todos os detalhes de todos os objetos na primeira
  conexão.
- Atualização manual de um nó ou de toda a árvore.
- Menu contextual: copiar nome qualificado, abrir definição ou inserir nome no editor.
- Identificação visual de objetos de sistema versus objetos do usuário.
- Metadados respeitando os privilégios do usuário conectado.

## 3. Regras

- Consultas ao catálogo ficam centralizadas em `MetadataRepository`.
- SQL de catálogo deve ter variantes compatíveis e testes por versão.
- Normalizar espaços de preenchimento de campos `CHAR` vindos de `RDB$`.
- Não depender exclusivamente da análise textual de definições para inferir relações;
  usar constraints/indicar quando a relação for apenas sugestão heurística.

## 4. Critérios de aceite

- [ ] Listar tabelas/views e expandir campos (FR-003).
- [ ] Mostrar PK/FK/índices/domínios quando suportados (FR-004).
- [ ] Primeiro nível de metadados carrega sem buscar detalhes de todos os objetos
      (meta de desempenho — ver [11-requisitos-nao-funcionais.md](11-requisitos-nao-funcionais.md)).
