# Spec 03 — Gerenciador de Conexões

**Documento de origem:** Seção 6.1 da especificação técnica
**Requisitos:** FR-001 (Gerenciar perfis, P0), FR-002 (Conectar ao Firebird, P0)

## 1. Requisitos

- Criar, editar, duplicar, testar, conectar e desconectar perfis.
- Suportar conexão TCP/IP com host, porta, caminho/alias do banco, usuário, role
  opcional, charset e dialeto.
- Permitir conexão local somente se o adaptador e a instalação do cliente suportarem
  esse modo.
- Salvar nome do perfil, host, porta, banco, usuário, charset, dialeto, timeout e caminho
  opcional da biblioteca cliente.
- Salvar senha/tokens **apenas no cofre de segredos do sistema**.
- Separar claramente perfil salvo de sessão conectada.
- Exibir status: `desconectado`, `conectando`, `conectado`, `reconectando` ou `erro`.
- Oferecer um teste de conexão sem registrar o perfil automaticamente.

## 2. Critérios de aceite

- [ ] Uma conexão inválida exibe erro acionável sem travar a interface.
- [ ] Nenhum campo de senha aparece em logs, mensagens de erro ou histórico.
- [ ] O usuário pode escolher uma biblioteca cliente e ver qual biblioteca foi carregada.

## 3. Notas de implementação

- O perfil enviado pela interface **não inclui senha**; o comando de conexão recebe uma
  referência de segredo (`secret_ref`) ou consulta o cofre no backend
  (ver [10-contratos-internos.md](10-contratos-internos.md)).
- Estado de sessão gerenciado pelo Connection Manager no backend (ver
  [02-arquitetura.md](02-arquitetura.md)).
