# Spec 15 — Open Source e Governança

**Documento de origem:** Seção 17 da especificação técnica, com a decisão de licença
atualizada conforme escolha do mantenedor.

## 1. Licença

**Decisão:** Apache-2.0 para o código da aplicação (confirmada pelo mantenedor em
14/10/2026), por permitir adoção comercial e comunitária com condições explícitas de
atribuição e patentes. Antes da publicação, revisar as licenças de todas as dependências
e a redistribuição ou vinculação dinâmica de bibliotecas Firebird (ADRs pendentes:
ADR-002 e ADR-006).

> Observação: durante o planejamento inicial foi considerada a licença MIT; prevaleceu a
> proposta da especificação técnica (Apache-2.0).

## 2. Arquivos obrigatórios

- `LICENSE` (Apache-2.0) ✔
- `README.md` ✔
- `CONTRIBUTING.md` ✔
- `CODE_OF_CONDUCT.md` ✔
- `SECURITY.md` ✔
- `CHANGELOG.md` ✔
- `THIRD_PARTY_NOTICES.md` ✔
- SBOM por release (Fase 4)
- Política de versionamento e releases (Fase 4)
- Matriz de compatibilidade Firebird (`docs/compatibility/`)

## 3. Práticas de contribuição

- Pull requests pequenos e revisão obrigatória.
- TDD nos módulos de domínio e serviços.
- Formatação/lint/testes no CI.
- Atualização de documentação e changelog quando necessário.
- Nenhum segredo, banco real ou dump de dados de clientes em fixtures.
- Fixtures sintéticas para esquema e dados de teste.
