# Spec 01 — Premissas de Compatibilidade Firebird

**Documento de origem:** Seção 4 da especificação técnica
**Requisitos relacionados:** FR-015 (Compatibilidade versionada, P0)

## 1. Conceito de suporte

"Suportar Firebird 2.0+" significa que cada versão declarada como suportada passou por
testes de integração para as funções incluídas na matriz. **Não** significa que todos os
recursos de administração estarão disponíveis em todas as versões.

A documentação oficial lista Firebird 2.0, 2.1 e 2.5 como séries descontinuadas. Isso não
impede conexões técnicas, mas implica risco de segurança e ausência de manutenção oficial.
O produto deve sinalizar versões legadas e não sugerir que estejam seguras ou recebam
correções.

## 2. Matriz inicial proposta

| Servidor | Objetivo do MVP | Critério |
|---|---|---|
| Firebird 2.0.x | Prioridade de prova de conceito | Conexão, versão, catálogo, SELECT, transação e erro |
| Firebird 2.1.x | Prioridade de prova de conceito | Mesmo conjunto básico; detectar recursos específicos |
| Firebird 2.5.x | Homologação do legado | Testes básicos e cobertura ampliada de metadados |
| Firebird 3.0.x | Homologação | Testes de conexão/autenticação e metadados |
| Firebird 4.0.x | Homologação | Tipos e recursos novos sem quebrar o catálogo comum |
| Firebird 5.0.x | Homologação | Testes da versão mais recente no escopo inicial |

Versões 6.x ou posteriores não fazem parte do compromisso inicial; serão adicionadas
após testes de regressão. Matriz completa em `docs/compatibility/`.

## 3. Biblioteca cliente

O adaptador deve suportar configuração explícita de localização e versão da biblioteca
`libfbclient.so`. **Não** substituir bibliotecas do sistema nem criar links simbólicos
globais automaticamente.

A aplicação deverá oferecer:

- Descoberta de bibliotecas em locais conhecidos e seleção manual do caminho.
- Validação de arquitetura e tentativa de carregamento isolada.
- Exibição do caminho e versão da biblioteca efetivamente carregada.
- Mensagem acionável quando a biblioteca estiver ausente, incompatível ou sem dependências.
- Capacidade de configurar bibliotecas clientes diferentes por perfil, se viável.
- Registro da combinação servidor/cliente testada no relatório de diagnóstico.

Não presumir que uma única `libfbclient.so` será ideal para todas as versões. A prova de
conceito deve avaliar pelo menos duas estratégias: biblioteca nativa oficial e
implementação/protocolo alternativo em Rust (ex.: `rsfbclient`), sem assumir que a
alternativa suporta Firebird 2.0.

## 4. Dialeto, charset e versão

- Armazenar dialeto SQL (1 ou 3) no perfil da conexão ou solicitar confirmação quando não
  determinável com segurança.
- Configurar charset da conexão explicitamente; UTF8 como padrão recomendado.
- Obter a versão do servidor pela API; consulta de catálogo apenas como fallback aplicável.
- Não executar SQL específico de versões novas sem verificar a versão do servidor.
- Normalizar metadados para um modelo interno comum; campos não suportados representados
  como ausentes, **não inventados**.
- Tratar identificadores entre aspas, nomes case-sensitive, caracteres nacionais e nomes
  com espaços.
