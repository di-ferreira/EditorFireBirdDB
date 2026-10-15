# Spec 08 — Arquitetura da IA

**Documento de origem:** Seção 7 da especificação técnica
**ADRs relacionadas:** ADR-004 (contratos de provedores de IA)

## 1. Componentes

1. `AIProviderRegistry`: lista provedores configurados e capacidades.
2. `AIProvider`: interface comum para envio de mensagens e recebimento de resposta.
3. `SchemaContextBuilder`: transforma catálogo/metadados em contexto compacto e relevante.
4. `PromptTemplateRegistry`: templates versionados por tarefa e versão do Firebird.
5. `SQLResponseParser`: extrai SQL, explicação, pressupostos e avisos da resposta.
6. `SQLRiskClassifier`: classificação conservadora do tipo de operação.
7. `SchemaReferenceValidator`: confere referências conhecidas a tabelas/campos quando possível.
8. `AIHistoryRepository`: guarda interação e metadados sem credenciais.

## 2. Provedores

### Provedor local
- Suportar endpoint OpenAI-compatible como primeira integração.
- Separar URL base, modelo e timeout do armazenamento de chaves.
- Verificar conectividade e listar modelos quando a API suportar.
- Não baixar modelos automaticamente no MVP.

### Provedor remoto
- Adaptador para API compatível com Chat Completions ou interface equivalente documentada.
- Guardar a chave em cofre do sistema.
- Configurar timeout, modelo, limite de contexto e parâmetros permitidos.
- Mostrar custo/consumo apenas com dados verificáveis do provedor.

### Contrato sugerido (conceitual, TS)

```ts
export type AIProviderCapabilities = {
  supportsStreaming: boolean;
  supportsJsonMode: boolean;
  supportsModelListing: boolean;
};

export type AIMessage = {
  role: 'system' | 'user' | 'assistant';
  content: string;
};

export type AICompletionRequest = {
  model: string;
  messages: AIMessage[];
  temperature?: number;
  maxOutputTokens?: number;
  timeoutMs: number;
};

export interface AIProvider {
  readonly id: string;
  getCapabilities(): Promise<AIProviderCapabilities>;
  complete(request: AICompletionRequest): Promise<AICompletionResponse>;
}
```

O contrato acima é conceitual; o contrato final deve ser implementado no backend Rust e
exposto ao frontend por DTOs serializáveis, **sem enviar tokens de acesso ao código da
interface**.

## 3. Fluxo de geração

1. Usuário informa a intenção e escolhe a conexão/contexto.
2. Backend identifica os objetos relacionados, sem obter registros de negócio.
3. `SchemaContextBuilder` constrói o contexto, respeitando limite de tokens/tamanho.
4. Interface mostra o escopo enviado ao provedor remoto, quando aplicável.
5. Provedor gera proposta de SQL e explicação.
6. Backend normaliza a resposta e tenta identificar o SQL.
7. Validação verifica referências conhecidas e classifica risco; **não** declara o SQL
   semanticamente correto só porque passou na validação.
8. SQL aparece em painel de revisão/editável.
9. Usuário decide inserir no editor; a execução é ação separada.
10. Em erro do servidor, usuário pode pedir correção enviando erro + SQL selecionado.

## 4. Regras de prompt

- Incluir versão, dialeto e charset da conexão.
- Declarar que apenas os objetos descritos no contexto são confirmados; nomes não
  encontrados devem gerar pergunta ou suposição sinalizada.
- Preferir aliases claros e nomes qualificados quando necessários.
- Pedir que o modelo indique pressupostos sobre regras de negócio ausentes.
- Solicitar SQL compatível com a versão do servidor selecionada.
- Separar código SQL de explicação e riscos.
- **Nunca** instruir o modelo a executar comandos; a execução é controlada pela aplicação.
- Não inserir secrets nos prompts.

## 5. Limites de segurança

- Nenhum SQL da IA será executado automaticamente.
- A classificação lexical de SQL é apenas barreira auxiliar, não um sandbox.
- Operações desconhecidas, múltiplas instruções, PSQL e procedimentos devem receber
  classificação conservadora.
- A aplicação não deve tentar "provar" que uma consulta é segura somente pelo primeiro
  token (`SELECT`, por exemplo).
- O usuário deve confirmar a conexão/ambiente antes de operações de escrita.
- Recomendado criar perfis de conexão de leitura com permissões mínimas no próprio Firebird.
