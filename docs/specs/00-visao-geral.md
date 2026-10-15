# Spec 00 — Visão Geral do Projeto

**Projeto:** Firebird Studio Linux
**Documento de origem:** [ESPECIFICACAO_TECNICA_FIREBIRD_STUDIO_LINUX.md](../original/ESPECIFICACAO_TECNICA_FIREBIRD_STUDIO_LINUX.md) (Seções 1–3)
**Versão:** 0.1
**Status:** proposta inicial para validação técnica

## 1. Resumo executivo

O Firebird Studio Linux é uma IDE desktop open source para conectar e administrar
bancos Firebird, escrever e executar SQL, explorar metadados e gerar consultas com
assistência de IA contextual. Funciona com bancos Firebird a partir da linha 2.0
(2.1, 2.5, 3.0, 4.0 e 5.0) dentro de uma matriz de compatibilidade publicada e testada.

### Capacidades do MVP (prioridade)

1. Gerenciador de conexões Firebird.
2. DB Explorer com objetos e metadados do banco.
3. SQL Editor com execução controlada e grade de resultados.
4. Assistente de IA que gera e explica SQL a partir do esquema real da conexão ativa.

### Diretrizes centrais

- IA configurável para provedores locais ou APIs externas; conteúdo enviado a provedor
  remoto deve ser mostrado e controlado pelo usuário. Por padrão, não se enviam registros
  de negócio, senhas ou strings de conexão.
- Persistência local de preferências e histórico via SQLite.
- Primeiras entregas: Linux x86_64, com foco em Linux Mint e Ubuntu.

> **Risco técnico prioritário:** a compatibilidade real com Firebird 2.0/2.1 no Linux
> moderno não deve ser presumida. A primeira entrega de engenharia é uma prova de
> conceito de conexão, leitura de metadados e execução de SQL contra cada versão mínima
> exigida (ver [14-plano-entrega.md](14-plano-entrega.md), Fase 0).

## 2. Decisões já confirmadas

| Tema | Decisão |
|---|---|
| SGBD | Somente Firebird; InterBase fora do escopo inicial |
| Versão mínima | Firebird 2.0; incluir 2.1, 2.5 e versões posteriores homologadas |
| Sistema operacional | Linux; primeira meta: Linux Mint/Ubuntu x86_64 |
| Tipo de aplicação | Desktop, não aplicação web hospedada |
| IA | Provedores locais e APIs externas configuráveis |
| MVP | SQL Editor, DB Explorer, resultados e IA contextual |
| Código-fonte | Open source |
| Licença | Apache-2.0 (decisão confirmada pelo mantenedor; ver [15-governanca.md](15-governanca.md)) |
| Método de engenharia | TDD, critérios de aceite e testes de integração por versão |

## 3. Objetivos funcionais (resumo)

- Registrar, testar, abrir e fechar conexões Firebird locais ou remotas.
- Navegar por bancos, tabelas, campos, views, índices, chaves, procedures, triggers,
  generators/sequences, domínios e constraints, conforme a versão e permissões.
- Executar uma instrução ou seleção SQL e apresentar resultados, metadados de colunas,
  erros e duração.
- Manter múltiplas abas SQL, histórico local e scripts salvos.
- Gerar SQL em linguagem natural com contexto de metadados do banco ativo.
- Explicar SQL existente e sugerir correções quando houver erro.
- Permitir revisão e edição humana antes de qualquer execução de SQL gerado pela IA.
- Identificar a versão do servidor e adaptar consultas de catálogo e recursos disponíveis.
- Funcionar sem conexão à internet com provedor local e dependência Firebird instalada.

## 4. Fora do escopo do MVP

- InterBase.
- Depurador visual de procedures/triggers.
- Modelador ERD completo e engenharia direta/reversa avançada.
- Assistentes completos de backup/restore e nbackup.
- OLAP, relatórios avançados e painel completo de monitoramento/Trace.
- Replicação, administração de servidores remotos, migração automática entre versões.
- Editor visual de tabelas com gravação de DDL no banco (MVP: inspeção + SQL manual
  com assistência da IA).
- Envio automático de amostras de registros ao modelo.

Esses itens podem ser adicionados após o MVP sem comprometer os contratos de domínio.

## 5. Links

- Arquitetura: [02-arquitetura.md](02-arquitetura.md)
- Requisitos rastreáveis (FR-xxx): [13-criterios-aceite-mvp.md](13-criterios-aceite-mvp.md)
- Riscos e decisões abertas: [16-riscos-decisoes.md](16-riscos-decisoes.md)
