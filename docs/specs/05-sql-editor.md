# Spec 05 — SQL Editor

**Documento de origem:** Seções 6.3 e 10 da especificação técnica
**Requisitos:** FR-005 (SQL Editor, P0), FR-007 (Histórico SQL, P1)

## 1. Capacidades do MVP

- Abas múltiplas e estado não salvo claramente sinalizado.
- Realce de sintaxe para Firebird SQL/PSQL (CodeMirror 6).
- Números de linha, localizar/substituir e indentação.
- Executar seleção ou instrução atual.
- Atalho configurável para executar.
- Mostrar duração, contagem de linhas e erro devolvido pelo servidor.
- Histórico consultável com data, conexão e estado do comando; permitir limpeza
  (e desativação — ver [09-modelo-dados-local.md](09-modelo-dados-local.md)).
- Inserir nomes de objetos/campos pelo DB Explorer.
- Mensagens de erro com código, texto e posição quando a API fornecer dados suficientes.
- Suporte a parâmetros e binds se o adaptador oferecer mapeamento de tipos robusto;
  caso contrário, documentar a limitação no MVP.

## 2. Escopo de execução inicial

- A execução principal será de **uma instrução preparada por vez** ou do texto selecionado.
- Execução de scripts múltiplos deve usar parser consciente de delimitadores e blocos PSQL;
  é **proibido** dividir texto por `;` com uma operação simples de string.
- Se o suporte completo a scripts com `SET TERM` não estiver pronto, o MVP deve informar
  essa limitação e direcionar o usuário a executar uma instrução/bloco por vez.
- Não alterar silenciosamente o texto antes de executar.

## 3. Convenções do editor e parser (Seção 10)

- Tratar SQL como texto até o servidor preparar/validar a instrução; **não** criar um
  parser completo para validar toda a gramática Firebird no MVP.
- Usar camada leve para encontrar a instrução atual e reconhecer comentários, literais e
  delimitadores básicos.
- Testes específicos para strings com `;`, comentários, identificadores entre aspas,
  `EXECUTE BLOCK`, procedimentos e `SET TERM`.
- Nunca dividir um script simplesmente por ponto e vírgula.
- Quando o editor não determinar a instrução atual com confiança, solicitar execução da
  seleção ou permitir execução do documento sob confirmação.
- Informar claramente qual texto será enviado ao servidor.

## 4. Critérios de aceite

- [ ] Editar, selecionar e executar instrução (FR-005).
- [ ] Pesquisar e reabrir histórico local, com opção de desativar (FR-007).
- [ ] Testes do parser cobrem `;` em strings, comentários, aspas, `EXECUTE BLOCK` e
      `SET TERM`.
