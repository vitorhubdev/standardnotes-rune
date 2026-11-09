# Standard Notes Rune

Cliente desktop nativo em **Rust + Rune**, em migração a partir do Standard Notes.
Versão **0.1.0-alpha.1**. O executável nativo não depende de Electron, Chromium,
Node.js ou de um WebView. Esta é uma implementação independente em desenvolvimento,
e ainda não tem paridade com todos os recursos do aplicativo original.

Leia [MIGRATION-COVERAGE.md](MIGRATION-COVERAGE.md) antes de abrir dados existentes.
O código original continua no repositório como referência de compatibilidade.

## Executar

- **Windows x64:** abra o `.exe`, ou extraia o pacote portátil `.zip`. O executável
  não tem assinatura Authenticode.
- **Linux x64:** extraia o `.tar.gz` e execute `./standardnotes-rune`. O pacote
  `.deb` é destinado ao Ubuntu 24.04 e distribuições compatíveis.
- A versão Linux precisa de X11 ou Wayland e de um driver Vulkan. O diálogo de
  arquivos usa o portal do desktop. Em Ubuntu, instale `libxkbcommon0`,
  `libxkbcommon-x11-0`, `libwayland-client0`, `libvulkan1`, um driver Vulkan e
  `xdg-desktop-portal` com o backend do seu ambiente.

Na primeira abertura, crie um cofre com uma senha. O cofre local `.snrvault` usa
Argon2id e XChaCha20-Poly1305. Sua senha local é independente da senha da conta
Standard Notes. O aplicativo não lê nem altera automaticamente o perfil Electron
original; para trazer dados, use a função de importar um backup.

Os dados são gravados em um diretório próprio obtido das pastas do sistema por
`rune::dirs::AppDirs`, com identificador `dev.vitorhub.standardnotes-rune`.
É possível escolher outro cofre pela interface ou definir `SN_RUNE_VAULT` com o
caminho completo de um arquivo. O arquivo `.bak` ao lado do cofre contém a versão
criptografada anterior. Guarde uma cópia externa do cofre e sua senha.

## Uso

| Ação | Atalho |
|---|---|
| Nova nota | Ctrl+N |
| Salvar agora | Ctrl+S |
| Buscar | Ctrl+F |
| Bloquear cofre | Ctrl+L |

A edição é salva automaticamente. O indicador de gravação só confirma o estado
após a resposta do processo de armazenamento. A lista oferece busca, etiquetas,
notas fixadas, arquivo e lixeira. Notas com formatos que o cliente não compreende
permanecem protegidas contra edição destrutiva.

Uma exportação sem criptografia contém o conteúdo legível das notas; a interface
pede confirmação antes de criá-la. O arquivo interno do cofre e seu backup são
sempre criptografados.

Em **Conta**, conecte uma conta Standard Notes de protocolo 004, usando o
servidor padrão ou um servidor próprio compatível. O código TOTP é opcional
quando a conta não exige autenticação de dois fatores. As credenciais da sessão
ficam dentro do cofre criptografado. **Sincronizar** inicia a troca manual com o
servidor; a interface aguarda o salvamento local antes de enviar alterações e
preserva cópias dos conflitos de notas e etiquetas. A integração foi testada com
HTTP simulado e vetores do upstream, ainda sem uma conta real.

O mesmo painel oferece **Importar** e **Backup com senha** em janelas pequenas.
Backups criptografados do protocolo 004 podem ser importados com a senha
correspondente. A exportação com senha cria um documento Standard Notes `.sn`
independente da senha local e não inclui a sessão da conta. Escolha um nome de
arquivo novo: o aplicativo recusa sobrescrever arquivos existentes.

## Compilar

Rune está fixado no commit
`e970367bd7d2a51e2e7d58e2978ea32eac7780cb`; Rust está fixado em **1.99.1**.
`Cargo.lock` faz parte do código da aplicação.

No Linux:

```bash
sudo apt-get install build-essential pkg-config libxkbcommon-dev libwayland-dev
cd native
cargo test --workspace --locked
cargo build --release --locked -p standardnotes-rune
bash scripts/package-linux.sh
```

No Windows, instale Rust e as ferramentas de C++/Windows SDK do Visual Studio.
Use a toolchain `x86_64-pc-windows-msvc`:

```powershell
cd native
cargo test --workspace --locked
cargo build --release --locked -p standardnotes-rune
./scripts/package-windows.ps1
```

O workflow `Rune native desktop` executa testes e build em Windows e Linux,
renderiza a cena de demonstração e fornece os pacotes com SHA-256 como artefatos.
Somente tags explícitas `rune-v*` publicam uma pré-release; pushes comuns apenas
geram artefatos de CI.

## Verificação da interface

A cena `demo` contém apenas dados sintéticos e não abre ou grava um cofre real:

```bash
cargo run -p standardnotes-rune -- --scene demo --snap demo.png --tree demo.json
```

Os testes de interface usam o runtime de Rune para acionar controles e verificar
o estado. Um build bem-sucedido, um teste de domínio e uma captura da interface
são verificações diferentes; a matriz de cobertura registra suas limitações.

## Estrutura

| Crate | Responsabilidade |
|---|---|
| `sn-rune-core` | Itens, etiquetas, backups, preservação de metadados e cofre local |
| `sn-rune-protocol` | Criptografia e transporte do protocolo Standard Notes 004 |
| `standardnotes-rune` | Interface Rune, edição, comandos e processo de armazenamento |

As operações de disco e derivação de chaves ficam fora do thread da interface.
O cofre tem bloqueio exclusivo entre processos, gravação por substituição
atômica e detecção de alterações externas. Seus limites e os recursos que ainda
precisam de migração constam em `MIGRATION-COVERAGE.md`.

## Licença e atribuição

Este fork preserva a licença **AGPL-3.0** do Standard Notes. Consulte `LICENSE`
na raiz. Rune é distribuído sob licença MIT por Anomaly. Este cliente não é uma
versão oficial do Standard Notes nem usa a identidade de assinatura da Proton.
As licenças das dependências e fontes embarcadas, o inventário e suas origens
estão em [`third-party/THIRD_PARTY_NOTICES.md`](third-party/THIRD_PARTY_NOTICES.md).
