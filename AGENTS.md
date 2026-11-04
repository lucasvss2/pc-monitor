# kvojps-monitor

App pessoal de bandeja para Ubuntu/GNOME/Wayland (Tauri v2: núcleo Rust + UI Svelte 5) que reúne Limites de uso de IA, Sessões de agente, GitHub, Trello, rede e sistema num Indicador com Painel. Tudo local, custo zero.

## Fonte da verdade

- **v1 publicada**: `.scratch/kvojps-monitor/spec.md` preserva o contrato original de produto; confira a implementação e os guias atuais ao alterar comportamento. O visual aprovado e os porquês seguem registrados na spec e em `docs/adr/`.
- **Mudando comportamento documentado**: confira as decisões existentes e registre mudanças de domínio em `docs/adr/`; use o tracker local em `.scratch/` quando a tarefa exigir acompanhamento por ticket.
- **Tarefas, Windows, versão reduzida ou relógio**: consulte o plano em [.scratch/jira-windows/map.md](.scratch/jira-windows/map.md) e os ADRs associados antes de implementar. Decisões aprovadas no mapa não comprovam suporte já implementado.

## Idioma

Código em inglês: arquivos, identificadores, comentários, nomes de testes, chaves de dados e comandos internos. Textos exibidos na interface em pt-BR. README, `docs/` e `GLOSSARY.md` em pt-BR. Mensagens de commit em inglês, no formato Conventional Commits (`feat:`, `fix:`, `docs:`).

## Privacidade (guardrails)

O app lê dados sensíveis do usuário; trate estas regras como invioláveis e cubra cada uma com teste:

- Credenciais vivem só no cofre nativo real: GNOME Keyring no Ubuntu; Windows Credential Manager no Windows, restrito ao usuário e à máquina, sem roaming. Sem cofre real, o app recusa salvar.
- Logs, cache em disco e notificações carregam só metadados (fonte, evento, status, duração), nunca credenciais nem conteúdo de prompts, respostas, commits, cards ou itens do Jira.
- `~/.claude/settings.json` e `~/.codex/hooks.json` só mudam após clique explícito do usuário, de forma aditiva e reversível, preservando o que outros apps instalaram.
- O token do Claude é lido só em memória e nunca renovado pelo app.

## Dados versionados e capturas

O repositório é público: fixtures, pesquisas, `.scratch/`, imagens, skills e histórico também são material publicado. Capturas reais devem preservar o contrato da fonte com identificadores e conteúdo privado substituídos por exemplos antes de entrar na árvore Git, no terminal compartilhado ou em relatórios.

Ao capturar dados, adicionar evidências, fazer commit ou preparar push, siga obrigatoriamente [docs/agents/sanitizacao.md](docs/agents/sanitizacao.md). Revise também endereços de rede, nomes de projetos/sessões e metadados Git; ausência de tokens não basta para aprovar a publicação.

## Testes

Um único seam no núcleo: toda E/S (processos, HTTP, sistema de arquivos, D-Bus, keyring, notificador, relógio) passa por portas injetáveis, alimentadas nos testes com saídas reais gravadas e sanitizadas como fixtures. A UI é testada a partir de fixtures de snapshot. Detalhes em "Testing Decisions" da spec.

## Git

Branches `research/*` e `prototype/*` são locais: nunca faça push delas. A spec v1 permanece em `.scratch/`; qualquer material novo ali também é publicado e deve seguir a sanitização.

## Agent skills

### Issue tracker

Markdown local em `.scratch/` (spec, tickets e mapas do `/wayfinder`). See `docs/agents/issue-tracker.md`.

### Triage labels

Os cinco papéis canônicos com os nomes padrão (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` e `docs/adr/` na raiz. See `docs/agents/domain.md`.
