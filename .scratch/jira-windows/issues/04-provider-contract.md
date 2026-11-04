# Como escolher Trello ou Jira preservando estado e privacidade?

Type: grilling
Labels: wayfinder:grilling
Status: resolved
Assignee: Codex / provider_contract
Parent: ../map.md
Blocked by: 01

Prerequisites: [Quais contratos do Jira Cloud atendem ao board alvo?](01-jira-cloud-api.md)

## Question

Qual é o contrato do Módulo que permite escolher Trello ou Jira nas Configurações, com um provedor ativo, sem perder a Âncora, misturar estados de contas/fontes ou manter coleta do provedor desativado?

Decidir nome/identidade do Módulo, autenticação própria (incluindo uso pessoal versus distribuição), seleção de contexto do Jira, dados exibidos e ações/transições, UX de falha/permissão/campos obrigatórios, troca com operação em andamento, destino das credenciais/cache do provedor anterior, migração das preferências existentes e projeção permitida em disco/log/notificação. Registrar termos consensuais no glossário e ADR apenas para trade-offs duráveis.

## Comments

### Rodada inicial — 03/11/2026

Ticket reivindicado por Codex. Rodada apresentada ao usuário; todas as propostas abaixo estão **pendentes de resposta**, não são decisões:

- Nome estável “Tarefas”, fonte Trello/Jira identificada, posição e Âncora preservadas.
- Conexão pessoal Jira por site/e-mail/token de API no cofre, independente da conexão do plugin deste chat.
- Um board selecionado, inicialmente com todos os itens acessíveis no contexto desse board.
- Trocar preserva conexão/seleção para retorno, mas interrompe coleta e limpa dados exibidos/cache do provedor desativado; desconectar apaga a credencial local.
- Oferecer transições simples permitidas e abrir o Jira quando houver campos obrigatórios não atendidos pelo monitor.

Auditoria local somente leitura em andamento para conferir persistência, geração de coletas, ações em andamento e migração da identidade atual. Nenhuma mudança de produto foi implementada.

### Evidência da auditoria local — 03/11/2026

Auditoria concluída sem testes, capturas do host ou alterações no código:

- `src-tauri/src/trello/mod.rs:15` guarda chave, token, login e quadro na mesma entrada do cofre. Selecionar quadro regrava essa entrada (`:242`); as preferências gerais não têm seletor de provedor (`src-tauri/src/state.rs:49`).
- `Module::Trello` identifica Âncora, eventos e cache (`src-tauri/src/module.rs:15`, `src-tauri/src/snapshot.rs:42`, `src-tauri/src/persistence.rs:110`). Uma identidade comum futura precisa migrar a Âncora; desserialização inválida hoje descarta preferências carregadas (`src-tauri/src/state.rs:81`).
- O worker serializa HTTP e comandos (`src-tauri/src/trello/mod.rs:741`); a geração atual cobre conectividade, sem geração de provedor/conta/quadro (`:362`, `:567`). Ignorar confirmação local de PUT não desfaz uma escrita remota. Ocultar o Módulo reduz polling, mas não o desativa (`:461`).
- Cache persistido omite títulos de cards, credenciais e conta; a leitura completa pode permanecer em memória durante offline (`src-tauri/src/persistence.rs:76`, `:125`). Desconectar apaga o cofre antes de limpar leitura/cache (`src-tauri/src/trello/mod.rs:280`); erro de remoção física do cache é ignorado hoje (`src-tauri/src/persistence.rs:250`).

Depois das respostas iniciais, a rodada seguinte deve esclarecer a troca durante escrita remota, falha ao limpar cache, projeção offline do Jira, campos exibidos e critérios de aceite. Esses comportamentos permanecem abertos; não se presume cancelamento remoto, migração automática ou retenção de conteúdo privado em disco.

### Respostas da rodada inicial — 03/11/2026

O usuário aceitou as cinco propostas:

- Módulo **Tarefas** com identidade e posição estáveis, Âncora preservada e Provedor de tarefas ativo identificado como Trello ou Jira.
- Conexão Jira pessoal por site, e-mail e token de API informados nas Configurações, com credencial no cofre do sistema. Autenticação independente da conexão Atlassian do chat.
- Seleção de um quadro e exibição dos seus itens acessíveis seguindo colunas e contexto do próprio quadro, sem filtro inicial por responsável ou projeto.
- Trocar Provedor de tarefas conserva credenciais no cofre e a seleção de quadro para retorno; interrompe coleta e limpa dados exibidos/cache da fonte desativada. Desconectar é ação distinta e apaga a credencial local.
- Oferecer transições simples permitidas pelo Jira. Se houver campos obrigatórios não atendidos, explicar e abrir o item no Jira para concluir, sem formulários adicionais no monitor.

Termos consensuais registrados em [GLOSSARY.md](../../../GLOSSARY.md). O ticket continua reivindicado: faltam os casos de operação em andamento, campos exibidos, leitura offline e falha de limpeza. O código ainda implementa a identidade Trello atual; o glossário registra o domínio aprovado para a evolução.

### Respostas da segunda rodada — 03/11/2026

O usuário aceitou as quatro propostas restantes:

- Exibir chave, título, tipo, estado, prioridade, responsável e prazo quando disponíveis, além de abrir o item no navegador. Descrições, comentários e anexos ficam fora do recorte inicial.
- Aguardar resposta ou timeout da mudança de estado antes de trocar provedor, conta ou quadro. Em resultado não confirmado, liberar a troca com aviso e reler a origem antes de permitir nova ação nela, sem reenviar automaticamente.
- Manter leitura Jira somente em memória. Após reiniciar offline, informar que não há leitura disponível; conexão e quadro selecionado continuam preservados no cofre.
- Se a limpeza do cache falhar, pausar coleta, ocultar os dados e impedir a conclusão da troca, com erro e ação para tentar novamente.

As duas rodadas confirmam o contrato abaixo. O ticket é resolvido como decisão de produto, sem implementação ou validação de execução nativa.

## Answer

Resolução confirmada pelo usuário em duas rodadas em 03/11/2026.

### Identidade e contexto

O Módulo chama-se **Tarefas**, ocupa a posição atual de Trello e preserva sua Âncora. A interface identifica Trello ou Jira como Provedor de tarefas ativo. Apenas um provedor participa da coleta por vez; ocultar o Módulo não equivale a trocar ou desativar a fonte.

Cada provedor conserva sua própria conexão e seleção de quadro. Dados e operações pertencem ao contexto composto por provedor, conta/site e quadro; não podem ser conciliados apenas por nome ou ID do item. Na evolução de uma instalação existente, Trello permanece selecionado e a Âncora antiga é migrada para Tarefas, preservando credenciais, quadro e demais preferências.

### Conexão e leitura Jira

A integração inicial é pessoal e usa a própria conta do usuário: site Jira Cloud, e-mail e token de API informados nas Configurações e guardados com o contexto no cofre do sistema. O acesso não reutiliza credenciais do plugin Atlassian do chat. Recusar salvar sem cofre; não usar arquivo alternativo nem persistir segredos em preferências, snapshots, cache, logs ou notificações. Expiração ou revogação pede atualização explícita da credencial.

Selecionar um quadro e ler seus itens acessíveis, seguindo o contexto e as colunas desse quadro, sem filtro adicional inicial por responsável ou projeto. Preservar o que o serviço estabelece como conteúdo do quadro; não substituir por uma consulta de projeto nem inferir Scrum/Kanban a partir de `simple`. Sem acesso ou configuração suficiente, explicar a indisponibilidade; não apresentar erro de leitura como quadro vazio nem fabricar colunas.

Exibir chave, título, tipo, estado, prioridade, responsável e prazo quando presentes. Ausência de campo não vira valor inventado. Conteúdo de descrição, comentários e anexos não é coletado para essa apresentação. O item pode ser aberto no navegador. Ranking e administração de backlog/sprints não são novas ações desse contrato.

### Transições e escritas em andamento

Consultar e oferecer transições permitidas para o item. Coluna, status e transição são distintos; quando houver mais de uma transição aplicável, apresentar as opções para escolha. Se a transição exigir campos não atendidos, explicar e abrir o item no Jira, sem preencher valores por suposição. A confirmação do serviço determina sucesso; recusa ou conflito exige atualização da leitura.

Enquanto houver escrita em andamento, impedir mudança de contexto (provedor, conta ou quadro) e desconexão até resposta ou timeout limitado. Timeout não significa cancelamento nem falha remota comprovada: mostrar **resultado não confirmado**, permitir sair/trocar contexto e exigir releitura da origem antes de permitir outra ação nela. Nunca reenviar automaticamente uma transição cujo resultado seja desconhecido. Se a releitura falhar, manter ações de escrita indisponíveis nesse contexto até conseguir atualizá-lo.

### Trocar e desconectar

Trocar conserva a credencial no cofre e a seleção de quadro para retorno, interrompe a coleta da fonte anterior, descarta seus dados exibidos e limpa seu cache. Apenas a fonte selecionada pode publicar nova leitura. Respostas tardias da fonte, conta ou quadro anteriores não podem repovoar o Painel nem o cache; voltar à origem requer uma leitura válida do contexto preservado.

Se a limpeza em disco falhar, a troca não é confirmada e o destino não começa a coletar. A origem fica pausada e seus dados ocultos, com mensagem e ação para tentar novamente. Enquanto essa limpeza estiver pendente, não restaurar seu cache nem retomar coleta automaticamente, inclusive após reiniciar; a implementação deve preservar essa pendência como metadado, sem conteúdo privado.

Desconectar remove a credencial local, o contexto selecionado e leitura/cache da integração correspondente. Só confirmar a desconexão quando a remoção exigida tiver sucesso; distinguir erro do cofre e erro de limpeza, sem sucesso aparente. Excluir credencial local não é revogar o token no Jira. Não alternar automaticamente para a outra fonte diante de desconexão, credencial expirada ou falha de rede.

### Offline e privacidade

A leitura de itens Jira existe somente em memória. Durante perda de conectividade na mesma execução, pode aparecer como última leitura, identificada por idade/indisponibilidade e sem permitir escrita até atualização válida. Reiniciar offline mostra ausência de leitura, sem restaurar itens do disco. Conexão e quadro escolhido continuam no cofre.

Não persistir itens Jira, inclusive chave/título, responsável, prioridade ou prazo, em cache ou preferências. Logs e notificações continuam limitados a fonte, evento, status e duração; não incluem corpos de erro, credenciais ou conteúdo de itens. O cache Trello existente conserva seu contrato de projeção permitido enquanto ativo, mas a limpeza ao desativar precisa ser verificável. Essas regras se aplicam também a respostas tardias e operações malsucedidas.

### Critérios de aceite para implementação

1. Uma instalação existente mantém Trello e seu quadro, migra a Âncora para Tarefas e preserva as demais preferências; a interface sempre identifica a fonte ativa.
2. Com ambas as conexões configuradas, somente a fonte ativa faz consultas. Trocar e voltar preserva a seleção própria, sem exibir leitura ou operações da outra fonte, conta ou quadro.
3. Conexão Jira valida a própria credencial no contexto informado; cofre indisponível recusa salvar, sem escrita alternativa ou vazamento de dados em erro.
4. O quadro Jira é lido por completo conforme seu contexto e paginação, com os campos acordados quando presentes. Quadro sem itens, acesso recusado e erro de leitura têm estados distinguíveis.
5. Transições consultadas por item respeitam a resposta do serviço. Destinos ambíguos exigem escolha; campos obrigatórios não atendidos levam ao navegador; recusa/conflito não aparece como sucesso.
6. Escrita pendente impede troca de contexto até resposta/timeout. Resultado incerto não é reenviado; a origem precisa ser relida antes de nova escrita, mesmo após sair e voltar.
7. Trocar pausa a fonte anterior e limpa leitura/cache. Falha de limpeza deixa a troca pendente, com origem oculta/pausada, destino inativo e opção de nova tentativa; reiniciar não restaura o cache pendente.
8. Desconexão não é confirmada se cofre ou limpeza falharem; não reativa a outra integração nem declara revogação remota do token.
9. Offline conserva somente a leitura Jira já em memória e sua idade, sem escrita. Reiniciar offline não restaura itens; conta e quadro continuam configurados no cofre.
10. Testes nas portas de E/S e snapshots devem cobrir migração, isolamento de contexto, respostas atrasadas, timeout/resultado incerto, limpeza falha e privacidade com sentinelas sintéticas. Capturas futuras seguem sanitização e distinguem origem real de variante sintética.

### Contexto para retomada e validação desta decisão

- [GLOSSARY.md](../../../GLOSSARY.md): termos Tarefas e Provedor de tarefas aprovados.
- [Tarefas com um Provedor de tarefas ativo](../../../docs/adr/0004-tarefas-com-provedor-alternavel.md): fronteira de domínio e motivo da identidade estável.
- [Jira Cloud: contratos para o Módulo local](../research/jira-cloud.md): fatos técnicos que embasam autenticação, leitura e transições.
- [Quais fronteiras e evidências tornam a implementação pronta para começar?](08-architecture-validation.md): define formatos, portas, fronteiras de plataforma e estratégia de validação, respeitando este contrato.

Revisão desta etapa: duas rodadas de respostas, confronto com o código versionado, revisão de texto e vínculos. Apenas documentação foi alterada; nenhuma credencial, configuração do host ou item privado entrou nos arquivos. Os critérios acima são requisitos futuros, não testes executados nem afirmação de integração já funcional.
