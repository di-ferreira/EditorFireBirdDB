#!/usr/bin/env bash
# Harness Docker dos ambientes Firebird de teste (Fase 0 — PoC, Task 4).
#
# Sobe um container Firebird por versão usando as imagens jacobalberty/firebird
# já baixadas neste host, com banco descartável `poc.fdb` (UTF8) e credencial de
# ambiente fictícia (FR-016: nada de credencial real no repositório).
#
# Uso:
#   ./scripts/firebird-env.sh up <tag>      # 2.5-sc | 3.0 | 4.0 | v5.0
#   ./scripts/firebird-env.sh down <tag>    # para e remove o container
#   ./scripts/firebird-env.sh status        # containers de pé + portas
#
# A senha do SYSDBA vem de FB_POC_PASSWORD (padrão: masterkey — credencial
# descartável do container de teste, nunca servidor real). O mesmo valor deve
# ser usado em FB_POC_PASSWORD ao rodar `cargo test -p integration-firebird`.
#
# Nota sobre env vars da imagem: o entrypoint (jacobalberty) usa ISC_PASSWORD
# para criar o SYSDBA dono do banco e FIREBIRD_DATABASE para criar o db em
# /firebird/data. FIREBIRD_PASSWORD só age com FIREBIRD_USER definido; por isso
# o script define ISC_PASSWORD (o que realmente vale) e repassa
# FIREBIRD_PASSWORD para simetria.
set -euo pipefail

IMAGEM_PREFIXO="jacobalberty/firebird"
NOME_PREFIXO="fb-studio"
DATABASE="poc.fdb"
# Caminho absoluto DO SERVIDOR (lado do container): nome relativo no CONNECT
# resolve contra o working dir do processo do servidor, não do cliente.
DATABASE_SERVIDOR="/firebird/data/${DATABASE}"
TIMEOUT_SEGUNDOS=90

porta_da_tag() {
    case "$1" in
        2.5-sc) echo 3051 ;;
        3.0) echo 3052 ;;
        4.0) echo 3053 ;;
        v5.0) echo 3054 ;;
        *) return 1 ;;
    esac
}

tags_suportadas() {
    echo "2.5-sc 3.0 4.0 v5.0"
}

senha() {
    echo "${FB_POC_PASSWORD:-masterkey}"
}

aguardar_porta() {
    local porta="$1"
    local i
    for i in $(seq 1 "${TIMEOUT_SEGUNDOS}"); do
        if (exec 3<>"/dev/tcp/127.0.0.1/${porta}") 2>/dev/null; then
            exec 3>&- 3<&- 2>/dev/null || true
            echo "porta ${porta} pronta após ${i}s"
            return 0
        fi
        sleep 1
    done
    echo "TIMEOUT: porta ${porta} não abriu em ${TIMEOUT_SEGUNDOS}s" >&2
    return 1
}

cmd_up() {
    local tag="${1:-}"
    local porta
    if ! porta=$(porta_da_tag "${tag}"); then
        echo "tag desconhecida: '${tag}' — tags: $(tags_suportadas)" >&2
        exit 1
    fi
    local nome="${NOME_PREFIXO}-${tag}"

    if docker ps --format '{{.Names}}' | grep -qx "${nome}"; then
        echo "container ${nome} já está de pé (porta ${porta})"
    else
        docker rm -f "${nome}" >/dev/null 2>&1 || true
        echo "subindo ${IMAGEM_PREFIXO}:${tag} como ${nome} (host ${porta} -> 3050)..."
        docker run -d --rm \
            --name "${nome}" \
            -p "${porta}:3050" \
            -e "FIREBIRD_DATABASE=${DATABASE}" \
            -e "ISC_PASSWORD=$(senha)" \
            -e "FIREBIRD_PASSWORD=$(senha)" \
            "${IMAGEM_PREFIXO}:${tag}" >/dev/null
    fi

    if ! aguardar_porta "${porta}"; then
        echo "--- últimas linhas do log do ${nome} ---" >&2
        docker logs --tail 40 "${nome}" >&2 || true
        exit 1
    fi

    cat <<FIM
Ambiente ${tag} de pé. Para rodar o checklist de integração:
  export FB_POC_HOST=127.0.0.1
  export FB_POC_PORT=${porta}
  export FB_POC_DATABASE=${DATABASE_SERVIDOR}
  export FB_POC_USER=SYSDBA
  export FB_POC_PASSWORD=$(senha)
  # A lib do host em /usr/lib/libfbclient* e symlink para /opt/firebird/lib e
  # pode estar ilegivel (permissao). A workaround usada na Fase 0 e extrair a
  # lib da propria imagem para target/firebird-libs/<versao>/ (ver secao 8 do
  # docs/compatibility/POC-FASE0.md). Se a lib do host estiver legivel, aponte
  # direto para ela (ex.: /usr/lib/libfbclient.so.3.0.11).
  export FB_POC_LIB_PATH=<caminho-da-libfbclient-legivel>
  cargo test -p integration-firebird -- --nocapture
FIM
}

cmd_down() {
    local tag="${1:-}"
    if ! porta_da_tag "${tag}" >/dev/null; then
        echo "tag desconhecida: '${tag}' — tags: $(tags_suportadas)" >&2
        exit 1
    fi
    local nome="${NOME_PREFIXO}-${tag}"
    if docker ps --format '{{.Names}}' | grep -qx "${nome}"; then
        echo "parando ${nome}..."
        docker stop "${nome}" >/dev/null
    else
        echo "container ${nome} não está de pé"
    fi
}

cmd_status() {
    echo "containers Firebird do harness:"
    local encontrou=0
    for tag in $(tags_suportadas); do
        local nome="${NOME_PREFIXO}-${tag}"
        local porta
        porta=$(porta_da_tag "${tag}")
        local linha
        linha=$(docker ps --filter "name=^${nome}$" --format '{{.Status}}' 2>/dev/null || true)
        if [ -n "${linha}" ]; then
            echo "  ${nome} (${IMAGEM_PREFIXO}:${tag}) porta ${porta}->3050: ${linha}"
            encontrou=1
        else
            echo "  ${nome} (${IMAGEM_PREFIXO}:${tag}) porta ${porta}: parado"
        fi
    done
    if [ "${encontrou}" -eq 0 ]; then
        echo "  (nenhum de pé — use 'up <tag>' para subir)"
    fi
}

main() {
    local cmd="${1:-}"
    shift || true
    case "${cmd}" in
        up) cmd_up "$@" ;;
        down) cmd_down "$@" ;;
        status) cmd_status ;;
        *)
            echo "uso: $0 up <tag $(tags_suportadas)> | down <tag> | status" >&2
            exit 1
            ;;
    esac
}

main "$@"
