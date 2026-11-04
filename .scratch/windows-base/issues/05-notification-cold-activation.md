# Clique na Central não reabre o monitor encerrado

Status: ready-for-human
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

### Aceite interativo e contexto do processo

O usuário localizou o Indicador nos ícones da área de notificação e confirmou clique esquerdo abrindo o Painel em cffd77f. O aviso da Store dizia não haver aplicativo para o protocolo; QueryUriSupportAsync também retornava NotSupported. SHChangeNotify não mudou a resposta. Os processos lançados pelas ferramentas tinham token restrito, sem elevação, AppContainer ou virtualização de registro habilitados. Isso não identifica, sozinho, a causa de uma diferença de visibilidade.

Após o usuário sair pelo Indicador e abrir o atalho temporário pelo menu Iniciar, conferiu-se EXE correto, um processo e pai Explorer. A mesma consulta passou a Available. Essa diferença exige que a prova interativa use uma abertura normal pelo Windows; os probes de ShellExecute e COM iniciados pelas ferramentas não são substitutos do aceite gráfico. Não foi necessário alterar assinatura, políticas, permissões ou confiança. A causa precisa da divergência do contexto das ferramentas não foi isolada.

Depois de reconectar os hooks por clique explícito, os 13 comandos foram conferidos contra o EXE atual antes de atualizar o helper privado. Os alertas antigos deste monitor de teste foram limpos. Uma notificação nova, com protocolo fixo e sem sentinelas privadas no XML, apareceu; o usuário confirmou clique no banner abrindo Agentes sem Store. A Âncora controlada GitHub permaneceu salva.

Outro evento sintético produziu um novo alerta. Após sete segundos, o processo foi encerrado e a contagem confirmada em zero. O usuário clicou exclusivamente nesse alerta na Central e confirmou reabertura em Agentes. A leitura posterior confirmou uma instância do EXE correto e Âncora GitHub preservada. O usuário desligou os hooks ao concluir; a reversão foi verificada contra o estado inicial. Esses resultados não comprovam o futuro instalador nem dispensam sua identidade e limpeza de associações.

### Limpeza ao encerrar

O usuário optou por deixar reconexão e suspensão/retomada pendentes. Depois de confirmar hooks desligados, foram limpos os alertas somente do monitor e encerrada exclusivamente a instância do artefato. A contagem final de processos é zero e o valor de início automático continua ausente. O atalho foi removido após conferir hash e manifesto de propriedade; as entradas de identidade, COM e protocolo foram removidas somente após conferir seus comandos contra o EXE temporário. As visões HKCU, HKU explícita do usuário e HKLM consultadas não contêm mais o protocolo. QueryUriSupportAsync ainda informou Available após remoção e SHChangeNotify; esse resultado pode refletir estado mantido pelo ativador e não foi usado como comprovação de limpeza. Não se reiniciou Explorer nem se alteraram caches/políticas do sistema.

A Âncora anterior ao controle foi restaurada com o app encerrado, conferindo que ainda era o valor controlado e preservando todos os outros campos. Os EXEs substituídos e o último artefato experimental foram apagados; o diretório privado de probes, logs, hashes, sessões sintéticas e atalhos preparados foi removido. Nenhuma instalação persistente ou novo instalador foi criado nesta continuação. Configurações e credenciais de usuário foram preservadas. O roteiro mantém os demais cenários não exercitados explícitos.
