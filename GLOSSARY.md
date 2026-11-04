# kvojps-monitor

Aplicativo pessoal de bandeja que reúne num só lugar, de forma enxuta, dados que hoje exigem abrir vários apps e abas: uso de IA, sessões de agentes, atividade no GitHub, tarefas do Trello ou Jira e saúde da rede.

## Language

**Indicador**:
O ícone do app na barra superior do Ubuntu ou na área de notificação do Windows; ponto de entrada do Painel.
_Avoid_: Doca, dock, ícone de bandeja

**Painel**:
A janela que abre a partir do Indicador e exibe os Módulos.
_Avoid_: Doca, popup, dashboard

**Modo do Painel**:
A forma de apresentação do Painel: completo ou reduzido. O reduzido apresenta resumos dos Módulos com acesso ao completo.
_Avoid_: Minimizado, movimento reduzido, aparência

**Modo preferido do Painel**:
O Modo do Painel escolhido explicitamente pelo usuário e lembrado para reaberturas. Uma Apresentação contextual pode exibir outro modo temporariamente sem substituir essa escolha.
_Avoid_: Modo atual, última tela, apresentação contextual

**Apresentação contextual**:
A exibição temporária do Painel completo para Configurações, detalhes de Módulo ou notificação, com retorno à apresentação anterior e preservação do Modo preferido do Painel.
_Avoid_: Troca de Modo, nova Âncora

**Fixação do Painel**:
A preferência “Manter no topo”, que mantém o Painel acima de janelas comuns sem exigir que ele tenha foco. É independente do Modo do Painel.
_Avoid_: Foco obrigatório, modo reduzido, bloqueio da janela

**Relógio do Painel**:
A indicação de hora e data locais do sistema apresentada na área global do Painel, nos modos completo e reduzido.
_Avoid_: Módulo Relógio, horário da fonte de dados

**Módulo**:
Uma área do Painel dedicada a um conjunto de informações (IA, Agentes, GitHub, Tarefas, Rede, Sistema).
_Avoid_: Widget, aba, card

**Tarefas**:
O Módulo que apresenta o quadro e os itens do Provedor de tarefas ativo, identificado como Trello ou Jira.
_Avoid_: Módulo Trello, Módulo Jira

**Provedor de tarefas**:
O serviço escolhido como fonte do Módulo Tarefas: Trello ou Jira. Apenas um está ativo por vez.
_Avoid_: Provedor de IA, integração ativa sem indicar o Módulo

**Âncora**:
O último Módulo com que o usuário interagiu no Painel; é lembrada entre execuções e restaurada ao reabrir. Abrir pelo clique numa notificação não a altera.
_Avoid_: Último Módulo, view

**Configurações**:
A área do Painel onde o usuário informa credenciais e preferências de cada Módulo.
_Avoid_: Settings, preferências

**Provedor de IA**:
Um serviço de IA pago por assinatura cujo uso é acompanhado (ex.: Claude, ChatGPT/Codex).
_Avoid_: Modelo, IA paga

**Limite de uso**:
A cota de um Provedor de IA dentro de uma janela de tempo (ex.: janela de sessão, semanal), com quanto já foi consumido e quando reinicia.
_Avoid_: Limite de tokens, quota

**Sessão de agente**:
Uma execução local de um CLI de agente (Claude Code, Codex) nesta máquina; a cada momento está em um dos Estados da sessão.
_Avoid_: Sessão, agente, run

**Estado da sessão**:
A situação atual de uma Sessão de agente: Em andamento (o agente trabalha), Aguardando você (precisa de aprovação ou resposta para seguir), Turno concluído (terminou e a sessão segue aberta; inclui interrupção pelo usuário), Com falha (o turno terminou em erro) ou Encerrada (o processo saiu).
_Avoid_: Finalizada, idle, status, Trabalhando

**Segurança da rede**:
A avaliação de riscos da conexão atual (criptografia, DNS, portal cativo, gateway), expressa como Sem alertas, Atenção ou Risco; nunca afirma que a rede é segura.
_Avoid_: Saúde da rede, rede segura

**Qualidade da rede**:
O desempenho da conexão atual (latência, jitter, perda, velocidade), expresso como Boa, Regular ou Ruim junto dos números medidos.
_Avoid_: Saúde da rede, status da rede

**Repo em destaque**:
O repositório com mais commits seus nos últimos 7 dias, exibido no Módulo GitHub.
_Avoid_: Repo principal, top repo

**Sequência de contribuições**:
Os dias consecutivos com ao menos uma contribuição no GitHub, terminando hoje ou ontem; um dia de hoje ainda sem contribuição não a interrompe.
_Avoid_: Streak, ofensiva, hot streak
