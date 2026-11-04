# Limites de IA no Windows

Type: task
Status: claimed

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
- Um harness temporário com entrada sintética encontrou emissão de `VoidTaskResult` no stdout. Após silenciar esse retorno, a saída anterior foi preservada, sem stderr. O EXE GUI anterior continuou sem copiar a entrada: a porta Windows agora lê o handle nativo real, mantendo prazo e limite. O harness não alterou a status line real e removeu seus arquivos.
- Fontes Codex reais responderam a conta e limites na CLI `0.154.0`; apenas flags de disponibilidade/sucesso foram expostas. Claude Code ausente no PATH; nenhum arquivo de credencial Claude foi consultado.
