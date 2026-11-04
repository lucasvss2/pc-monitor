# IA e Sessões de agente no Windows

Data da consulta: **03/11/2026**. Ticket: [Como acompanhar IA e Sessões de agente no Windows?](../issues/03-windows-agents.md). Pesquisa documental; nenhuma instalação, leitura de credenciais, configurações pessoais ou sessões reais, nem execução dos agentes. Não escolhe entre Windows nativo e WSL para o produto.

## Resultado para a decisão de escopo

Há caminhos oficialmente documentados para executar os dois agentes no Windows nativo e no WSL. O trabalho de integração muda conforme o ambiente escolhido: processo, diretório de configuração, identidade da sessão, shell e canal de entrega devem pertencer ao mesmo ambiente. Um arquivo de histórico não comprova processo vivo. Um servidor iniciado pelo monitor também não comprova observar todas as execuções independentes do agente. As fontes e limites que sustentam essas conclusões estão discriminados abaixo.

O caminho de menor dependência de formatos privados é conservar `app-server` para Limites do Codex, a status line para Limites do Claude e as interfaces públicas de estado/eventos onde houver cobertura. SQLite, rollouts e arquivos internos continuam sendo dependências de adaptadores específicos, sem promessa de compatibilidade. Isso é uma avaliação técnica desta pesquisa, não uma nova decisão de produto.

## Contratos existentes no repositório

Fontes locais lidas antes da pesquisa OpenAI: [AGENTS.md](../../../AGENTS.md), [GLOSSARY.md](../../../GLOSSARY.md), [ADR 0002](../../../docs/adr/0002-limites-claude-status-line-e-oauth-opt-in.md) e implementações abaixo. Mantêm-se clique explícito para conectar, configuração aditiva/reversível, metadados apenas e token Claude exclusivamente em memória, sem renovação.

| Área atual | Evidência local | Consequência para Windows |
| --- | --- | --- |
| Limites Codex | [ai/process.rs](../../../src-tauri/src/ai/process.rs) inicia `codex app-server` como filho, stdin/stdout em pipes, descarta stderr; [ai/mod.rs](../../../src-tauri/src/ai/mod.rs) consulta conta/limites e recebe atualizações | Transporte stdio é reaproveitável conceitualmente; resolução do executável, contexto/home e ausência de janela precisam de adaptador validado |
| Fallback dos Limites Codex | [ai/mod.rs](../../../src-tauri/src/ai/mod.rs), funções de leitura de `/sessions` e eventos `token_count`/`rate_limits` | Contrato interno de rollout; leitura antiga deve conservar origem e idade, sem fingir consulta atual |
| Sessões Codex | [agents/codex.rs](../../../src-tauri/src/agents/codex.rs) lê `state_5.sqlite`, tabela `threads`, rollouts; cruza arquivos abertos em `/proc/<pid>/fd` de processos `codex` | Não é uma descoberta portável: arquivo presente em SQLite só entra como sessão nova se houver observação viva; `/proc`, symlink e UID são Linux |
| Sessões Claude | [agents/mod.rs](../../../src-tauri/src/agents/mod.rs) executa `claude agents --json --all`; `Record` converte `status`, `state`, `pid`, `sessionId`, `waitingFor` | Interface pública relevante; seus estados e campos opcionais precisam de fixtures por versão/ambiente |
| Hooks | [agents/hooks.rs](../../../src-tauri/src/agents/hooks.rs) adiciona entradas próprias e registra manifesto para remover só essas entradas; comando recebe quoting POSIX | Preservar estratégia de propriedade e rollback; a serialização do comando exige escolha por plataforma |
| Transporte dos hooks | [ports.rs](../../../src-tauri/src/ports.rs): `UnixDatagram`, modo `0600`, stdin limitado por `libc::poll/read`; [agents/hooks.rs](../../../src-tauri/src/agents/hooks.rs): socket em `XDG_RUNTIME_DIR` | Contrato `HookSocket` pode sobreviver; implementação Unix e orçamento do stdin precisam de outro adaptador nativo |
| Status line Claude | [ai/claude.rs](../../../src-tauri/src/ai/claude.rs) salva valor original, cria wrapper `#!/bin/sh`, encadeia via `sh -c`, projeta somente duas janelas de `rate_limits`; desconexão preserva substituição de terceiro | Reescrever invocação e encadeamento para o shell escolhido; não executar o original PowerShell como texto POSIX |

Essas afirmações descrevem o checkout pesquisado. Nenhuma delas comprova o comportamento de uma instalação Windows real.

## Execução nativa e WSL são alvos distintos

O Codex possui agente nativo Windows/PowerShell e opção WSL2. O app usa `%USERPROFILE%\.codex`, enquanto o CLI em WSL usa home Linux por padrão; configuração, autenticação e histórico não são automaticamente compartilhados. O shell do terminal pode ser configurado separadamente do ambiente do agente. Logo, detectar um terminal PowerShell não identifica onde o agente roda. [OpenAI: Windows app](https://learn.chatgpt.com/docs/windows/windows-app)

No WSL2, Codex usa ambiente e sandbox Linux. WSL1 deixou de ser suportado a partir do Codex `0.115`. [OpenAI: WSL](https://learn.chatgpt.com/docs/windows/wsl)

Claude Code admite Windows nativo, WSL2 e WSL1; sandbox de comandos é suportado em WSL2, não nos outros dois. No nativo, Git for Windows é opcional: sua presença habilita Bash; sem ele os comandos usam PowerShell. A instalação nativa não exige administrador. [Anthropic: setup](https://code.claude.com/docs/en/setup#set-up-on-windows)

No Windows, `~/.claude` corresponde a `%USERPROFILE%\.claude`. `CLAUDE_CONFIG_DIR` pode deslocar configurações, histórico e plugins. Há camadas de usuário, projeto, local e gerenciadas; instalar em um arquivo não garante que ele determine a configuração efetiva. [Anthropic: settings](https://code.claude.com/docs/en/settings)

**Inferência para o monitor:** representar o ambiente de origem explicitamente, incluindo distribuição WSL quando aplicável. Não copiar homes, sincronizar auth nem converter `C:/...`, `/mnt/c/...` e `\\wsl$\...` por substituição textual presumida. O escopo pode ser nativo, WSL ou ambos, mas suporte duplo requer descoberta e deduplicação próprias. Não há experimento nesta pesquisa que autorize prometer essa ponte.

## Limites e protocolo do Codex

O `app-server` documenta JSON-RPC com stdio/JSONL padrão, handshake `initialize` seguido de `initialized`, e geração de schemas correspondentes à versão do CLI. `account/rateLimits/read` e `account/rateLimits/updated` fornecem percentuais, duração da janela em minutos e reset Unix em segundos; há visão compatível `rateLimits` e mapa `rateLimitsByLimitId`. [OpenAI: app-server](https://learn.chatgpt.com/docs/app-server)

Também documenta `thread/list` para histórico, `thread/read` sem carregar a thread, `thread/loaded/list` para IDs em memória, estados `notLoaded`, `idle`, `systemError`, `active` e `thread/status/changed`, incluindo espera por aprovação. `thread/closed` pode decorrer de descarregar memória após inatividade; não equivale necessariamente ao fim do processo. O WebSocket TCP é experimental e sem suporte. [OpenAI: app-server](https://learn.chatgpt.com/docs/app-server)

**Limite/inferência:** esses métodos descrevem o servidor consultado. Não há garantia verificada de que um filho stdio recém-iniciado observe o runtime de todos os CLIs/app/IDE independentes. `loaded` não significa inventário global de processos vivos. Não usar `thread/resume` para tentar descobrir estado: retomar altera o runtime. Respostas de histórico podem carregar `preview` e nomes; a projeção deve eliminá-los antes de log/cache/transporte.

O CLI classifica `codex app-server` como experimental, sujeito a mudança, e oferece conexão do TUI a servidor compartilhado. Isso permite investigar cobertura adicional, não torna o servidor próprio do monitor um observador global. [OpenAI: developer commands](https://learn.chatgpt.com/docs/developer-commands#codex-app-server)

**Validação necessária:** binário nativo versus launcher/shim, versão mínima, handshake, interrupção/EOF, fallback sem login, múltiplas janelas/buckets, notificações fora de ordem, timestamps, servidor próprio versus compartilhado. O adaptador atual SQLite/rollout fica identificado como interno instável; não há nesta pesquisa contrato público para `state_5.sqlite`, schema `threads` ou associação Windows PID→rollout aberto.

## Fontes de Sessões de agente Claude

`claude agents --json --all` é a interface oficialmente recomendada para leitura externa do estado. A lista abrange sessões vivas e background que seguem trabalhando/bloqueadas mesmo com processo ausente; `--all` acrescenta background concluídas. `cwd/kind/startedAt` são constantes; `pid/status` só aparecem com processo vivo; `sessionId/name` são opcionais. `status` pode ser `busy/waiting/idle`; `state` pode ser `working/blocked/done/failed/stopped`. `waitingFor` detalha a espera. `done` não implica processo encerrado, e `working` não implica CPU/processo ocupado agora. Os arquivos de `~/.claude/jobs/<id>/` não são interface estável. [Anthropic: agent view](https://code.claude.com/docs/en/agent-view#list-sessions-as-json)

A mesma documentação registra supervisor e transições de processo no Windows. `CLAUDE_CONFIG_DIR` separa instâncias e sessões. Isso não demonstra que uma consulta nativa alcance uma instância WSL ou outra configuração. [Anthropic: agent view](https://code.claude.com/docs/en/agent-view)

O Agent SDK oferece `list_sessions()` e `get_session_info()` para histórico local, lendo arquivos de transcript sem API nem subprocesso. Os resultados incluem campos como resumo e primeiro prompt; a função não é inventário de processos vivos e sua adoção exigiria projeção de privacidade. Não é necessário adicionar SDK apenas para reproduzir o CLI de estado já usado. [Anthropic: session browser](https://platform.claude.com/cookbook/claude-agent-sdk-05-building-a-session-browser)

**Limite de domínio:** o Glossário atual define sessão como execução local e Encerrada como saída do processo. O estado durável background introduz outra dimensão. Antes de mudar precedência `status`/`state`, retenção ou rótulo, decidir como isso se encaixa no domínio e registrar ADR se houver mudança. Exercitar especialmente `working+idle`, `blocked` sem PID, `done` com PID e `done` sem PID. Não equiparar todos a Encerrada nem alterar o contrato silenciosamente nesta pesquisa.

## Hooks: eventos, configuração e entrega

Codex descobre `hooks.json` e `[hooks]` junto das camadas ativas, incluindo usuário/projeto; fontes são combinadas, e hooks não gerenciados exigem confiança/revisão. `commandWindows` é override específico Windows; comando usa o `cwd` da sessão. Os eventos usados hoje — `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PermissionRequest`, `PostToolUse`, `Stop`, `Interrupt` — são documentados. JSON chega por stdin com `session_id`, `cwd` e evento; `Interrupt` cobre interrupção do turno principal. `SessionEnd` é síncrono, com timeout padrão curto. [OpenAI: hooks](https://learn.chatgpt.com/docs/hooks)

**Limite Codex:** a página consultada não estabelece aqui o shell exato de `commandWindows` nem um transporte HTTP equivalente ao Claude. Não assumir que o quoting POSIX atual serve. Validar na versão-alvo ou consultar código oficial dessa versão antes de gerar comandos nativos. `PostToolUse` não comprova que toda modalidade de ferramenta emita evento; há exclusões documentadas. [OpenAI: hooks](https://learn.chatgpt.com/docs/hooks#tool-coverage)

Claude documenta hooks de sessão/turno, `PermissionRequest`, `Notification`, `StopFailure` e demais eventos usados no app. Command hooks recebem JSON no stdin; HTTP hooks recebem o JSON inteiro no POST. Exec form, com `args`, inicia executável sem shell; no Windows deve ser executável real, como `.exe`, não `.cmd/.bat`. Shell form usa Git Bash no Windows, ou PowerShell sem Git Bash, com seleção explícita de shell possível. Hooks correspondentes rodam em paralelo. [Anthropic: hooks](https://code.claude.com/docs/en/hooks)

**Inferência de privacidade:** preferir manter coletor local que reduza o payload a metadados antes de qualquer canal. O HTTP hook direto enviaria também prompt/tool input/resposta quando presentes no evento; não é substituição neutra do datagrama atual. O observador deve terminar sem saída de decisão/contexto: observar aprovação não concede aprovação.

**Requisitos de implementação futura:** após clique, fazer merge aditivo; identificar exatamente nossas entradas; preservar desconhecidas e alterações de terceiros; remover somente o que instalamos; rollback em escrita parcial. A confiança adicional do próprio fornecedor pode continuar necessária. Tratar receptor ausente, bloqueio por política, timeout e processo encerrado sem hook como ausência/falha de observação, não prova de conclusão.

O `HookSocket` é contrato interno do monitor, não do fornecedor. Para nativo, investigar canal local com isolamento por usuário e limite de tamanho/tempo. Para WSL, testar no ambiente da distribuição escolhida; um socket Unix sob `XDG_RUNTIME_DIR` não se transforma em endpoint Windows acessível por trocar o caminho. Canal entre Windows e WSL seria outro requisito, pendente de decisão e experimento.

## Status line, wrapper e OAuth Claude

Status line recebe JSON via stdin e imprime a linha no stdout. `rate_limits` aparece após a primeira resposta para assinaturas elegíveis ou gateway, com janelas opcionais, percentual e reset em segundos; janelas vencidas são removidas. Há atualizações por eventos, resets e `refreshInterval` opcional; uma sessão precisa estar aberta para executar o comando. Windows usa Git Bash quando disponível, PowerShell caso contrário; a documentação recomenda barras `/` nos caminhos porque Bash interpreta `\`. [Anthropic: status line](https://code.claude.com/docs/en/statusline)

**Aplicação ao ADR 0002:** continuar copiando somente Limites; guardar o valor original da configuração, alimentar o comando anterior com os mesmos bytes de stdin e preservar sua saída. O wrapper atual `sh -c` não estabelece compatibilidade com comando anterior PowerShell. Testar espaços, apóstrofo, Unicode, `$`, crase, aspas, caminhos absolutos e stdin com CRLF/linhas vazias. Se terceiro substituir a status line, mostrar desconectada e não reinstalar sozinho. Ausência de janela não é zero de consumo. Campos novos como `spend_limit` precisam de decisão explícita de escopo.

O uso de `GET /api/oauth/usage` permanece classificado como dependência não documentada no [ADR 0002](../../../docs/adr/0002-limites-claude-status-line-e-oauth-opt-in.md) e no [adaptador Claude](../../../src-tauri/src/ai/claude.rs). Não foi encontrado contrato público desse endpoint nas páginas consultadas. As regras públicas de autenticação reservam OAuth ao uso ordinário dos apps Anthropic e restringem intermediação de credenciais por terceiros; opt-in não comprova permissão ou suporte ao endpoint. [Anthropic: autenticação e credenciais](https://code.claude.com/docs/en/legal-and-compliance#authentication-and-credential-use)

Esta pesquisa não altera o opt-in existente: desligado por padrão, token apenas em memória, nunca renovar; token expirado/recusado volta à status line. Para Windows, formato/localização de credencial é uma pendência, não autorização para descobrir ou ler credenciais reais. A migração não deve ampliar acesso nem apresentar OAuth como API pública garantida.

## Plano de evidência para implementação posterior

Todas as variantes abaixo são experimentos futuros, **não executados**. Capturar em ambiente de teste e sanitizar em memória antes de gravar, conforme [sanitização](../../../docs/agents/sanitizacao.md). Cada fixture deve identificar versão do agente, Windows/WSL/distribuição, origem real sanitizada ou variante sintética e campos substituídos. Usar IDs, caminhos/projetos e conteúdo sintéticos estáveis. SQLite deve ser reconstruído, nunca copiado após apagar linhas.

| Seam/contrato | Cenários mínimos | Critério verificável |
| --- | --- | --- |
| `AppServer`/`Connection` | Stdio nativo e WSL escolhidos; handshake; erro/EOF; consulta de limites e update; schemas por versão | Sem iniciar turno, retomar thread, renovar credencial ou persistir respostas completas; fonte/idade corretas |
| Fonte de sessões Codex | Duas instâncias independentes, app/CLI, servidor próprio/compartilhado; thread carregada ociosa; descarregamento; processo morto; permissão negada | Cobertura medida por ambiente; `notLoaded` e banco histórico não viram afirmação global sobre processo |
| `Processes` Claude | Interativa/background; PID opcional; estados combinados; `--all`; comando indisponível; homes separados | Estados compatíveis com decisão de domínio; metadados sanitizados; leitura falha preserva última observação com idade |
| `HookInput`/`HookSocket` | Payload mínimo e com sentinelas sintéticas de conteúdo; excesso/truncamento; receptor ausente; duplicação/ordem; término sem evento | Projeção antes de transporte/cache/log; orçamento de tempo; app não bloqueia o agente nem emite aprovação |
| Configuração de hooks | Arquivo ausente/inválido, entradas de terceiros, mudanças após instalar, caminhos especiais, confiança/política | Nenhuma escrita antes do clique; adição e remoção próprias; reversão e rollback preservam terceiros |
| Wrapper da status line | Sem original, original Bash/PowerShell, saída ANSI, stdin idêntico, execução simultânea/cancelada, substituição por terceiro | Linha anterior preservada; somente Limites chegam ao cache; falha não instala silenciosamente outra configuração |
| OAuth opt-in | Porta fake com token sintético: desligado, válido, expirado, recusado e rede indisponível | Zero leitura antes do opt-in; zero refresh; token não aparece em log/cache/notificação nem fixture real |

Não houve build, teste ou prova de execução nesta etapa. O artefato contém somente análise do código versionado e documentação pública. A decisão de ambiente e os experimentos acima permanecem necessários antes de prometer suporte Windows.
