# Clique na Central não reabre o monitor encerrado

Status: ready-for-agent
Assignee: Codex
Parent: ../spec.md

## Evidência e limite do teste anterior

Na sessão Windows 11, o usuário confirmou banner visível e abertura do Painel com o processo ativo após registrar um atalho temporário de identidade no menu Iniciar. O atalho tem AppUserModelID e ToastActivatorCLSID conferidos por leitura nativa; não altera assinatura ou proteções. O EXE experimental isolado não havia criado esse atalho. A distribuição deverá fornecer identidade válida e removê-la na desinstalação; o teste manual não comprova entrega pronta no instalador.

Com uma nova notificação mantida na Central, o processo foi encerrado. O usuário encontrou o alerta e confirmou que o clique não abriu o Painel. Nenhum processo foi iniciado. CoGetClassObject externo retornou REGDB_E_CLASSNOTREG apesar de LocalServer32 estar presente na visão nativa por usuário. A causa dessa divergência não foi determinada; não há evidência para atribuí-la a assinatura ou política de segurança. O teste COM da CI despacha dentro do mesmo processo e não cobre esse defeito.

## Correção em validação

Um protocolo temporário isolado conseguiu iniciar o mesmo EXE por ShellExecuteW, com remoção do registro ao terminar. O adaptador passa a usar activationType=protocol com destino fixo kvojps-monitor://agents, sem identificadores, conteúdo ou parâmetros. A associação por usuário usa o EXE entre aspas e um marcador de propriedade; recusa sobrescrever uma associação de outro dono. A aplicação valida a lista inteira de argumentos e rejeita destinos, parâmetros, fragmentos e argumentos extras. A abertura inicial e a segunda instância encaminham para open_agent, preservando a Âncora. COM permanece para identidade e compatibilidade com alertas anteriores.

Testes de argumentos e Registro isolado protegem os novos limites, mas não substituem o loop nativo que reproduziu o defeito. Antes de resolver: CI Ubuntu/Windows, clique no banner e na Central com instância ativa e encerrada, Âncora diferente preservada, instância única, início automático silencioso e limpeza do atalho/associações/artefatos temporários.

## Privacidade e fontes

Evidência registrada somente como resultados, categorias e códigos públicos de API. Caminhos pessoais, nomes de sessão/projeto e capturas brutas ficaram fora do Git. O payload de validação é sintético. Revisão do diff e metadados de commit obrigatória antes do push.

[Esquema de notificações](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-schema), [identidade de app desktop por atalho](https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid). A preservação do alerta após falha de consulta é tratada separadamente no ticket 03.

## Progresso da validação

A [CI de cffd77f](https://github.com/lucasvss2/pc-monitor/actions/runs/41867458322) passou Ubuntu e Windows, incluindo parsing restrito, Registro isolado, cofre real e hook no EXE GUI de release. O primeiro run parou no rustfmt por ordem de imports, corrigida no segundo commit.

Na máquina Windows 11, o processo e a associação foram conferidos contra o hash do artefato dessa CI; o atalho temporário foi atualizado depois de verificar a propriedade e remover somente a cópia de teste anterior. ShellExecuteW iniciou o EXE correto com processo inicialmente encerrado e argumentos fixos conferidos. A automação não encontrou o Painel visível nas sondas por script, inclusive ao repetir com processo ativo. Esse resultado não comprova ativação gráfica: comparar com o Indicador e repetir o clique humano na notificação antes de concluir ou atribuir a causa a foco, setup ou criação do WebView. Nenhuma proteção foi alterada.

O EXE GUI aprovado foi executado com três ativações inválidas sintéticas: parâmetro de sessão, argumento extra e URL externa. As três encerraram com código zero e stdout/stderr vazios antes do início da aplicação. A proteção não depende apenas do teste unitário do parser. O aceite de apresentação do Painel continua pendente.
