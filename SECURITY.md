# Política de Segurança — Firebird Studio Linux

## Suporte a versões

Como o projeto ainda está em Fase 0 (prova de conceito), **nenhuma versão é considerada
estável para produção**. Após o primeiro release, correções de segurança serão aplicadas
apenas na versão mais recente, salvo acordo explícito do mantenedor.

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
