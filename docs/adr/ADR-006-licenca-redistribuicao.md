# ADR-006: Política de licença e redistribuição de bibliotecas

**Status:** Parcialmente decidida — redistribuição pendente
**Contexto:** [Spec 15 — Governança](../specs/15-governanca.md)

## Contexto

O projeto é open source e pode depender de vinculação dinâmica com `libfbclient.so`
(código Firebird com licença própria IPL/CDDL-1.0). A licença do código da aplicação
precisa ser compatível com a redistribuição e com adoção comunitária/comercial.

## Decisão (código da aplicação)

**Apache-2.0** para o código-fonte da aplicação, conforme proposta da especificação
técnica e confirmação do mantenedor (14/10/2026). MIT foi considerado e descartado em
favor da proteção de patentes implícita no Apache-2.0.

## Pendente

- Revisar licenças de todas as dependências (Rust, npm) antes da primeira publicação.
- Definir política de redistribuição/vinculação dinâmica das bibliotecas Firebird e o
  que vai em `THIRD_PARTY_NOTICES.md` e no SBOM por release.
- Confirmar se o cliente Firebird pode ser redistribuído nos pacotes `.deb` ou se a
  instalação do cliente fica por conta do usuário/sistema.

## Consequências

- Builds não podem embutir credenciais de serviços externos (Spec 11, §1).
- Cada release publica SBOM e changelog (Spec 15, §2).
