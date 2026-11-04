# Validação da base Windows

Este guia acompanha a base da [especificação](../.scratch/jira-windows/spec.md). Ainda não é suporte oficial Windows completo. Limites de IA, Rede e Sistema permanecem explicitamente indisponíveis; descoberta completa de CLIs e status line Windows continuam na adaptação. Tarefas/Jira foi implementado, com conexão/leitura/troca confirmadas em um recorte assistido descrito em [Validação de Tarefas](tarefas-validacao.md); esse resultado não encerra os demais critérios. Painel reduzido e geometrias continuam no plano aprovado.

## Verificações automáticas

A CI executa Ubuntu e Windows separadamente, mantendo check/testes/build da UI, fmt, clippy, testes Rust, fixtures de desenvolvimento e build Tauri sem empacotamento. No Windows, testa adaptadores de credencial, pipe, callbacks de energia e ativação COM. A prova do cofre real usa uma credencial sintética isolada, confere persistência local sem roaming e a remove ao terminar.

Testes de configuração de hooks usam diretórios temporários. Nenhum instala hooks na conta real. A prova de subprocesso usa o gerador de comando da aplicação, caminho temporário com espaços/caracteres especiais e JSON sintético com sentinelas de conteúdo, conferindo saída silenciosa e projeção de metadados.

O hook Windows tem orçamento externo de três segundos para acomodar a inicialização do PowerShell; o binário mantém limite de 100 ms e 64 KiB para stdin e transporte sem espera por receptor ausente/ocupado. O teste do executável usa o mesmo limite externo. Ubuntu conserva o orçamento anterior de um segundo. Falha do hook não aprova nem bloqueia a ação do agente; a leitura periódica continua sendo a alternativa, com cobertura Windows ainda parcial neste incremento.

A CI repete o teste do hook com o executável de release que usa subsistema GUI, antes de oferecer o artefato. Assim, o teste não se limita ao binário de console do perfil debug. SQLite embutido fica restrito ao Windows; Ubuntu conserva a dependência de sistema anterior.

O runner `windows-2025` comprova execução das APIs no Windows do runner. Não substitui uma sessão gráfica em Windows 11 x64 nem aceite Ubuntu/GNOME/Wayland. Depois das verificações, o workflow oferece um executável experimental sem assinatura como artefato temporário, sem publicar release.

Durante este incremento, os coletores ainda inativos no Windows e suas portas preservadas têm exceções localizadas somente para `dead_code`, com motivo no código. A CI conserva `-D warnings` para os demais lints e continua verificando Ubuntu integralmente. Remover essas exceções ao ativar os coletores nas próximas etapas; elas não validam suporte às fontes ausentes.

## Aceite em sessão gráfica

Registrar versão de SO/app, cenário, resultado e limitação sem valores privados ou capturas brutas. Usar sessões/projetos de exemplo e seguir [sanitização](agents/sanitizacao.md).

1. Clique esquerdo na bandeja abre/ativa o Painel existente sem outra instância; direito abre menu. Pela revisão de foco aprovada durante o aceite de Tarefas, clicar fora mantém o Painel aberto, inclusive nas Configurações. X/Escape fecham o Painel preservando o Indicador; Escape resolve primeiro um diálogo interno. Início automático é opcional e silencioso.
2. Confirmar recusa sem cofre e roundtrip de credencial sintética no Credential Manager local. Não usar nem imprimir credenciais reais como evidência.
3. Após clique explícito nas Configurações, instalar hooks para CLIs de teste. Conferir preservação de terceiros, evento real e reversão. Histórico sem processo vivo não valida detecção; não ampliar cobertura antes de medir o adaptador Windows.
4. Gerar evento de atenção de exemplo. Notificações mostram apenas fonte/evento/estado/duração; nomes de projeto/sessão e conteúdo ficam fora do texto e dos argumentos de ativação.
5. Com outra Âncora salva, fechar o Painel e clicar no banner: abre Agentes sem mudar a Âncora. Repetir deixando a notificação ir à Central antes do clique. Conferir também processo reiniciado, políticas de notificação e Não Incomodar. Testar entrega e clique separadamente: callback COM unitário não comprova esses cenários.
6. Suspender/retomar e alternar conectividade, sem duplicar instâncias ou retomar respostas obsoletas. No Ubuntu, repetir os cenários existentes de SNI, foco e notificação.

Se entrega/ativação falharem, manter a etapa pendente e investigar o adaptador. EXE sem assinatura não tem execução garantida por toda política do Windows. Não desativar proteções, remover marca de origem ou instalar confiança artificial para passar o aceite.

## Evidência desta máquina

Em 04/11/2026, o build local encontrou erro 4551 de Controle de Aplicativo ao executar um build script gerado pelo Rust. O usuário escolheu validar pela CI. A política permaneceu intacta; cofre e notificações ainda não haviam sido executados, portanto o bloqueio não comprova falha nesses adaptadores. Resultados por commit e pendências: [ticket de implementação](../.scratch/windows-base/issues/01-native-foundation.md).

Rotas técnicas: [CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew), [ativação de toast Win32 por COM](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/win32_tile_badge_notif/respond-to-toast-activations), [conectividade nativa](https://learn.microsoft.com/en-us/windows/win32/api/netlistmgr/nf-netlistmgr-inetworklistmanager-get_isconnectedtointernet) e [bandeja Tauri](https://v2.tauri.app/learn/system-tray/). O adaptador usa WinRT/COM e registro por usuário, sem exigir MSIX nem assumir que assinatura acrescentaria ativação.

Nos testes Linux, o prazo do subprocesso inclui a carga do executável GUI e segue o orçamento externo de um segundo já configurado no hook. A leitura nativa tem prova separada com descritor local mantido aberto: timeout de 100 ms, verificado abaixo de 200 ms sem o custo do loader. Os testes de subprocesso são serializados e encerram o filho se exceder o orçamento; silêncio e projeção de metadados continuam obrigatórios.

Na composição Windows ainda sem consulta periódica Codex completa, ausência de adaptador é fonte indisponível, não confirmação de encerramento. O último evento recebido por hook pode permanecer em memória com falha de consulta indicada; isso não comprova processo vivo e não satisfaz a descoberta exigida pelo mínimo. Uma leitura válida posterior ou SessionEnd explícito reconcilia o estado. Não usar a retenção de uma notificação como prova de liveness.

A ativação COM dentro do processo não comprovou abertura com o processo encerrado na sessão gráfica. O adaptador usa protocolo local de destino fixo, validado pelo banner com instância ativa e pela Central com processo encerrado; o [ticket de ativação pela Central](../.scratch/windows-base/issues/05-notification-cold-activation.md) registra reprodução, limites e necessidade de identidade válida no menu Iniciar. Nenhum resultado do atalho temporário comprova o instalador distribuível.

Na sessão de cffd77f, após uma abertura normal pelo menu Iniciar, a consulta de suporte ao protocolo passou a disponível; os processos iniciados pelas ferramentas haviam produzido resultados divergentes. Não tratar a consulta/execução no contexto restrito das ferramentas como prova de ativação gráfica. O usuário confirmou banner e Central abrindo Agentes, inclusive com processo encerrado, com Âncora diferente preservada e uma única instância. Hooks foram reconectados e depois desligados pelo usuário; a reversão corresponde ao estado inicial. Início automático ligou/desligou sem erro em 8f92ec1, ficando desligado, e o relançamento --autostart manteve uma instância e nenhum Painel. Logon real, eventos reais das CLIs, regressão gráfica Ubuntu e instalador continuam separados dessas provas.

Ao encerrar esta rodada, hooks foram desligados por clique, configurações anteriores verificadas, alertas do monitor limpos, Âncora restaurada, app encerrado e atalho/associações/artefatos temporários removidos. Reconexão e suspensão/retomada ficaram pendentes por escolha do usuário. Não Incomodar/políticas de entrega, logon real, eventos reais das CLIs, regressão gráfica Ubuntu e instalador ainda não têm aceite gráfico nesta rodada. Nenhuma proteção foi alterada.
