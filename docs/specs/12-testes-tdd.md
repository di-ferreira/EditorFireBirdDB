# Spec 12 — Testes e TDD

**Documento de origem:** Seção 12 da especificação técnica

O desenvolvimento seguirá TDD nos componentes de domínio e serviços: primeiro escrever
teste para o comportamento esperado, implementar o mínimo necessário e refatorar com a
suíte verde.

## 1. Pirâmide de testes

### Unitários — Rust
- Normalização de metadados `RDB$`.
- Construção de contexto de esquema.
- Classificação conservadora de SQL.
- Redação de segredos em logs.
- Construção e validação de prompts.
- Limites de paginação, timeout e tamanho de resposta.
- Mapeamento de erros de driver para erros de domínio.

### Unitários — UI
- Editor mantém abas e estado não salvo.
- Resultados distinguem `NULL` de string vazia.
- Confirmação para operações de escrita.
- Assistente mostra SQL gerado sem executá-lo.
- Modo de privacidade impede envio remoto sem confirmação configurada.

### Integração Firebird
- Testar cada versão na matriz ([01-compatibilidade-firebird.md](01-compatibilidade-firebird.md)).
- Conectar e obter versão.
- Ler tabelas, campos, domínios, índices e constraints.
- Executar SELECT, parâmetros, Unicode, datas, decimais, BLOB e NULL.
- Testar falha de senha, falta de permissão e banco indisponível.
- Verificar rollback quando a operação falha.
- Testar biblioteca cliente escolhida e a arquitetura Linux usada no build.

### Integração IA
- Provedores falsos para teste determinístico.
- Endpoint local compatível com API documentada.
- API remota com segredos carregados do cofre e nunca colocados nos logs.
- Timeouts, resposta malformada, rate limit e indisponibilidade.
- Resposta com SQL de nomes desconhecidos: aviso, sem execução automática.

### End-to-end
- Abrir app, conectar, expandir tabela, inserir nome no editor, executar SELECT, ver
  resultado e pedir explicação à IA.
- Gerar consulta e confirmar que ela aparece no editor sem ser executada.
- Configurar provedor local e remoto, alternar o provedor e testar falhas.

## 2. Matriz de compatibilidade contínua

Manter ambientes de teste isolados por versão. Se versões antigas não puderem rodar em
CI moderno, documentar estratégia reproduzível de VM/container e incluir relatório de
homologação manual versionado. **Não** marcar uma versão como "suportada" somente porque
a conexão funcionou uma única vez.

## 3. Definition of Done

Um requisito está concluído somente quando:

- [ ] testes unitários relevantes escritos antes ou junto à implementação e passando;
- [ ] testes de integração correspondentes passam nas versões aplicáveis;
- [ ] critérios de aceite demonstrados;
- [ ] erros e logs não expõem segredos;
- [ ] documentação do recurso e limitações atualizada;
- [ ] revisão de código concluída;
- [ ] interface com estados de loading, vazio, sucesso e erro quando aplicável.
