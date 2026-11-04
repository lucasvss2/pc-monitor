# Validação da base Windows

Este incremento inicia a [primeira etapa da especificação](../.scratch/jira-windows/spec.md). Ainda não é suporte oficial Windows completo. Limites de IA, Rede e Sistema permanecem explicitamente indisponíveis; descoberta completa de CLIs e status line Windows continuam na adaptação. Jira e Painel reduzido seguem no plano aprovado.

## Verificações automáticas

A CI executa Ubuntu e Windows separadamente, mantendo check/testes/build da UI, fmt, clippy, testes Rust, fixtures de desenvolvimento e build Tauri sem empacotamento. No Windows, testa adaptadores de credencial, pipe, callbacks de energia e ativação COM. A prova do cofre real usa uma credencial sintética isolada, confere persistência local sem roaming e a remove ao terminar.

Testes de configuração de hooks usam diretórios temporários. Nenhum instala hooks na conta real. A prova de subprocesso usa o gerador de comando da aplicação, caminho temporário com espaços/caracteres especiais e JSON sintético com sentinelas de conteúdo, conferindo saída silenciosa e projeção de metadados.

O runner `windows-2025` comprova execução das APIs no Windows do runner. Não substitui uma sessão gráfica em Windows 11 x64 nem aceite Ubuntu/GNOME/Wayland. Depois das verificações, o workflow oferece um executável experimental sem assinatura como artefato temporário, sem publicar release.

## Aceite em sessão gráfica

Registrar versão de SO/app, cenário, resultado e limitação sem valores privados ou capturas brutas. Usar sessões/projetos de exemplo e seguir [sanitização](agents/sanitizacao.md).

1. Clique esquerdo na bandeja abre/ativa o Painel existente sem outra instância; direito abre menu. Fechar/perder foco no completo livre preserva o Indicador. Início automático é opcional e silencioso.
2. Confirmar recusa sem cofre e roundtrip de credencial sintética no Credential Manager local. Não usar nem imprimir credenciais reais como evidência.
3. Após clique explícito nas Configurações, instalar hooks para CLIs de teste. Conferir preservação de terceiros, evento real e reversão. Histórico sem processo vivo não valida detecção; não ampliar cobertura antes de medir o adaptador Windows.
4. Gerar evento de atenção de exemplo. Notificações mostram apenas fonte/evento/estado/duração; nomes de projeto/sessão e conteúdo ficam fora do texto e dos argumentos de ativação.
5. Com outra Âncora salva, fechar o Painel e clicar no banner: abre Agentes sem mudar a Âncora. Repetir deixando a notificação ir à Central antes do clique. Conferir também processo reiniciado, políticas de notificação e Não Incomodar. Testar entrega e clique separadamente: callback COM unitário não comprova esses cenários.
6. Suspender/retomar e alternar conectividade, sem duplicar instâncias ou retomar respostas obsoletas. No Ubuntu, repetir os cenários existentes de SNI, foco e notificação.

Se entrega/ativação falharem, manter a etapa pendente e investigar o adaptador. EXE sem assinatura não tem execução garantida por toda política do Windows. Não desativar proteções, remover marca de origem ou instalar confiança artificial para passar o aceite.

## Evidência desta máquina

Em 04/11/2026, o build local encontrou erro 4551 de Controle de Aplicativo ao executar um build script gerado pelo Rust. O usuário escolheu validar pela CI. A política permaneceu intacta; cofre e notificações ainda não haviam sido executados, portanto o bloqueio não comprova falha nesses adaptadores. Resultados por commit e pendências: [ticket de implementação](../.scratch/windows-base/issues/01-native-foundation.md).

Rotas técnicas: [CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew), [ativação de toast Win32 por COM](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/win32_tile_badge_notif/respond-to-toast-activations), [conectividade nativa](https://learn.microsoft.com/en-us/windows/win32/api/netlistmgr/nf-netlistmgr-inetworklistmanager-get_isconnectedtointernet) e [bandeja Tauri](https://v2.tauri.app/learn/system-tray/). O adaptador usa WinRT/COM e registro por usuário, sem exigir MSIX nem assumir que assinatura acrescentaria ativação.
