# Tarefas com um Provedor de tarefas ativo

O contrato aprovado para a evolução do produto reúne Trello e Jira no Módulo **Tarefas**, com uma identidade estável e apenas um Provedor de tarefas ativo. A posição do Módulo e a Âncora permanecem as mesmas; credenciais e quadro selecionado pertencem ao contexto de cada provedor. Isso evita duplicar Módulos e perder a orientação do usuário a cada troca, ao custo de migrar a identidade Trello existente e coordenar explicitamente a desativação de uma fonte antes de ativar outra.

Trocar preserva a conexão e o quadro para retorno, mas interrompe coleta e limpa leitura/cache da fonte desativada; desconectar também remove sua credencial local. Respostas de outro provedor, conta ou quadro não podem atualizar o contexto atual, e uma escrita remota com resultado desconhecido exige releitura antes de nova ação, sem reenvio automático.

Esta decisão registra o contrato a implementar. A resolução e os critérios de aceite estão em [Como escolher Trello ou Jira preservando estado e privacidade?](../../.scratch/jira-windows/issues/04-provider-contract.md); o suporte de plataforma continua sendo definido no mesmo mapa.
