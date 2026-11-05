# Limites de IA no Windows

Type: task
Status: ready-for-human
Assignee: maintainer
Parent: ../spec.md

## Escopo

Ativar o Módulo IA no Windows 11 x64 conforme a especificação de Jira/Windows. Em 04/11/2026, o usuário priorizou IA e adiou o aceite gráfico Trello/Ubuntu. A implementação compartilhada existente permanece preservada.

## Diagnóstico

O coletor estava condicionado a Linux e IA figurava entre os Módulos indisponíveis no Windows. O adaptador Codex usava um nome de comando sem tratar o launcher npm Windows; o wrapper Claude era POSIX. Não é necessário especular sobre falha da conta para explicar esse bloqueio explícito.

## Aceite

- Codex nativo lê limites pelo app-server existente, sem abrir terminal, criar conversa ou solicitar nova autenticação.
- Claude status line conecta apenas por clique, encadeia a anterior e restaura sem sobrescrever alterações de terceiros.
- OAuth opcional explicitamente indisponível no Windows; nenhum acesso às credenciais Claude nessa plataforma.
- Configurações distinguem fonte ausente, falta de leitura e indisponibilidade.
- Cache/logs só contêm metadados e limites. Payload bruto da status line nunca vai ao disco.
- CI nativa verifica processo e modo silencioso do EXE release; aceite gráfico permanece separado.

## Evidência inicial

`npm exec vitest -- run src/lib/ai/AiSettings.test.ts --pool=threads --maxWorkers=1`: 15 testes passaram e o novo teste falhou porque o opt-in ainda era oferecido. Após a mudança, 16 passaram. Rust local indisponível nesta máquina; a validação nativa usará CI, preservando as proteções do Windows.

## Iteração nativa

- UI completa: 393 testes passaram; Svelte sem erros/avisos. O segundo teste novo falhou antes de corrigir o indicador Codex sem leitura.
- CI [41929410363](https://github.com/lucasvss2/pc-monitor/actions/runs/41929410363), `495e833`: 270 testes Rust Windows passaram, dois ignorados; os dois testes nativos da status line falharam. Linux automatizado passou. Não há aceite gráfico Ubuntu nesta etapa.
- A política local PowerShell é `Restricted`. O wrapper passou a usar um comando gerado em `EncodedCommand` que lê dados JSON; não executa código carregado de arquivo, não usa `Bypass`, não altera políticas.
- Um harness temporário com entrada sintética encontrou emissão de `VoidTaskResult` no stdout. Após silenciar esse retorno, a saída anterior foi preservada, sem stderr, mas o cache não foi produzido. Isso sozinho não isolou uma falha de stdin. O harness não alterou a status line real e removeu seus arquivos.
- Fontes Codex reais responderam a conta e limites na CLI `0.154.0`; apenas flags de disponibilidade/sucesso foram expostas. Claude Code ausente no PATH; nenhum arquivo de credencial Claude foi consultado.

Na CI `41931513580`, o primeiro job Windows expirou no teste de hooks antes de alcançar a status line. Na repetição, hooks passaram e os testes da status line reproduziram falha de preservação da entrada/cache. O harness isolou 103 bytes enviados e 106 recebidos: o StreamWriter que o .NET Framework cria para stdin acrescentava BOM UTF-8. Mantendo a mesma code page e removendo somente o preâmbulo dessa instância de encoding, os 103 bytes chegaram intactos e o EXE GUI anterior gravou o cache sanitizado. A alteração experimental do leitor Windows foi revertida por não ser necessária para a causa confirmada. Nenhuma política ou code page foi mudada.

Na CI [41933649688](https://github.com/lucasvss2/pc-monitor/actions/runs/41933649688), `0ec3d05`, os testes nativos da status line passaram em PowerShell e Git Bash, preservando stdin/stdout e a projeção do cache. Hooks, 270 testes de núcleo e cofre real sintético passaram. A etapa Codex revelou que seu `CODEX_HOME` isolado precisava existir. Um probe local sem login reproduziu encerramento com diretório ausente e inicialização bem-sucedida com diretório existente; a CI passou a criar apenas essa pasta temporária antes do teste. O aplicativo não precisa alterar configurações ou criar autenticação para essa correção do harness.

## Resultado automatizado e candidato

CI [41934400956](https://github.com/lucasvss2/pc-monitor/actions/runs/41934400956), revisão `ab09af5`, passou nos dois alvos. Windows: 393 testes UI; 270 testes de núcleo; testes de subprocesso hooks/status line; cofre real com credencial sintética removida; Codex npm `0.154.0` inicializando em home isolado sem login; 13 testes de fixtures; clippy, formatação e builds. O EXE GUI release passou novamente nos testes de hooks e da status line em PowerShell e Git Bash. Isso não substitui o aceite gráfico Windows 11.

Candidato sem assinatura, 10.028.032 bytes, mantido para esse aceite em `%TEMP%/pc-monitor-ai-ab09af5-qa/kvojps-monitor.exe`. SHA-256: `DA8CF3EBD0E4C75DF134C0FB92303CF145DF83BC331EDCFE87B3010CF9000FD9`. Executado pela interface após autorização do usuário; aceite visual pendente. Roteiro em [Limites de IA Windows](../../../docs/ia-windows-validacao.md).

Ferramentas portáteis de Rust, arquivos baixados para formatação e diretórios dos probes locais foram removidos. A instância anterior foi encerrada para a substituição autorizada.

Na sessão Windows 11, a candidata iniciou como única instância e criou um filho Codex app-server. O cache sanitizado do próprio aplicativo confirmou leitura recente com fonte `appServer`; a projeção de evidência registra somente essas flags, sem números ou identidade. A automação não obteve janela endereçável, então a conferência visual segue assistida pelo usuário.

Em 05/11/2026, o usuário confirmou a leitura semanal do Codex no Painel. A captura também confirmou que o Jira carregou. A leitura local continha a janela semanal e não incluía a janela de 5 h. A comparação com o Codex revelou que o Painel mostrava a parcela consumida. Os instrumentos de IA passaram a mostrar o restante, com rótulos explícitos nas janelas de 5 h e semanal. O teste usa a fixture sanitizada existente.

A [CI 42075201282](https://github.com/lucasvss2/pc-monitor/actions/runs/42075201282) passou na revisão `4e9305e`, com 393 testes de interface, incluindo 46 do módulo de IA, e verificação nativa do EXE release. O executável sem assinatura tem SHA-256 `936ED90CDCF6AF4E16C7689C6F8AE5CF6B15A8A7575C5B44ACB5849058705A8A` e está em `%TEMP%/pc-monitor-ai-4e9305e-qa/kvojps-monitor.exe`.

Após autorização do usuário, a revisão corrigida iniciou como única instância e obteve uma leitura do app-server após a abertura. O usuário confirmou que a porcentagem restante semanal no Painel coincide com a do Codex. Duas versões temporárias anteriores foram removidas após verificar o hash, a ausência de processos ativos e as referências do Windows.

Pendências: repetir arraste, X e Escape. O aceite Claude real depende da disponibilidade do CLI. A descoberta de Sessões de agente continua parcial no Windows. Esse próximo incremento exige evidência de execução viva. Trello e aceite gráfico Ubuntu continuam adiados conforme orientação do usuário.

## Apresentação automática por ferramenta

Em 05/11/2026, o usuário confirmou que o Painel deve adaptar a apresentação às ferramentas instaladas e às janelas retornadas por cada uma. A ausência de uma ferramenta libera seu espaço, a janela semanal sozinha ocupa o mostrador principal e as Configurações identificam as janelas detectadas.

A composição Windows chamava a consulta do Claude mesmo quando seu CLI estava ausente. A consulta agora é pulada nessa situação. Agentes informa quando a descoberta automática está indisponível e mantém a opção de Detecção instantânea com consentimento explícito. A descoberta periódica de sessões vivas do Codex ainda requer um adaptador Windows. Rede e Sistema seguem sem coletores Windows registrados. A conectividade básica usada pelo ciclo de vida não representa as métricas do módulo Rede.

A [CI 42081953119](https://github.com/lucasvss2/pc-monitor/actions/runs/42081953119), revisão `aae49d3`, passou nos testes de interface e de núcleo, incluindo 271 testes Rust Windows, e na verificação nativa do EXE release. A prévia com dados sintéticos confirmou o mostrador semanal principal, a ausência do cartão Claude e a orientação de sessões.

No Windows 11 local, a abertura autorizada do EXE sem assinatura foi recusada pela Integridade de Código (eventos 3077 e 3033). O aceite gráfico do EXE segue pendente. A versão anterior voltou a rodar e a cópia temporária bloqueada foi removida. O candidato continua disponível como artefato da CI.

## Prova isolada de execução viva

Um `codex exec` nativo com home de teste e provedor HTTP em loopback gravou um rollout e sua referência no SQLite. O Restart Manager associou o arquivo ao processo durante a requisição de teste. Após encerrar somente o processo de teste, a associação desapareceu apesar de o histórico continuar no disco. Foram inspecionados apenas metadados de origem e presença no home sintético. O diretório foi removido.

A prova cobre uma execução `exec` e confirma um sinal que distingue execução viva de histórico. A consulta periódica continua sem adaptador Windows. Para integrar o sinal, falta verificar o CLI interativo, a atribuição entre execuções simultâneas, as permissões e o custo de verificar históricos grandes. [API RmGetList](https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/nf-restartmanager-rmgetlist)
