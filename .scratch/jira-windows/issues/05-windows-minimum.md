# Qual é o mínimo obrigatório do produto no Windows?

Type: grilling
Labels: wayfinder:grilling
Status: resolved
Assignee: Codex / windows_minimum
Parent: ../map.md
Blocked by: 02, 03, 09

Prerequisites: [Quais capacidades nativas do Windows substituem os adaptadores Linux?](02-windows-platform.md); [Como acompanhar IA e Sessões de agente no Windows?](03-windows-agents.md); [Quando a assinatura do app Windows é necessária?](09-windows-signing.md)

## Question

Quais capacidades precisam funcionar na primeira versão Windows para ela ser útil, e quais podem aparecer como indisponíveis com explicação explícita?

Decidir versão/arquitetura do Windows, Indicador e abertura do Painel, IA/Agentes nativos ou WSL, GitHub, Trello/Jira, Rede/Sistema e suas métricas, cofre, notificações, autostart e suspensão. Definir equivalência esperada no Ubuntu e casos de aceite por plataforma. Cofre nativo e recusa sem cofre já foram aceitos; escolher backend real e persistência local, evitando mock e roaming involuntário, com a evidência técnica disponível. Distinguir estado durável de agente background e processo vivo antes de mudar os Estados da sessão atuais.

## Comments

### Primeira rodada confirmada — 04/11/2026

Ticket reivindicado por Codex. O usuário aceitou as cinco recomendações:

- Primeiro suporte oficial: Windows 11 x64, mantendo Ubuntu como alvo existente. Windows 10, ARM64 e x86 ficam para outra etapa de validação.
- Agentes executados nativamente no Windows primeiro; integração com agentes no WSL fica para etapa posterior.
- Mínimo funcional: Limites de IA, Sessões de agente, GitHub e Tarefas; Rede com conexão, taxas totais, latência e teste de velocidade; Sistema com CPU, RAM, armazenamento e bateria quando houver. Temperatura, tráfego por processo e detalhes avançados sem equivalente validado podem aparecer como indisponíveis com motivo explícito.
- Clique esquerdo do Indicador abre/ativa o Painel; direito abre menu. Inicialização automática é opcional e silenciosa, sem abrir o Painel no login. Respeitar suspensão/retomada e instância única.
- Priorizar instalador EXE por usuário, sem exigir administrador no fluxo normal, verificando WebView2. Políticas/permissões que impeçam instalar devem gerar explicação clara.

A definição de Indicador foi atualizada no [GLOSSARY.md](../../../GLOSSARY.md). A segunda rodada esclarecerá modalidades de cliente/sessões de agentes, hooks/notificações e o OAuth Claude opcional. Essas escolhas ainda não estão resolvidas; nenhuma integração nativa foi implementada ou testada nesta etapa.

### Segunda rodada confirmada e pesquisa adicional — 04/11/2026

O usuário confirmou:

- Sessões dos CLIs Codex e Claude Code nativos, com execução local viva, primeiro. Sessões exclusivas de apps/IDEs e tarefas persistidas sem processo ficam para ampliação posterior; os Estados da sessão atuais são preservados.
- Detecção instantânea por hooks após conexão explícita, com consulta periódica como alternativa; notificações nativas configuráveis com clique abrindo Agentes sem mudar Âncora. Focar qualquer terminal externo não é exigência desse mínimo. Hooks são aditivos e reversíveis.
- Status line Claude obrigatória no Windows; opt-in OAuth pode aparecer indisponível até adaptação/validação próprias. O recurso opcional Ubuntu mantém seu contrato e o token nunca é renovado pelo monitor.

Antes de concluir o ticket, o usuário solicitou verificar a necessidade de assinatura do app Windows. A pesquisa [Quando a assinatura do app Windows é necessária?](09-windows-signing.md) é novo pré-requisito; não se presume que custo zero garanta instalação irrestrita nem que assinatura comercial seja obrigatória em toda distribuição.

Em seguida, o usuário direcionou a pesquisa para encontrar um meio de dispensar a assinatura. O contrato de distribuição deve refletir esse objetivo e os limites técnicos/políticos encontrados, sem presumir compra de certificado ou inscrição em serviço.

## Answer

Contrato confirmado nas duas rodadas de 04/11/2026, incorporando a pesquisa de assinatura solicitada e o direcionamento para dispensá-la. A resolução define requisitos para implementação futura; não comprova que o app já compila ou funciona no Windows.

### Plataformas e cobertura inicial

- Acrescentar Windows 11 x64 e preservar Ubuntu/GNOME/Wayland. Não declarar Windows 10, ARM64 ou x86 como suportados sem esforço próprio de validação.
- Acompanhar inicialmente os CLIs Codex e Claude Code executados nativamente em terminais Windows, com execução local viva. WSL, sessões exclusivas de app/IDE e tarefas persistidas sem processo ficam para ampliação posterior.
- Preservar o domínio atual de Sessão de agente e seus Estados. Histórico, SQLite ou registro durável sem evidência de execução não tornam uma sessão ativa. Falha de observação não comprova encerramento; apresentar indisponibilidade/idade da leitura em vez de inventar estado.

### Capacidades obrigatórias

| Área | Mínimo a validar no Windows |
| --- | --- |
| Limites de uso | Codex nativo pela fonte de limites suportada e Claude pela status line conectada explicitamente. Mostrar ausência, erro e idade sem confundir falta de leitura com consumo zero. |
| Sessões de agente | Descoberta das execuções CLI nativas cobertas, Estados da sessão e atualização correta. Hooks opcionais após clique explícito; consulta periódica como alternativa. |
| GitHub | Integração funcional com as fontes/configurações disponíveis, preservando autenticação segura e projeção de privacidade existentes. |
| Tarefas | Trello e Jira selecionáveis, somente um ativo, cumprindo [Como escolher Trello ou Jira preservando estado e privacidade?](04-provider-contract.md). |
| Rede | Conexão atual, taxas totais de tráfego, latência e teste de velocidade. Indisponibilidade e falta de permissão são distinguíveis de valor zero. |
| Sistema | CPU, RAM, armazenamento e bateria quando houver hardware. Ausência de bateria é estado normal, sem dado fictício. |
| Desktop | Indicador, Painel, Configurações, tema, instância única, autostart opcional, suspensão/retomada e abertura de links por meios nativos. |
| Notificações | Notificações nativas configuráveis; clique abre Agentes sem alterar a Âncora. Foco de qualquer terminal externo não é requisito. |
| Credenciais | Windows Credential Manager real, restrito ao usuário e à máquina, sem roaming nem fallback em arquivo/backend simulado. |

Temperatura, tráfego por processo e detalhes avançados sem equivalente validado podem ficar indisponíveis com motivo explícito. RAM/commit/paginação não recebem automaticamente o rótulo Linux “Swap”. Segurança da rede parcial não vira avaliação “Sem alertas” sem as evidências exigidas. Preservar o contrato sem componente privilegiado; não exigir elevação para completar métricas opcionais.

### IA, hooks e notificações

A status line Claude é requisito. O opt-in OAuth pode aparecer como indisponível no Windows até adaptação e validação próprias; no Ubuntu, mantém o contrato do ADR 0002. Token Claude é apenas lido em memória, nunca renovado. Nenhuma configuração de hooks/status line muda antes do clique explícito; instalação/desinstalação preservam alterações de terceiros e são aditivas/reversíveis.

Limites do Codex e descoberta de Sessões de agente são capacidades distintas. Um app-server iniciado pelo monitor não comprova observar todos os CLIs independentes. A cobertura nativa precisa ser medida; somente listar histórico não satisfaz esse mínimo.

Notificações só carregam metadados permitidos. Validar o clique tanto no banner quanto na Central de Notificações, com abertura do Painel/Agentes e preservação da Âncora. Há caminho oficial unpackaged para ativação; verificar runtime, registro por usuário, instância única e limpeza na desinstalação. Não presumir que um plugin oferece a ativação completa nem que assinar o app a implementa.

### Indicador, início e retomada

No Windows, clique esquerdo abre/ativa o Painel e clique direito abre o menu. Início automático é escolha do usuário e inicia silenciosamente, sem apresentar o Painel no login; desativar não é revertido pelo app. Instância única evita processos duplicados ao abrir por atalho, Indicador ou notificação.

Suspender pausa as coletas aplicáveis. Retomar/desbloquear refaz leituras e baselines sem duplicar loops, calcular taxas sobre toda a suspensão ou disparar rajada de requisições. Respeitar offline e backoff. O contrato Ubuntu do Indicador permanece no ADR 0001. Composição reduzida/manter no topo e semântica do relógio serão decididas nos tickets próprios, sem presumir paridade Wayland.

### Cofre por sistema

Usar Windows Credential Manager para credenciais genéricas do usuário, com persistência local à máquina (`CRED_PERSIST_LOCAL_MACHINE`) ou mecanismo equivalente cuja restrição seja comprovada. Ubuntu mantém GNOME Keyring. Sem cofre real ou diante de erro de acesso, recusar salvar e explicar a falha, sem sucesso aparente por mock ou arquivo alternativo.

A configuração atual do `keyring` não basta para afirmar suporte Windows: a pesquisa encontrou fallback para mock sem backend aplicável e persistência potencialmente itinerante no backend v3. A implementação deverá provar seleção do cofre real e persistência local. A escolha da biblioteca/adaptador e da composição por alvo pertence à decisão de arquitetura, sem mudar este contrato.

### Distribuição sem assinatura própria

Priorizar **EXE/NSIS por usuário, inicialmente sem assinatura Authenticode**, seguindo o pedido explícito de dispensar assinatura e manter custo zero. O fluxo normal não exige administrador; verifica WebView2 e informa instalação/ausência/impedimento conforme o ambiente. Não pressupor que ter Edge garante o runtime necessário.

Esse caminho é permitido para distribuição direta, mas não garante instalação/execução em toda política do Windows 11: SmartScreen pode advertir e Smart App Control ou políticas gerenciadas podem bloquear arquivo desconhecido. A página de download deve informar o limite, pois um instalador bloqueado pode nem iniciar para mostrar diagnóstico. A aplicação não exige desativar proteções, manipular marca de origem ou importar certificado como parte normal de instalar.

Checksums e atestação de proveniência GitHub permanecem evidências de integridade/origem; não são Authenticode nem dispensam política de execução. Assinatura gratuita OSS e Store/MSIX ficam como alternativas futuras condicionadas a requisitos/aprovação, sem inscrição ou compra implícitas. MSIX via Store é assinado pela Microsoft e portanto dispensa certificado próprio, mas não é distribuição realmente sem assinatura.

### Critérios de aceite para implementação

1. Compilar e testar Windows 11 x64 com dependências/adaptadores nativos, preservando verificações e pacotes Ubuntu. Não declarar suporte apenas por compilar a UI ou abrir fixtures.
2. Instalar/desinstalar EXE por usuário em ambiente limpo, com WebView2 presente e ausente, sem elevação no fluxo normal. Registrar resultado sob SmartScreen/SAC/políticas sem alterá-las para obter sucesso.
3. Verificar Indicador, abertura/menu/Configurações, instância única, tema, escala e múltiplos monitores. Início automático respeita opt-in, inicia oculto e deixa de iniciar após opt-out.
4. Provar salvar/ler/remover credenciais no cofre nativo e sua persistência local ao usuário/máquina. Falhas, cofre indisponível e backend não nativo recusam salvar; testes de privacidade comprovam ausência de segredos fora do cofre.
5. Exercitar os dois CLIs nativos cobertos: execução viva, espera, turno concluído, falha e encerramento. Histórico sem execução, observação negada e fonte ausente não geram sessões ativas fictícias ou falsa confirmação de encerramento.
6. Conectar/desconectar hooks e status line somente após clique; preservar comando/configuração anterior e terceiros, incluindo caminhos com espaços/Unicode e shells nativos. Sem hooks, a alternativa periódica funciona.
7. Comprovar limites Codex e status line Claude a partir das fontes reais sanitizadas. OAuth Windows indisponível é explícito; zero leitura/renovação de token quando desligado e nenhuma renovação em qualquer modo.
8. Validar notificações no app instalado, banner e Central, abertura de Agentes sem mudar Âncora e conciliação com instância única. Notificação desativada/recusada não interrompe o restante do app.
9. Validar GitHub e Tarefas com fontes configuradas, erros de credencial/rede e os critérios da decisão Tarefas. Nenhum caminho depende de `xdg-open`, D-Bus ou outra integração Linux no Windows.
10. Exercitar Rede/Sistema como usuário padrão: interface/troca de rota, offline/retomada, métricas básicas, desktop sem bateria e dado opcional ausente/negado. Sem zeros fictícios, soma duplicada de interfaces ou rótulos Linux semanticamente incorretos.
11. Confirmar pausa/retomada sem duplicação ou rajadas e com taxas/baselines renovados. Ausência de privilégio não bloqueia o mínimo nem provoca instalação de componente privilegiado.
12. Manter fixtures reais projetadas e sanitizadas nas portas e snapshots UI correspondentes; variantes sintéticas identificadas. Relatórios públicos não carregam caminhos pessoais, contas, rede, sessões ou conteúdo real.

### Contexto e validação desta etapa

- [Windows nativo com escopo inicial explícito](../../../docs/adr/0005-windows-nativo-com-escopo-inicial.md) registra a decisão de domínio.
- [AGENTS.md](../../../AGENTS.md) registra cofre nativo por sistema e a leitura obrigatória do mapa ao implementar essa evolução.
- [Plataforma Windows nativa: capacidades, limites e verificações](../research/windows-platform.md), [IA e Sessões de agente no Windows](../research/windows-agents.md) e [Assinatura Windows: necessidade e caminho sem certificado próprio](../research/windows-signing.md) guardam evidências e limites.
- [Quais fronteiras e evidências tornam a implementação pronta para começar?](08-architecture-validation.md) decidirá portas, dependências por alvo, escolhas de bibliotecas, matriz de execução e pipeline concretos a partir desses critérios.

Validação: duas rodadas de respostas, direcionamento explícito sobre assinatura, revisão documental de fontes primárias e do código versionado, vínculos e dependências locais. Apenas documentação foi alterada; builds, instalações, testes nativos e certificados não foram executados/configurados. Os critérios acima são verificações futuras, não resultados já aprovados.
