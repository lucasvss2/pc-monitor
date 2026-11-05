# Desenvolvimento

Guia para executar, testar e compilar o app a partir do código, inclusive em um fork. O suporte declarado é Ubuntu 24.04+ com GNOME/Wayland e extensão AppIndicator. Outras distribuições podem funcionar, mas não são o ambiente validado.

Este é um projeto pessoal que não recebe contribuições externas. Para adaptar ou manter sua própria versão, faça um fork conforme a licença MIT; preserve o aviso de copyright e a licença nas cópias distribuídas. Este guia não estabelece um fluxo de contribuição ao projeto original.

## Ferramentas e dependências

Use Git, Node.js 24 com npm e Rust estável via rustup, com `rustfmt` e `clippy`. O mínimo declarado em `src-tauri/Cargo.toml` é Rust 1.90; o CI acompanha o estável. Instale Node e rustup antes dos comandos abaixo. A CLI Tauri é uma dependência local do projeto, sem instalação global.

No Ubuntu, instale as dependências nativas usadas pelo [CI](../.github/workflows/ci.yml):

```sh
sudo apt-get update
sudo apt-get install -y git curl build-essential file pkg-config \
  libayatana-appindicator3-dev libdbus-1-dev librsvg2-dev libssl-dev \
  libwebkit2gtk-4.1-dev libxdo-dev
rustup component add rustfmt clippy
```

Para a sessão gráfica, mantenha o GNOME Keyring/Secret Service desbloqueado e a extensão AppIndicator habilitada. No Ubuntu, os pacotes são `gnome-keyring` e `gnome-shell-extension-appindicator`; habilite “Ubuntu AppIndicators” no aplicativo Extensões e, se necessário, encerre e reabra a sessão. Sem keyring, o app recusa salvar credenciais.

As integrações dependem das fontes disponíveis: Claude Code e Codex no `PATH` e autenticados para seus respectivos Módulos; GitHub CLI (`gh`) autenticado pode fornecer o token do GitHub. Elas não são necessárias para compilar ou executar os testes com fixtures. Ausência de uma fonte deve aparecer como indisponibilidade no Painel.

## Clone e instalação

```sh
git clone https://github.com/kvojps/kvojps-monitor.git
cd kvojps-monitor
npm ci
cargo fetch --locked --manifest-path src-tauri/Cargo.toml
```

Para um fork, substitua a URL pela do seu repositório. Preserve `package-lock.json` e `src-tauri/Cargo.lock` para reproduzir as dependências. A instalação inicial precisa de rede para os registros npm e Cargo.

## Executar com dados reais

```sh
npm run tauri -- dev
```

O Tauri inicia o Vite na porta 1420 e o núcleo Rust. O app aparece no Indicador; clique nele para abrir o Painel. Esse modo usa as configurações, cache e integrações reais do usuário, inicia coletas locais e acessos de rede conforme as preferências. Credenciais ficam no keyring; status line e hooks só são instalados após ação explícita na interface. O token do Claude é lido em memória e nunca renovado pelo app.

`npm run dev` sozinho inicia apenas o frontend no navegador; não fornece o núcleo nem as APIs nativas. Para ver dados conhecidos sem coleta real, use o protótipo ou o modo nativo de fixtures abaixo. Esses recursos de desenvolvimento não existem no build de produção.

## Verificações

Execute da raiz, na ordem usada pelo CI (o build da UI cria `dist`, lido por `generate_context!` durante a compilação Rust):

```sh
cargo fmt --check --manifest-path src-tauri/Cargo.toml
npm run check
npm test
npm run build
cargo clippy --all-targets --locked --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo clippy --all-targets --locked --features fixtures --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --locked --features fixtures --manifest-path src-tauri/Cargo.toml --lib fixtures
npm run tauri -- build --no-bundle
```

Para uma iteração curta, `npm test -- src/lib/ai/AiModule.test.ts` executa um arquivo da UI (use um caminho existente); `cargo test --locked --manifest-path src-tauri/Cargo.toml nome_do_teste` filtra testes Rust. A suíte completa usa portas injetáveis e fixtures sanitizadas, sem exigir contas reais.

## Compilar pacotes locais

```sh
npm run tauri -- build
# Ou selecione um formato:
npm run tauri -- build --bundles deb
npm run tauri -- build --bundles appimage
```

A configuração atual gera `.deb` em `src-tauri/target/release/bundle/deb/` e AppImage em `src-tauri/target/release/bundle/appimage/`. O executável fica em `src-tauri/target/release/kvojps-monitor`. `--no-bundle` compila somente o executável. O empacotador pode baixar ferramentas na primeira execução; sem rede, um build do executável não comprova que o empacotamento funciona.

Não habilite `fixtures` em release: a compilação é recusada. Pacotes locais não recebem automaticamente os checksums e atestados de uma release publicada. Em um fork, revise nome, identificador, versão e ícones em `src-tauri/tauri.conf.json`, metadados em `src-tauri/Cargo.toml` e `package.json`, além de URLs do projeto e workflows de release antes de distribuir.

## Release

`src-tauri/tauri.conf.json` é a fonte da verdade da versão, inicialmente `0.1.0`. Atualize também `src-tauri/Cargo.toml`, `package.json` e os lockfiles correspondentes antes de criar a tag. Confira localmente com:

```sh
python3 scripts/check-release-version.py v0.1.0
```

Um push de tag `v*` dispara o [pipeline de release](../.github/workflows/release.yml). Ele recusa divergências entre tag e manifestos, reutiliza todo o CI e usa `tauri-action` em Ubuntu 24.04 para gerar `.deb` e AppImage. Em seguida, gera `SHA256SUMS`, atesta os três arquivos e verifica sua proveniência antes de criar o release em rascunho com notas automáticas. A publicação do rascunho é manual; não há `CHANGELOG.md` nem assinatura GPG.

Após baixar os dois pacotes e `SHA256SUMS` para a mesma pasta, verifique:

```sh
sha256sum -c SHA256SUMS
gh attestation verify SHA256SUMS --repo Kvojps/kvojps-monitor
for artifact in *.deb *.AppImage; do
  gh attestation verify "$artifact" --repo Kvojps/kvojps-monitor
done
```

As attestations são registradas na API do GitHub pela action oficial [`actions/attest`](https://github.com/actions/attest), sem chave privada mantida no repositório. A validação local não substitui a primeira execução do workflow: emissão OIDC, permissões e criação do rascunho só podem ser comprovadas no GitHub. Se uma execução falhar depois de criar um rascunho, remova o rascunho incompleto antes de repetir a criação; o workflow não sobrescreve releases existentes.

## Protótipo B

```sh
npm run prototype:compact
```

Abre `compact-prototype.html?variant=B` no navegador (porta 1421). É a referência de design e conteúdo do Painel ([spec, "Visual do Painel"](../.scratch/kvojps-monitor/spec.md#visual-do-painel)); a captura aprovada está em [`assets/painel-b.png`](assets/painel-b.png). Fixtures e ações do protótipo são simuladas.

## Modo de fixtures nativo

Abre o app nativo (Tauri/WebKitGTK) com os snapshots do protótipo B, para comparar a produção lado a lado com o B na janela real. Feche qualquer instância antes (instância única: a nova entregaria o Painel à antiga e sairia).

```sh
npm run dev:fixtures
KVOJPS_FIXTURES="ai=claude-failure,network=poor" npm run dev:fixtures
```

Módulos: `ai`, `agents`, `github`, `tasks`, `network`, `system`; o identificador legado `trello` continua aceito na seleção. Nomes das fixtures em `src/lib/<módulo>/fixtures/`; os exemplos Trello permanecem em `src/lib/trello/fixtures/`. Padrão (B): IA `claude-usage`, Agentes `states`, GitHub/Tarefas/Rede `full`, Sistema `notebook`. Para apresentar Jira sem acesso real, use `KVOJPS_FIXTURES="tasks=full-jira" npm run dev:fixtures`; essa leitura vem de `src/lib/tasks/fixtures/full-jira.json`. Em PowerShell, defina `$env:KVOJPS_FIXTURES='tasks=full-jira'` antes de executar e remova a variável com `Remove-Item Env:KVOJPS_FIXTURES` ao terminar.

O que o modo garante:

- Os snapshots seguem o caminho normal (`snapshot:<módulo>` e `initial_state`); a UI não tem caminho paralelo. A injeção é no nível do snapshot, não nas portas brutas, porque as fixtures do B são snapshots da UI. Nenhum coletor inicia: sem leitura do sistema, rede, keyring, `~/.claude/settings.json` ou `~/.codex/hooks.json`, sem notificações e sem verificação de versão.
- Mover card, testar velocidade, conectar/desconectar Claude, `oauth/usage`, token do GitHub, Trello e hooks respondem em memória. A fixture Jira permite verificar a apresentação; autenticação e transições reais exigem o aceite separado. Nenhuma escrita deste modo alcança o Jira ou o Trello. Atalhos de release, logs e administração do Trello podem abrir navegador/arquivos reais.
- Relógio da webview fixado em `1793401200000` (30/10/2026 20:00), com as duas janelas de IA abertas como no B.
- Preferências e autostart em `$TMPDIR/kvojps-monitor-fixtures`; boas-vindas puladas.
- Feature Cargo `fixtures`, desligada por padrão; `--release` com a feature falha por `compile_error!`; fixtures lidas do repositório em tempo de execução. Testes em `src-tauri/src/fixtures.rs` e `build_tests` em `src-tauri/src/lib.rs`.

## Captura nativa

O GNOME recusa `org.gnome.Shell.Screenshot` a clientes comuns. No modo de fixtures, a própria instância renderiza a webview e mede a rolagem (`src-tauri/src/capture.rs`). Com `npm run dev:fixtures` rodando:

```sh
scripts/capture-panel.sh /caminho/absoluto/b-1100-escuro 1100x700 dark
scripts/capture-panel.sh /caminho/absoluto/b-1000-claro 1000x700 light
```

Gera `<prefixo>.png` e `<prefixo>.json` e sai com erro se a captura falhar. A instância reabre o Painel, pede o tamanho lógico e corrige pela diferença medida (no Wayland o tao entrega 52 px a menos por eixo). O JSON traz `width`/`height`, `opened` (tamanho antes do ajuste), `scroller`, `overflowing` (**vazio = sem rolagem**), `theme`, `devicePixelRatio` e `styles`.

**Vite frio:** na primeira partida, o `vite-plugin-svelte` pode falhar ao carregar o CSS de alguns componentes ("failed to load virtual css module"); o WebKit mantém a falha nas reaberturas. A captura recusa esse estado; reinicie `npm run dev:fixtures`.

## Imagens sanitizadas

Para imagens do app nativo com dados de exemplo, inicie `npm run dev:fixtures` e use a captura nativa acima, com saída temporária fora do repositório. Isso captura a UI real do Tauri alimentada por snapshots de exemplo; identifique esse modo na legenda, sem apresentar o protótipo HTML como versão distribuída.

## Base Windows experimental

A adaptação Windows tem acompanhamento e limites em [Validação da base Windows](windows-validacao.md). O suporte oficial completo ainda depende das etapas e do aceite da especificação; o incremento não habilita por suposição os coletores Linux. A interface identifica Módulos indisponíveis e descoberta parcial de sessões.

O coletor de limites de IA tem adaptador Codex nativo/npm e status line Claude no Windows, com OAuth Claude opcional explicitamente indisponível. Pré-requisitos, limites e aceite separado em [Limites de IA no Windows](ia-windows-validacao.md). A descoberta completa de Sessões de agente permanece uma etapa distinta.

O Módulo Tarefas usa o mesmo coordenador e adaptadores de serviço nos dois sistemas, com cofre nativo por plataforma. O [guia de uso](tarefas.md) e o [roteiro de aceite](tarefas-validacao.md) distinguem testes automatizados de leitura e ação em conta de teste. Não habilite o provedor de produção para validar escrita sem autorização para o item escolhido.

Para compilar nativamente, use MSVC x64 com Windows SDK, Rust estável, Node.js 24 e o WebView2 necessário ao Tauri. Da raiz, em PowerShell:

```powershell
npm ci
npm run check
npm test
npm run tauri -- build --bundles nsis
```

O build gera o EXE portátil em `src-tauri/target/release/` e o instalador em `src-tauri/target/release/bundle/nsis/`. Use `--no-bundle` para compilar apenas o portátil. A CI oferece o instalador experimental sem assinatura no artefato `windows-x64-installer-unsigned`. O instalador usa o perfil do usuário e baixa o bootstrapper da Microsoft quando o WebView2 está ausente. Consulte a [validação Windows](windows-validacao.md#instalador-exe-por-usuário) para instalar e remover.

Sem cofre nativo real, o app recusa salvar credenciais. A prova explícita do cofre usa somente uma entrada sintética isolada e a remove ao terminar:

```powershell
cargo test --locked --manifest-path src-tauri/Cargo.toml native_vault_roundtrip_is_local_machine_and_cleanup_is_idempotent -- --ignored
```

Os demais testes não instalam hooks na conta real; usam diretórios temporários. A captura GTK descrita neste guia continua exclusiva do Linux. Se uma política impedir compilar ou executar binários sem assinatura, preserve as proteções e valide em um ambiente compatível; não trate compilação cruzada ou testes de navegador como prova nativa Windows.
