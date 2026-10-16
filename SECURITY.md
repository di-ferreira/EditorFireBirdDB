# Política de Segurança — Firebird Studio Linux

## Suporte a versões

O projeto publicou a **primeira release (`v0.1.0`)**. Correções de segurança são
aplicadas **apenas na versão mais recente**; versões anteriores recebem correção apenas
com acordo explícito do mantenedor.

| Série | Status |
|---|---|
| `0.1.x` (atual) | suportada — recebe correções |
| anteriores | sem suporte de segurança |

Atenção: Firebird 2.0/2.1/2.5 são séries oficialmente descontinuadas (EOL). O Firebird
Studio Linux sinaliza essas versões no produto, mas **não** pode mitigar vulnerabilidades
do lado do servidor — usar essas versões é decisão e responsabilidade do operador.

## Reportando vulnerabilidades

**Não abra issues públicas para vulnerabilidades.**

Reporte de forma privada ao time mantenedor pelo canal oficial do repositório
(seção *Security* do GitHub → *Report a vulnerability*, advisories privados), incluindo:

- descrição do problema e impacto potencial;
- passos de reprodução ou PoC;
- versão do aplicativo, distribuição Linux, versão do Firebird e biblioteca cliente;
- correção sugerida, se houver.

Você receberá confirmação do recebimento em até 72 horas e uma avaliação em até 10 dias
úteis. Divulgação coordenada após correção ou acordo de prazo.

## Compromissos de segurança do projeto

- Senhas e API keys armazenadas somente no cofre de segredos do sistema operacional;
  nunca em texto puro no SQLite, logs ou erros ([Spec 11](docs/specs/11-requisitos-nao-funcionais.md)).
- Nenhum SQL gerado pela IA é executado automaticamente ([Spec 08](docs/specs/08-arquitetura-ia.md)).
- Fronteira Tauri com permissões mínimas; sem shell genérico exposto à UI.
- Sem telemetria por padrão; envio a provedores remotos de IA é visível e confirmável.
- Nenhum credencial de serviço externo embutida em builds.
- Cada release publica SBOM e changelog ([Spec 15](docs/specs/15-governanca.md)).

## Superfície do aplicativo (0.1.0)

O que **entra** e o que **não entra** no escopo de segurança:

| Área | Estado |
|---|---|
| Segredos (senhas de conexão, chaves de API) | só no cofre do sistema; o SQLite guarda `secret_ref` |
| Redação em logs/erros/DTO | valores após `password=`, `passwd=`, `pwd=`, `token=` viram `***` |
| Frontend (WebView) | CSP `default-src 'self' blob:`; sem shell/plugin genérico exposto à UI |
| Rede de saída | apenas para o **servidor Firebird** e para o **provedor de IA configurado** |
| IA | SQL sugerido **nunca** é executado automaticamente; escrita/DDL exigem confirmação |
| Envio de contexto a provedor remoto | exige consentimento explícito por perfil |
| Permissões Tauri | apenas `core:default` (validado em teste) |
| Telemetria | nenhuma |

### SBOM

Cada release publica o inventário de componentes em
[`docs/security/sbom.cdx.json`](docs/security/sbom.cdx.json) (CycloneDX 1.5), gerado de
`Cargo.lock` e `pnpm-lock.yaml` por `scripts/gen_sbom.py` (sem rede). Para conferir:

```sh
python3 scripts/gen_sbom.py --check docs/security/sbom.cdx.json
```

(Para regerar: `python3 scripts/gen_sbom.py > docs/security/sbom.cdx.json`.)

As **licenças** não vêm dos lockfiles e por isso aparecem como `NOASSERTION` no SBOM; o
inventário de licenças continua em [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
