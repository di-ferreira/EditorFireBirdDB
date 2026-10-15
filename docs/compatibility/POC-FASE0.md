# PoC Fase 0 — Compatibilidade Firebird (checklist da prova de conceito)

**Data da execução:** 2026-10-15
**Documento:** relatório da prova de conceito exigido pela
[Spec 01 — Compatibilidade](../specs/01-compatibilidade-firebird.md) e pela
[Spec 14 — Plano de entrega](../specs/14-plano-entrega.md) (Fase 0), alimentando a
[ADR-002 — Driver Firebird](../adr/ADR-002-driver-firebird.md).

## 1. O que este documento é (e o que ele NÃO é)

Neste relatório, **"validado no checklist PoC (N/N)"** significa apenas que a
combinação executou os 6 itens obrigatórios da prova de conceito com sucesso **nesta
máquina, nesta data**. **Não** significa "suportado": segundo a Spec 01, §1, suporte é
a passagem pela matriz de testes de integração completa (e em ambiente de
homologação/CI), o que a Fase 0 não faz. Nenhuma versão é declarada suportada aqui,
e as combinações não testadas aparecem explicitamente como **não testadas**.

## 2. Método

### 2.1 Binário `firebird-spike` (crates/firebird-spike)

CLI de uma execução por combinação:

```text
firebird-spike --host <H> --port <P> --database <D> --user <U> --lib <caminho> [--charset <C>]
```

- A senha é lida **exclusivamente** da variável de ambiente `FB_PASSWORD` — não
  existe flag de senha, `--help` não a mostra, `--password`/`--senha` são rejeitados
  com erro acionável e o `Debug` da struct de argumentos sai redigido como `***`
  (FR-016; 13 testes unitários cobrem isso).
- `--lib` aponta a `libfbclient.so` exata a carregar (seleção manual, Spec 01, §3);
  antes de conectar, o spike valida o cabeçalho ELF e a arquitetura da biblioteca.
- A saída é um bloco por combinação: cabeçalho (versão do servidor, biblioteca com
  caminho + versão deduzida do nome, charset, validação ELF), linhas `setup`/
  `teardown` e os **6 itens** classificados ✅ (passou) / ⚠️ (não executado, com
  motivo) / ❌ (falhou, com o texto real do erro censurado).
- Código de saída: `0` = 6/6 ✅; `1` = falha ou item não executado; `2` = uso inválido.

### 2.2 Checklist (os mesmos 6 itens do harness `tests/integration-firebird`)

1. conexão + `server_version` (`RDB$GET_CONTEXT('SYSTEM','ENGINE_VERSION')`);
2. `relations()` (leitura de `RDB$RELATIONS`) contém `CLIENTES` (Table) e `V_CLIENTES` (View);
3. `relation_fields("CLIENTES")` (leitura de `RDB$RELATION_FIELDS`) com tipos/nullable/domain corretos;
4. `SELECT ... WHERE ID = ?` parametrizado devolve a linha certa (`O'BRIEN`);
5. Unicode (`João da Conceição`), decimais (`1234.56`/`-987.65`) e NULL (NULL ≠ vazio) no round-trip;
6. SQL inválido → `DomainError::Driver` **sem a senha** em `Debug`/mensagem.

Schema sintético (`CLIENTES` + `V_CLIENTES`, 2 linhas) é criado com dados apenas
sintéticos (FR-016) e derrubado ao final em **conexão nova**. O spike **pré-checa**
`relations()` antes de criar e **recusa** criar/derrubar caso `CLIENTES`/`V_CLIENTES`
já existam — ele nunca derruba objeto que não criou na própria execução.

### 2.3 Ambientes

- **Docker** (imagens `jacobalberty/firebird` já baixadas neste host), sobidos com
  `scripts/firebird-env.sh up|down|status <tag>` (portas 3051–3054), banco
  descartável `poc.fdb` (UTF8). Após o `up`, o script de execução espera o
  **healthcheck do container ficar `healthy`** antes de conectar (ver §5.4).
- **Servidor local `0.0.0.0:3050`** (`/opt/firebird/bin/fbserver`): **uma única
  tentativa** com credencial padrão `SYSDBA` fornecida via env `FB_PASSWORD`
  (nenhuma repetição; o valor não é reproduzido neste documento).
- **Bibliotecas**: as libs do host (`/usr/lib/libfbclient.so.*` → `/opt/firebird/lib`,
  legíveis apenas por root) **não** podem ser usadas sem sudo; as libs dos testes
  foram **extraídas das próprias imagens Docker** para `target/firebird-libs/{2.5,3.0}`
  (fonte condizente com cada servidor; sem tocar em bibliotecas do sistema, sem
  symlinks globais — Spec 01, §3).
- **Verificação cruzada**: o mesmo checklist foi executado de forma independente pelo
  harness `cargo test -p integration-firebird` (env vars `FB_POC_*`) na Task 4, com
  logs preservados em `/tmp/opencode/task4-logs/run-*-final.log`.

## 3. Matriz de resultados (execução real, 2026-10-15)

| # | Servidor (ambiente) | Porta | Biblioteca cliente (`--lib`) | Resultado |
|---|---|---|---|---|
| 1 | Firebird **2.5.9** — Docker `jacobalberty/firebird:2.5-sc` | 3051 | `libfbclient.so.2.5.9` (extraída da imagem `2.5-sc`) | **validado no checklist PoC (6/6)** |
| 2 | Firebird **3.0.10** — Docker `jacobalberty/firebird:3.0` | 3052 | `libfbclient.so.3.0.10` (extraída da imagem `3.0`) | **validado no checklist PoC (6/6)** |
| 3 | Firebird **4.0.0** — Docker `jacobalberty/firebird:4.0` | 3053 | `libfbclient.so.3.0.10` (extraída da imagem `3.0`) | **validado no checklist PoC (6/6)** |
| 4 | Firebird **5.0.0** — Docker `jacobalberty/firebird:v5.0` | 3054 | `libfbclient.so.3.0.10` (extraída da imagem `3.0`) | **validado no checklist PoC (6/6)** |
| 5 | Firebird **2.5.9** — servidor **local** `/opt/firebird` (alias `employee`) | 3050 | `libfbclient.so.3.0.10` (extraída da imagem `3.0`) | **validado no checklist PoC (6/6)** |
| 6 | Firebird **2.0.x** | — | — | **não testado — pendente de ambiente** (nenhuma tag de imagem reproduzível) |
| 7 | Firebird **2.1.x** | — | — | **não testado — pendente de ambiente** (nenhuma tag de imagem reproduzível) |
| 8 | Qualquer servidor × lib do host **3.0.11** (`/opt/firebird/lib`) | — | `libfbclient.so.3.0.11` | **não exercitada** — ilegível para o usuário comum (sem sudo neste host) |

Notas da matriz:

- As linhas 1–4 também passaram **independentemente** pelo harness da Task 4
  (`cargo test -p integration-firebird`, logs `run-{2.5-sc,3.0,4.0,v5.0}-final.log`),
  com a mesma lib e o mesmo banco — duas execuções distintas, mesmo resultado.
- A linha 5 é **combinação cruzada** (servidor 2.5.9 × cliente 3.0.10): o protocolo
  negociou com sucesso; o servidor local foi identificado como **2.5.9** pela própria
  conexão. A linha 1 valida a combinação homônima 2.5.9 × 2.5.9.
- As linhas 6–8 são **lacunas, não sucessos**. Nada é afirmado sobre 2.0/2.1.
- Saída real preservada em `/tmp/opencode/task5-logs/run-<tag>.log` e
  `run-local-3050.log` (fora do repo; regeneráveis com os comandos da §8).
  O harness da Task 4 preserva `run-*-final.log` no mesmo diretório.

## 4. Registro da execução no servidor local `:3050`

- Porta `0.0.0.0:3050` aberta, processo `/opt/firebird/bin/fbserver` (SuperServer,
  diretórios do servidor ilegíveis para o usuário — nenhuma inspeção de arquivo foi
  possível).
- **Uma única tentativa** de conexão (credencial padrão `SYSDBA` via env `FB_PASSWORD`;
  nenhuma repetição, conforme a instrução da task): **autenticou**.
- Versão identificada pela conexão: `server_version = 2.5.9`.
- Banco: alias `employee` resolveu (13 relações no total no momento do item 2 — as 11
  pré-existentes do banco de exemplo do vendor + as 2 sintéticas da execução).
- Checklist: **6/6 ✅**; `setup` ✅ (pré-checagem: `CLIENTES`/`V_CLIENTES` não
  pré-existiam) e `teardown` ✅ (schema sintético derrubado em conexão nova — nada foi
  deixado para trás).
- Log: `/tmp/opencode/task5-logs/run-local-3050.log`.
- Atenção de segurança registrada: o servidor local **aceitou a credencial padrão do
  SYSDBA**. O valor não é reproduzido aqui; recomenda-se rotacionar a credencial desse
  servidor antes de qualquer uso além desta prova.

## 5. Falhas e limitações observadas (texto real, sem maquiagem)

### 5.1 Bug do `rsfbclient` 0.28/0.29 nativo: classificação de statement

- `isc_info_sql_stmt_type` é lido errado (o terminator `isc_info_end = 1` é
  interpretado como tipo `Select`), então o driver tenta `isc_dsql_fetch` após
  DDL/DML: com cliente **2.5 → SIGSEGV** no processo Rust; com cliente **3.0+ →
  `sql error -504: Cursor is not open`**.
- Contorno (adotado e ratificado): `FirebirdAdapter::execute` usa
  `Execute::execute` (sem fetch) para DDL/DML; `INSERT ... RETURNING` e
  `EXECUTE BLOCK` com saída **permanecem bloqueados para a Fase 1** (não há release
  corrigido do `rsfbclient-native` — 0.29 é a última versão).
- Evidência detalhada: relatório da Task 4 (§3.3), incluindo backtrace do gdb.

### 5.2 `RDB$DOMAIN_NAME` não existe em 2.5/3.0

```text
sql error -206: Column unknown / F.RDB$DOMAIN_NAME
```

Corrigido no `MetadataRepository` via `RF.RDB$FIELD_SOURCE` (portátil 2.0–5.0);
evidência nos logs da Task 4.

### 5.3 Bibliotecas do host ilegíveis

```text
Driver(erro de driver: error: /usr/lib/libfbclient.so.2.5.9: cannot open shared object file: Permission denied)
```

`/usr/lib/libfbclient.so.*` são symlinks para `/opt/firebird/lib` (root-only, sem
sudo neste host). Workaround registrado: libs extraídas das imagens Docker. A lib
**3.0.11** do host nunca foi exercitada (lacuna da linha 8 da matriz).

### 5.4 Corrida de startup do ambiente Docker (achado desta task)

Conectar logo após `scripts/firebird-env.sh up` produz:

```text
[1] ❌ conexão + server_version: conexão falhou: Driver(erro de driver: sql error -923: connection rejected by remote interface)
```

Causa: a porta do mapeamento Docker-proxy aceita conexão **antes** de o `fbserver`
interno estar pronto (o healthcheck da imagem leva ~30–45 s para `healthy`). Na Task 4
a corrida foi mascarada pelo tempo de build do `cargo test`. Evidência preservada em
`/tmp/opencode/task5-logs/run-{3.0,4.0,v5.0}-race-startup.log`; a execução definitiva
aguarda `healthy` e passa 6/6. **Lição para a Fase 1/CI: porta aberta ≠ servidor pronto.**

### 5.5 Demais limitações herdadas/registradas

- **Firebird 2.0 e 2.1**: sem ambiente reproduzível nesta etapa — **não testado,
  pendente de ambiente** (nunca "suportado").
- **Teardown em conexão viva trava** no 2.5 SuperClassic (espera infinita de lock de
  metadata) — corrigido no harness com `drop(adapter)` antes da reconexão (Task 4, §3.5);
  o spike segue o mesmo padrão.
- **Quirk 2.5-sc**: em uma execução da Task 4, `RDB$GET_CONTEXT` devolveu
  `-901: Context variable ENGINE VERSION is not found` em uma segunda conexão
  simultânea; não reproduzido nos runs finais nem nesta task.
- **Auth legada do 2.5**: `Warning - maximum 8 significant bytes of password used`
  (a credencial do container fictício opera com os 8 primeiros bytes significativos;
  valor não reproduzido aqui — FR-016).
- **Dialeto**: a prova executa com o dialeto padrão do banco (3); a variação
  dialeto 1 × 3 fica para a Fase 1 (armazenar no perfil — Spec 01, §4).
- **Charset**: UTF8 em todas as execuções (padrão recomendado); charset distinto por
  conexão não foi exercitado além do `--charset` da CLI.
- Este PoC prova execução reproduzível **nestas combinações, nesta máquina**; não
  substitui a matriz de homologação da Spec 01 (§2).

## 6. Lições

1. Compatibilidade é provada por execução, não presumida — e cada lacuna (2.0/2.1,
   lib 3.0.11) fica escrita como lacuna.
2. Porta de container aberta não significa servidor pronto (§5.4); aguardar healthcheck.
3. O caminho sem fetch (`Execute::execute`) é obrigatório para DDL/DML enquanto o bug
   do `rsfbclient` existir; SELECT com `RETURNING` exige patch/fork (Fase 1).
4. Consultas de catálogo precisam de variantes por versão com evidência de erro real
   (`RDB$DOMAIN_NAME` só existiria "de graça" se ninguém tivesse testado em 2.5).
5. Extrair as libs das imagens de teste mantém cliente e servidor condizentes e evita
   tocar em bibliotecas do sistema (Spec 01, §3).
6. Pré-checagem de schema antes de DDL evita destruir objeto alheio com nomes
   sintéticos genéricos (`CLIENTES`).
7. Servidor local com credencial padrão do SYSDBA: registrar e rotacionar (§4).

## 7. Recomendação de driver (base da ADR-002)

**Estratégia primária: `rsfbclient-native` 0.29 com a feature `dynamic_loading` e
`DynLoad.lib_path`**, apontando para a `libfbclient.so` escolhida pelo usuário.

Justificativa: foi a única estratégia **executada** na Fase 0 e validou o checklist
6/6 nas 5 combinações da matriz; mantém o adaptador isolado atrás do trait
`FirebirdAdapter`; atende à exigência de seleção manual/por-perfil da Spec 01, §3.

Riscos que a Fase 1 precisa fechar (detalhados na ADR-002): bug de classificação de
statement (fork/patch), `lib 3.0.11` não exercitada, 2.0/2.1 sem ambiente, libs do
host ilegíveis (política de descoberta/seleção) e a corrida de startup em CI.

## 8. Reprodução (senha nunca em argv nem em arquivo)

```sh
# 1) subir um ambiente (aguardar o healthcheck "healthy" antes de conectar —
#    ver §5.4):
./scripts/firebird-env.sh up 3.0

# 2) executar o spike (senha SOMENTE via env; valor documentado em
#    scripts/firebird-env.sh — credencial descartável do container fictício,
#    não reproduzido aqui — FR-016):
FB_PASSWORD=<credencial-do-container> ./target/debug/firebird-spike \
  --host 127.0.0.1 --port 3052 \
  --database /firebird/data/poc.fdb \
  --user SYSDBA \
  --lib "$PWD/target/firebird-libs/3.0/usr/local/firebird/lib/libfbclient.so.3.0.10"

# 3) derrubar:
./scripts/firebird-env.sh down 3.0
```

Bibliotecas recriáveis (quando `target/firebird-libs/` for limpo):

```sh
mkdir -p target/firebird-libs/3.0
docker create --name fbtmp jacobalberty/firebird:3.0
docker export fbtmp | tar -x -C target/firebird-libs/3.0 usr/local/firebird/lib
docker rm fbtmp
```

Checklist independente (mesmos 6 itens, env `FB_POC_*`):

```sh
cargo test -p integration-firebird -- --nocapture   # sem env: ignorado, nunca falha
```
