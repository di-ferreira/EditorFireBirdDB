# Matriz de Compatibilidade — Firebird Studio Linux

**Publicada junto com a release.** É o contrato de suporte do produto: nenhuma versão é
declarada suportada sem evidência de teste reproduzível.

- Release: **0.1.0**
- Última validação real: **2026-10-16** (checklist PoC + harness de integração)
- Fonte primária dos dados: [`POC-FASE0.md`](POC-FASE0.md)

## 1. Servidor × biblioteca cliente

| Servidor | Biblioteca cliente | Dialeto no app | Resultado da validação |
|---|---|---|---|
| **2.5.9** (Docker `2.5-sc`) | `libfbclient.so.2.5.9` | 2.0 | ✅ checklist 6/6 |
| **2.5.9** (servidor local `:3050`) | `libfbclient.so.3.0.10` | 2.0 | ✅ checklist 6/6 (combinação cruzada) |
| **3.0.10** (Docker `3.0`) | `libfbclient.so.3.0.10` | 3.0 | ✅ checklist 6/6 |
| **4.0.0** (Docker `4.0`) | `libfbclient.so.3.0.10` | 4.0 | ✅ checklist 6/6 |
| **5.0.0** (Docker `v5.0`) | `libfbclient.so.3.0.10` | 3.0 | ✅ checklist 6/6 |
| **2.0.x** | — | — | ❌ **não testado** (sem ambiente reproduzível) |
| **2.1.x** | — | — | ❌ **não testado** (sem ambiente reproduzível) |
| qualquer | `libfbclient.so.3.0.11` (do host) | — | ⚠️ não exercitada no PoC (leitura do arquivo exigia privilégio) |

## 2. O que o checklist valida (6 itens)

1. Identificação da versão do engine.
2. `SELECT` parametrizado com Unicode.
3. `SELECT` de catálogo (`RDB$RELATIONS`, `RDB$RELATION_FIELDS`).
4. Tipos numéricos/decimal e `NULL`.
5. Tratamento de erro do servidor.
6. Execução de DML (`INSERT`/`UPDATE`).

## 3. Recursos por versão do servidor

| Recurso | 2.5 | 3.0 | 4.0 | 5.0 |
|---|---|---|---|---|
| Catálogo (`RDB$RELATION_TYPE`) | ✅ | ✅ | ✅ | ✅ |
| `RDB$FIELD_SOURCE` (domínio de campo) | ✅ | ✅ | ✅ | ✅ |
| Chave primária (`RDB$INDEX_SEGMENTS`) | ✅ | ✅ | ✅ | ✅ |
| Paginação com `ROWS` | ✅ | ✅ | ✅ | ✅ |
| Transação manual (begin/commit/rollback) | ✅ | ✅ | ✅ | ✅ |
| `INSERT ... RETURNING` | ❌ bloqueado | ❌ bloqueado | ❌ bloqueado | ❌ bloqueado |
| `EXECUTE BLOCK` | ❌ bloqueado | ❌ bloqueado | ❌ bloqueado | ❌ bloqueado |

`RETURNING` e `EXECUTE BLOCK` estão bloqueados por limitação conhecida do driver
`rsfbclient` 0.29 em modo *dynamic loading* (ver
[`POC-FASE0.md` §5.1](POC-FASE0.md)).

## 4. Plataformas

| Plataforma | Situação |
|---|---|
| Linux x86_64 (Ubuntu 22.04+, Debian 12+, Mint 21+) | ✅ suportado — `.deb` publicado |
| AppImage / RPM | planejado (Fase 4, após validação de dependências) |
| Windows / macOS | fora do escopo do MVP |

## 5. Como reproduzir a validação

```sh
docker compose -f tests/integration-firebird/docker-compose.yml up -d
FB_POC_URL_25SC=... FB_POC_URL_30=... FB_POC_URL_40=... FB_POC_URL_50=... \
  cargo test -p integration-firebird -- --ignored --test-threads=1
```

Sem as variáveis de ambiente o harness **é ignorado** e nunca falha o `cargo test`.

## 6. Responsabilidade do operador

Firebird 2.0 a 2.5 são séries **EOL** (fora de suporte oficial). O app sinaliza isso,
mas não pode mitigar vulnerabilidades do servidor — usar versões EOL é decisão e
responsabilidade do operador. Ver [`SECURITY.md`](../../SECURITY.md).
