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

Candidato sem assinatura, 10.028.032 bytes, mantido para esse aceite em `%TEMP%/pc-monitor-ai-ab09af5-qa/kvojps-monitor.exe`. SHA-256: `DA8CF3EBD0E4C75DF134C0FB92303CF145DF83BC331EDCFE87B3010CF9000FD9`. Ainda não executado na sessão gráfica. Roteiro em [Limites de IA Windows](../../../docs/ia-windows-validacao.md).

Ferramentas portáteis de Rust, arquivos baixados para formatação e diretórios dos probes locais foram removidos. A pasta da candidata contém somente o EXE. A versão anterior permanece ativa e útil até a substituição validada; não remover seu arquivo enquanto estiver em uso.

Pendências: autorização para abrir a candidata, leitura Codex no Painel e smoke de Jira/arraste/X/Escape; aceite Claude real condicionado à disponibilidade do CLI. A descoberta de Sessões de agente continua parcial: a composição Windows não usa o adaptador Codex de `/proc` e a consulta Claude depende da fonte externa. Esse próximo incremento exige evidência de execução viva, sem promover histórico a agente ativo. Trello e aceite gráfico Ubuntu continuam adiados conforme orientação do usuário.
