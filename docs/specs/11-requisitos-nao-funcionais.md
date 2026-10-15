# Spec 11 — Requisitos Não Funcionais e Segurança

**Documento de origem:** Seção 11 da especificação técnica
**Requisitos relacionados:** FR-016 (Logs seguros, P0)

## 1. Segurança

- Senhas e API keys armazenadas no cofre de segredos do sistema operacional.
- Não imprimir dados de conexão, senhas, chaves ou tokens nos logs.
- Não disponibilizar comandos shell genéricos à interface.
- Restringir comandos de arquivo a operações necessárias e caminhos explícitos.
- Validar entradas na fronteira Tauri, mesmo quando a UI já as validou.
- Permitir que logs detalhados sejam desativados e ocultar SQL potencialmente sensível.
- TLS para provedores remotos sempre que aplicável; advertir sobre endpoints HTTP não
  locais sem TLS.
- Sem telemetria por padrão. Caso seja adicionada, deve ser opt-in, documentada e sem
  dados do banco.
- Não embutir credenciais de serviços externos em builds.

## 2. Desempenho — metas iniciais a medir

Objetivos de engenharia, **não resultados comprovados**:

- Interface responsiva enquanto consulta/geração de IA está em andamento.
- Explorar o primeiro nível de metadados sem carregar detalhes de todos os objetos.
- Buscar páginas de resultados com limite padrão configurável, inicialmente 500 linhas
  por lote.
- Evitar que resultados grandes sejam mantidos integralmente na memória.
- Feedback de progresso em conexões, refresh e chamadas de IA.
- Iniciar a UI sem esperar conexão a banco ou provedor de IA.

## 3. Usabilidade e acessibilidade

- Interface em português brasileiro no primeiro idioma, com arquitetura preparada para
  internacionalização.
- Atalhos de teclado, navegação por teclado e foco visual.
- Mensagens de erro orientadas à ação.
- Confirmações específicas para mudanças destrutivas, não diálogos genéricos frequentes.
- Diferenciar visualmente desenvolvimento/teste/produção sem depender apenas da cor.

## 4. Confiabilidade

- Operações de banco canceláveis quando o driver permitir.
- Fechar a janela deve tratar sessões ativas e avisar sobre consultas/alterações pendentes.
- Recuperar o estado dos scripts abertos após encerramento inesperado, se possível, sem
  persistir conteúdo quando o usuário desativar esse recurso.
- Recursos dependentes de versão devem expor capacidade detectada e motivo de
  indisponibilidade.
