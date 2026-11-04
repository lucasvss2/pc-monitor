# kvojps-monitor

Indicador de bandeja para acompanhar limites de uso de IA, sessões de agentes, atividade do GitHub, tarefas do Trello ou Jira Cloud, rede e sistema no Ubuntu com GNOME. O Painel reúne esses Módulos em uma janela; o app funciona localmente e não envia dados a um servidor próprio. A base Windows está em validação experimental; acompanhe os limites no [guia de desenvolvimento](docs/desenvolvimento.md#base-windows-experimental).

A escolha Trello/Jira está implementada nesta árvore de desenvolvimento; não implica disponibilidade nos pacotes já publicados. O [guia de Tarefas](docs/tarefas.md) explica conexão, troca de fonte e limitações.

![Painel do kvojps-monitor aberto no app Tauri nativo, com dados de demonstração sanitizados](docs/assets/painel-fixtures.png)

*Captura do app Tauri em execução com snapshots de demonstração. Os dados são exemplos; esta imagem não é uma captura da versão distribuída.*

## Requisitos

- Ubuntu 24.04 ou mais recente, com GNOME. Outros ambientes podem funcionar, mas não são validados.
- Extensão **Ubuntu AppIndicators** habilitada. Instale-a e ative **Ubuntu AppIndicators** no aplicativo **Extensões**:

  ```sh
  sudo apt install gnome-shell-extension-appindicator
  ```

- GNOME Keyring/Secret Service desbloqueado para salvar credenciais. Sem keyring, o app recusa salvá-las.
- `gh` (GitHub CLI) instalado e autenticado somente se quiser verificar as attestations dos arquivos baixados.

## Instalação

Na [release mais recente](https://github.com/kvojps/kvojps-monitor/releases/latest), baixe os pacotes `.deb` e `.AppImage` e o arquivo `SHA256SUMS` para a mesma pasta, para conferir todos os hashes e attestations. Você instalará apenas um dos formatos:

```sh
sha256sum -c SHA256SUMS
gh attestation verify SHA256SUMS --repo Kvojps/kvojps-monitor
for artifact in *.deb *.AppImage; do
  gh attestation verify "$artifact" --repo Kvojps/kvojps-monitor
done
```

O primeiro comando verifica os hashes. Os comandos `gh` verificam as attestations de proveniência publicadas com a release. Selecione **um** formato:

```sh
# .deb
sudo apt install ./kvojps-monitor_*.deb

# AppImage
chmod +x ./kvojps-monitor*.AppImage
./kvojps-monitor*.AppImage
```

No `.deb`, abra o app pelo menu de aplicativos. No AppImage, o último comando já o inicia. O ícone aparece na bandeja; clique nele para abrir o Painel. O guia de [desenvolvimento e forks](docs/desenvolvimento.md) cobre a compilação a partir do código.

## Primeiros passos

Na primeira abertura, a tela de boas-vindas mostra o estado de cada Módulo e atalhos para configurá-los. Você pode voltar às opções pelo ícone de engrenagem no Painel.

- **Limites de uso de IA:** abra uma sessão do Codex para ler seus limites. Para o Claude Code, use **Conectar ao Claude Code** para instalar a status line; ela encadeia a configuração anterior. `oauth/usage` é opcional, vem desligado e consulta um endpoint não documentado da Anthropic, em uma zona cinzenta nos termos. Leia o aviso e confirme explicitamente antes de ativar. O token é lido somente em memória e nunca renovado pelo app.
- **Sessões de agente:** instale e autentique Claude Code e/ou Codex. Em Configurações → Agentes, **Detecção instantânea** instala hooks de forma aditiva; o Codex pode pedir que você confie neles em `/hooks`. Sem hooks, as sessões ainda são detectadas por consulta periódica, com menos detalhes imediatos.
- **GitHub:** autentique `gh` ou conecte um token clássico nas Configurações → GitHub para ver sua atividade.
- **Tarefas:** em Configurações → Tarefas, escolha Trello ou Jira. No Trello, informe chave/token e escolha um quadro; conexões existentes são preservadas. No Jira Cloud, informe site, e-mail e token de API da sua conta, escolha um quadro e consulte as transições permitidas. As credenciais ficam no cofre nativo. Consulte o [passo a passo](docs/tarefas.md).
- **Rede:** mostra a conexão e a qualidade automaticamente. O teste de velocidade leve é automático; você também pode iniciar uma medição completa no Módulo.
- **Sistema:** exibe uso de CPU, memória, swap, disco, bateria e sensores disponíveis, sem configuração de conta.

## Privacidade e conexões

As coletas e preferências ficam no computador. Credenciais ficam no GNOME Keyring no Ubuntu e no Windows Credential Manager na base Windows, restritas ao usuário e à máquina. Sem cofre real, o app recusa salvá-las. Logs, cache e notificações não recebem prompts, respostas, mensagens de commits ou conteúdo de cards e itens Jira. As leituras do Jira ficam somente em memória. Não há telemetria nem servidor próprio.

Quando os respectivos Módulos são usados, o app acessa `api.github.com` (atividade e consulta de releases), `api.trello.com` (quadros e cards), o site Jira Cloud `https://<seu-site>.atlassian.net` conectado pelo usuário e `speed.cloudflare.com` (teste de velocidade). Somente o provedor de Tarefas selecionado é consultado. **Somente após ativar o opt-in**, consulta `api.anthropic.com/api/oauth/usage`. Para conectividade no Ubuntu, mede ping ao gateway da rede local e a `1.1.1.1`; o NetworkManager informa o estado de conectividade e portal cativo.

## Desinstalação

Antes de remover o app, abra Configurações e:

1. Em **IA**, desconecte o Claude Code para restaurar a status line anterior e desligue `oauth/usage`, se estiver ativo.
2. Em **Agentes**, desligue **Detecção instantânea** para remover apenas os hooks adicionados pelo app.
3. Em **Geral**, desligue **Iniciar com a sessão** para remover o autostart.

Depois, remova o pacote:

```sh
sudo apt remove kvojps-monitor
```

Para o AppImage, feche o app e apague o arquivo `.AppImage` que baixou.

## Projeto e licença

Este é um projeto pessoal que não recebe contribuições externas. Não abra issues de funcionalidades nem pull requests. Forks e adaptações são permitidos conforme a [licença MIT](LICENSE); consulte o [guia de desenvolvimento e forks](docs/desenvolvimento.md).

Para relatar uma vulnerabilidade em privado, leia a [política de segurança](SECURITY.md) e use o [formulário de relato confidencial do GitHub](https://github.com/kvojps/kvojps-monitor/security/advisories/new).
