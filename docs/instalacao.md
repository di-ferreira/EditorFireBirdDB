# Guia de Instalação — Firebird Studio Linux

Este guia cobre as três formas de obter o aplicativo e o que fazer **depois** de
instalar (biblioteca cliente do Firebird, conexão e diagnóstico).

## 1. Requisitos

| Item | Versão | Observação |
|---|---|---|
| Linux | x86_64, glibc ≥ 2.31 | Ubuntu 22.04+, Debian 12+, Linux Mint 21+ |
| WebKitGTK | 4.1 | dependência do Tauri 2 (instalada pelo `.deb`) |
| Biblioteca cliente Firebird | 2.5 / 3.0 / 4.0 / 5.0 | **não** é embarcada; veja a seção 3 |
| Servidor Firebird | 2.5 → 5.0 (TCP) | 2.0/2.1 **não testados** |

Memória recomendada: 2 GB (resultados grandes são paginados em janelas de 100 linhas).

## 2. Instalação

### 2.1 pelo pacote `.deb` (recomendado)

Baixe o asset da release mais recente em
<https://github.com/di-ferreira/EditorFireBirdDB/releases> e instale:

```sh
sha256sum -c Firebird.Studio.Linux_0.1.0_amd64.deb.sha256   # verificação
sudo apt install ./Firebird.Studio.Linux_0.1.0_amd64.deb
```

O pacote declara `Recommends: libfbclient2, firebird3.0-client-common` — o `apt`
oferece instalar a biblioteca cliente junto. Remover:

```sh
sudo apt remove firebird-studio-linux
```

### 2.2 a partir do código-fonte

```sh
# Rust estável e Node 20+
git clone https://github.com/di-ferreira/EditorFireBirdDB.git
cd EditorFireBirdDB
pnpm install

# verificação completa
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
(cd apps/desktop && pnpm build)

# executar em desenvolvimento
(cd apps/desktop && pnpm tauri dev)

# gerar o .deb
(cd apps/desktop && pnpm tauri build)
# → target/release/bundle/deb/
```

### 2.3 binário sem empacotar

```sh
cargo build --release -p firebird-studio-desktop --manifest-path apps/desktop/src-tauri/Cargo.toml
./target/release/firebird-studio-desktop
```

## 3. Biblioteca cliente do Firebird (`libfbclient.so`)

O app **carrega a biblioteca em runtime** (`dlopen`), por isso ela não vem no pacote.

1. Descubra o que existe no sistema:
   ```sh
   find / -name "libfbclient*.so*" 2>/dev/null | grep -v timeshift
   ```
2. Instale se faltar:
   ```sh
   sudo apt install firebird3.0-client-common   # 3.0 (funciona com servidores 2.5+)
   ```
3. Se houver **mais de uma** versão, informe o caminho em
   **Conexões → Novo perfil → Biblioteca cliente** (`clientLibraryPath`), por exemplo:
   - `/usr/lib/libfbclient.so.3.0.11`
   - `/usr/lib/libfbclient.so.2.5.9`

Erros comuns:

| Mensagem | Causa | Solução |
|---|---|---|
| `nenhuma libfbclient.so compatível foi encontrada` | nenhuma biblioteca no sistema | instale o client ou informe `clientLibraryPath` |
| `não pôde ser carregada: verifique se existe e é compatível` | symlink quebrado ou arquitetura errada | `file /usr/lib/libfbclient.so*` e aponte o caminho real |
| `SQL error -902: user name and password are not defined` | credenciais | `SYSDBA`/`masterkey` é o padrão, mas confirme o do seu servidor |

## 4. Primeiro uso

1. **Conexões** → *Novo perfil*:
   - Host `localhost`, porta `3050`
   - Banco: caminho **no servidor** (ex.: `localhost:/var/lib/firebird/3.0/data/app.fdb`)
   - Usuário/senha do servidor
   - Dialeto: **2.0** para servidor 2.5, **3.0** para 3.0+, **4.0** para 4.0
2. **Testar** e depois **Conectar**.
3. **SQL** → digite a consulta e execute (`Ctrl+Enter`).
4. Na lista **Objetos** (à esquerda), **duplo clique** numa tabela abre uma aba com os
   dados — com paginação no servidor e edição por chave primária.

## 5. Assistente de IA (opcional)

Na aba **IA**:

1. Configure um provedor compatível com a API da OpenAI (Ollama, LM Studio, vLLM,
   OpenAI). Exemplo local:
   ```sh
   ollama serve                      # http://localhost:11434/v1
   ollama pull qwen2.5-coder
   ```
2. A chave da API (quando houver) é guardada **no cofre do sistema** — o perfil local
   guarda apenas uma referência.
3. Marcar **“Autorizo enviar o esquema do banco para este provedor remoto”** só deve ser
   feito para provedores em que você confia: o contexto enviado é **apenas metadados**
   (nomes de tabelas e colunas), nunca dados.
4. O SQL sugerido **nunca é executado automaticamente**: ele é inserido no editor para
   sua revisão, e qualquer escrita/DDL pede confirmação antes de rodar.

## 6. Onde ficam os dados locais

| Caminho | Conteúdo |
|---|---|
| `~/.local/share/com.firebirdstudio.desktop/studio.db` | perfis, scripts, histórico, provedores, interações |
| cofre do sistema (Secret Service) | senhas de conexão e chaves de API |

Nada disso é sincronizado nem enviado. Para backup, copie o arquivo `.db` com o app
fechado.

## 7. Diagnóstico

```sh
./target/release/firebird-studio-desktop            # logs no terminal
# ou, instalado:
firebird-studio-linux
```

Problemas conhecidos e limitações estão em
[`CHANGELOG.md`](../CHANGELOG.md) (seção *Limitações conhecidas*) e na
[matriz de compatibilidade](../docs/compatibility/MATRIX.md).
