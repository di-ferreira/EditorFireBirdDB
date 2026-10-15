# Spec 07 — Assistente de IA Contextual

**Documento de origem:** Seção 6.5 da especificação técnica
**Requisitos:** FR-008 (Provedor local, P0), FR-009 (Provedor remoto, P0),
FR-010 (Contexto do esquema, P0), FR-011 (Pré-visualização de SQL, P0),
FR-012 (Explicar SQL, P1), FR-013 (Corrigir SQL, P1)

## 1. Comandos do usuário

- Gerar consulta SQL.
- Explicar consulta selecionada.
- Corrigir SQL usando a mensagem de erro.
- Sugerir otimização a partir do SQL e, quando disponível, do plano/estatísticas.
- Gerar descrição preliminar para tabela ou campo, sempre identificada como inferência
  até ser revisada.

## 2. Contexto enviado ao modelo

- Versão do Firebird, dialeto, charset e limites relevantes.
- Objetos selecionados explicitamente pelo usuário.
- Definição compacta das tabelas, campos, tipos, constraints e relacionamentos relevantes.
- Definições de views/procedures somente quando necessário e permitido pelo usuário.
- SQL selecionado e mensagem de erro, mediante ação explícita.
- Descrições locais e documentação fornecida pelo usuário, se selecionadas.

## 3. Não enviar por padrão

- Senhas, tokens, DSNs com credenciais ou arquivos de configuração sensíveis.
- Linhas de dados reais do banco.
- Resultados completos de consultas.
- DDL de todo o banco quando somente alguns objetos forem relevantes.

## 4. UX de privacidade

- Para provedor remoto, mostrar provedor e resumo do contexto enviado.
- Oferecer modo de confirmar antes de cada envio remoto.
- Não enviar nada ao provedor até uma ação explícita do usuário.
- Permitir desativar o compartilhamento de definições PSQL e comentários dos metadados.
- Guardar histórico de prompts localmente por padrão, com opção para não guardar.

## 5. Critérios de aceite

- [ ] Testar conexão e solicitar geração contextual com provedor local (FR-008).
- [ ] Configurar provedor remoto e exigir política explícita de envio (FR-009).
- [ ] Enviar apenas metadados relevantes selecionados (FR-010).
- [ ] SQL gerado nunca é executado automaticamente (FR-011).
- [ ] Explicação separada do código SQL (FR-012).
- [ ] Usar erro retornado pelo servidor, mediante ação do usuário (FR-013).
- [ ] SQL com tabela/campo inexistente gera aviso de referência não confirmada.
- [ ] Aplicação continua utilizável quando o provedor está indisponível.
- [ ] Modo de privacidade impede envio remoto sem confirmação configurada (teste de UI).
