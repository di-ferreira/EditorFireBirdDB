#!/usr/bin/env python3
"""Gera o SBOM CycloneDX 1.5 do Firebird Studio Linux.

Fontes (locais, sem rede):
  * `Cargo.lock`      — crates Rust (nome, versão, dependências)
  * `pnpm-lock.yaml`  — pacotes npm/frontend

Licenças não vêm dos lockfiles; por isso cada componente sai com
`licenses: [{license: {id: "NOASSERTION"}}]` — nenhuma licenca inventada.
O SBOM é um inventário de *versões*, para SBOM/auditoria, não uma fonte de
licenças. O inventário de licenças continua em `THIRD_PARTY_NOTICES.md`.

Uso:  python3 scripts/gen_sbom.py > docs/security/sbom.cdx.json
"""

from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path

import toml
import yaml

RAIZ = Path(__file__).resolve().parent.parent
APP_VERSION = toml.load(RAIZ / "apps/desktop/src-tauri/Cargo.toml")["package"]["version"]
NOMES_RUST = {
    "firebird-domain",
    "firebird-adapter",
    "firebird-storage",
    "firebird-security",
    "firebird-studio-desktop",
    "firebird-spike",
    "integration-firebird",
}


def purl_crate(nome: str, versao: str) -> str:
    return f"pkg:cargo/{nome}@{versao}"


def purl_npm(nome: str, versao: str) -> str:
    # escopo npm vira `namespace/name`
    if "/" in nome:
        escopo, base = nome.split("/", 1)
        return f"pkg:npm/{escopo}%2F{base}@{versao}"
    return f"pkg:npm/{nome}@{versao}"


def componentes_rust() -> tuple[list[dict], list[dict]]:
    lock = toml.load(RAIZ / "Cargo.lock")
    componentes: list[dict] = []
    relacoes: list[dict] = []
    ids: dict[tuple[str, str], str] = {}

    for pacote in lock.get("package", []):
        nome = pacote["name"]
        versao = pacote["version"]
        purl = purl_crate(nome, versao)
        ids[(nome, versao)] = purl
        componentes.append(
            {
                "type": "library",
                "bom-ref": purl,
                "name": nome,
                "version": versao,
                "purl": purl,
                "scope": "required",
                "licenses": [{"license": {"id": "NOASSERTION"}}],
                "properties": [
                    {"name": "ecosystem", "value": "cargo"},
                    {"name": "source", "value": "workspace" if nome in NOMES_RUST else "crates.io"},
                ],
            }
        )

    for pacote in lock.get("package", []):
        origem = (pacote["name"], pacote["version"])
        if origem not in ids:
            continue
        for dep in pacote.get("dependencies", []):
            alvo = (dep if isinstance(dep, str) else dep[0], None)
            if alvo[0] in ids or any(k[0] == alvo[0] for k in ids):
                versao_alvo = next(k[1] for k in ids if k[0] == alvo[0])
                relacoes.append(
                    {"ref": ids[origem], "dependsOn": [ids[(alvo[0], versao_alvo)]]}
                )

    return componentes, relacoes


def componentes_npm() -> tuple[list[dict], list[dict]]:
    lock = yaml.safe_load((RAIZ / "pnpm-lock.yaml").read_text(encoding="utf-8"))
    secoes = lock.get("snapshots", {}) or {}
    pacotes = lock.get("packages", {}) or {}

    componentes: list[dict] = []
    relacoes: list[dict] = []
    ids: dict[tuple[str, str], str] = {}

    for chave, meta in sorted(pacotes.items()):
        if "/" not in chave:
            continue
        nome, versao = chave.split("@", 1)
        versao = versao.lstrip("/")
        purl = purl_npm(nome, versao)
        if purl in ids.values():
            continue
        ids[(nome, versao)] = purl
        componentes.append(
            {
                "type": "library",
                "bom-ref": purl,
                "name": nome,
                "version": versao,
                "purl": purl,
                "scope": "required",
                "licenses": [{"license": {"id": "NOASSERTION"}}],
                "properties": [
                    {"name": "ecosystem", "value": "npm"},
                    {"name": "integrity", "value": str(meta.get("resolution", {}).get("integrity", ""))},
                ],
            }
        )

    for chave in sorted(secoes):
        if "/" not in chave:
            continue
        nome, versao = chave.split("@", 1)
        versao = versao.lstrip("/")
        origem = ids.get((nome, versao))
        if origem is None:
            continue
        depende = secoes[chave].get("dependencies", {}) or {}
        alvos = []
        for dep_nome in sorted(depende):
            for (n, v), purl in ids.items():
                if n == dep_nome:
                    alvos.append(purl)
                    break
        if alvos:
            relacoes.append({"ref": origem, "dependsOn": alvos})

    return componentes, relacoes


def comparar_com_arquivo(caminho: Path) -> int:
    """Compara o SBOM gerado com o arquivo, ignorando o timestamp."""
    gerado = json.loads(json.dumps(_construir()))
    atual = json.loads(caminho.read_text(encoding="utf-8"))
    for doc in (gerado, atual):
        doc.get("metadata", {}).pop("timestamp", None)
    if gerado == atual:
        print("SBOM atualizado")
        return 0
    print("SBOM desatualizado: rode `python3 scripts/gen_sbom.py > docs/security/sbom.cdx.json`")
    return 1


def _construir() -> dict:
    comps_rust, rels_rust = componentes_rust()
    comps_npm, rels_npm = componentes_npm()

    purl_app = f"pkg:generic/firebird-studio-linux@{APP_VERSION}"
    bom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:firebird-studio-{APP_VERSION}-{len(comps_rust) + len(comps_npm):04d}",
        "version": 1,
        "metadata": {
            "timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "tools": [
                {
                    "vendor": "Firebird Studio Linux",
                    "name": "scripts/gen-sbom.py",
                    "version": "1",
                }
            ],
            "component": {
                "type": "application",
                "bom-ref": purl_app,
                "name": "Firebird Studio Linux",
                "version": APP_VERSION,
                "purl": purl_app,
                "licenses": [{"license": {"id": "Apache-2.0"}}],
                "properties": [
                    {"name": "distro", "value": "deb"},
                    {"name": "repository", "value": "https://github.com/di-ferreira/EditorFireBirdDB"},
                ],
            },
        },
        "components": comps_rust + comps_npm,
        "dependencies": [
            {"ref": purl_app, "dependsOn": sorted(c["bom-ref"] for c in comps_rust + comps_npm)}
        ]
        + rels_rust
        + rels_npm,
    }

    return bom


def main() -> int:
    if len(sys.argv) > 2 and sys.argv[1] == "--check":
        return comparar_com_arquivo(RAIZ / sys.argv[2])
    json.dump(_construir(), sys.stdout, indent=2, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
