# Limites de IA no Windows

Este incremento habilita o coletor de limites no Windows 11 x64. O suporte continua experimental até o aceite gráfico; não conclui as etapas de Sessões de agente, Rede, Sistema ou distribuição. Trello e aceite gráfico Ubuntu foram adiados pelo usuário nesta sessão; os testes compartilhados continuam na CI.

## Fontes e configuração

- **Codex:** CLI nativo no `PATH`, autenticado pela própria ferramenta. O monitor inicia um app-server sem terminal, consulta conta/limites e usa a leitura local de rollout como alternativa. Não cria conversas, não faz login nem renova tokens. Aceita `codex.exe` e a instalação npm oficial com dependência nativa Windows x64, incluindo o diretório local `.bin`. Launchers personalizados e WSL não estão cobertos. `CODEX_HOME` é respeitado; sem ele, usa `.codex` na pasta pessoal.
- **Claude Code:** CLI nativo no `PATH`; após **Conectar ao Claude Code**, o monitor instala um wrapper PowerShell local que copia somente `rate_limits` e executa a status line anterior. Sem Git Bash, o comando anterior usa PowerShell; com Git Bash, usa o executável Git Bash identificado, sem escolher o `bash.exe` do WSL. `CLAUDE_CONFIG_DIR` é respeitado. Não há execução do comando anterior durante a conexão.
- **OAuth Claude:** indisponível nesta versão Windows, tanto na UI quanto no comando do núcleo. O coletor Windows não lê `.credentials.json` nem restaura a escolha desse opt-in. A leitura de limites pela status line continua disponível.

Uma instalação de CLI que altere o `PATH` pode exigir reabrir o monitor a partir de uma sessão que já tenha o ambiente atualizado. Fonte ausente e falta de leitura são estados distintos; nenhuma janela ausente equivale a 0% de uso. O Claude pode não enviar `rate_limits` antes da primeira resposta ou em contas sem essa informação.

O wrapper preserva bytes de entrada e saída do comando anterior, incluindo Unicode, quebra de linha final e sequências ANSI. Limita a entrada a 1 MiB e cada subprocesso a dois segundos. Um comando anterior que exceda esse prazo pode ficar sem saída; nenhum conteúdo bruto é gravado ou incluído em erros. O comando gerado usa `EncodedCommand` e lê uma configuração JSON local; não executa um arquivo `.ps1`, não usa `Bypass` e não altera a política de execução. Restrições organizacionais a programas continuam aplicáveis. Desconectar restaura a configuração anterior sem sobrescrever uma substituição feita por terceiros.

O app-server fica num Job Object com encerramento ao fechar o monitor, evitando processos filhos residuais. O cache de limites e logs usa diretórios locais da aplicação, inclusive quando não há diretório de estado no Windows. Configurações e leituras reais não são fixtures de teste.

## Verificação

As portas existentes de processo, arquivos, relógio, persistência e snapshots da UI são a fronteira de teste. Os testes Windows de subprocesso usam diretórios temporários e sentinelas sintéticas; não instalam status line nem hooks na conta real. A CI repete o teste da status line com o EXE GUI release, além de validar inicialização stdio do Codex npm fixado em uma pasta e um `CODEX_HOME` isolados, sem login.

Em 04/11/2026, a verificação local pela CLI instalada `0.154.0` confirmou `initialize`, `account/read` e `account/rateLimits/read`, com conta disponível e resposta multibucket. A projeção em memória registrou somente sucesso/ausência de erro; identidade, conteúdo e números privados não foram gravados. Claude Code não foi encontrado no `PATH` desta sessão. Isso valida disponibilidade da fonte Codex, sem substituir o teste do novo aplicativo.

Roteiro gráfico, após abrir o candidato aprovado:

1. Verificar instrumentos Codex e **Configurações → IA**. IA não deve aparecer como Módulo indisponível. Atualizar deve trazer uma leitura ou explicar a ausência/falha da fonte.
2. Confirmar que Claude ausente aparece como tal e que OAuth mostra indisponibilidade sem opt-in.
3. Em ambiente com Claude Code nativo, guardar uma referência local da configuração anterior sem publicá-la. Clicar em **Conectar ao Claude Code**, abrir uma sessão elegível e conferir a status line anterior e os limites no monitor. Não modificar configurações por ferramentas para simular esse clique.
4. Desconectar pela UI e conferir restauração e remoção dos arquivos próprios do wrapper. Não alterar entradas de terceiros nem publicar caminhos/configuração reais.
5. Reabrir o monitor e conferir Jira preservado, arraste, X e Escape. Ao encerrar pelo Indicador, o app-server que o monitor iniciou deve terminar; outras sessões Codex permanecem intactas.

Resultados e pendências: [ticket de implementação](../.scratch/windows-ai/issues/01-native-limits.md).

Fontes primárias: [Codex app-server](https://developers.openai.com/codex/app-server), [status line Claude no Windows](https://code.claude.com/docs/en/statusline#windows-configuration), [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects). O formato de pacote foi conferido também no launcher da CLI instalada; não depende de executar um shell npm intermediário.
