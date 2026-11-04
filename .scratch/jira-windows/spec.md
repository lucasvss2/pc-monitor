# Tarefas, Windows e apresentação do Painel

Especificação aprovada em 04/11/2026. Origem: [mapa de decisões](map.md). Este documento organiza a implementação futura; não declara suporte Windows ou integração Jira já implementados. O código atual e os guias continuam sendo a evidência do comportamento disponível.

## Contratos de produto

As respostas e os critérios de aceite de cada área permanecem nos respectivos tickets. Esta especificação acrescenta os contratos técnicos da [decisão de arquitetura](issues/08-architecture-validation.md), sem substituir aquelas respostas.

| Área | Contrato aprovado e fonte normativa |
| --- | --- |
| Tarefas | Um provedor ativo, Trello ou Jira Cloud; quadro selecionado inteiro, campos acordados, transições permitidas, isolamento, limpeza e leitura Jira só em memória. [Contrato de provedor](issues/04-provider-contract.md). |
| Windows | Windows 11 x64, agentes CLI nativos com execução viva, mínimo funcional obrigatório, cofre local, hooks opcionais, notificações e EXE por usuário. Ubuntu preservado. [Mínimo Windows](issues/05-windows-minimum.md). |
| Distribuição | EXE direto inicialmente sem assinatura Authenticode, custo zero; avisos e bloqueios de políticas são limites explícitos. [Assinatura e alternativas](issues/09-windows-signing.md). |
| Painel | Reduzido Núcleo de IA; Modo e Fixação independentes, comportamento de foco e geometrias lembradas. [Composição e comportamento](issues/06-compact-panel.md). |
| Relógio | Hora/fuso do sistema nos dois sistemas, `HH:mm:ss · dd/MM/yyyy`, visível nos dois modos. [Contrato do relógio](issues/07-system-clock.md). |

Configurações, detalhes de Módulo e notificações podem abrir o completo temporariamente, com retorno ao reduzido e sem substituir o Modo preferido salvo. Notificações preservam a Âncora. A ordem aprovada começa pela base Windows e pelos riscos nativos de cofre, hooks e notificações, antes de Tarefas/Jira, restante do mínimo Windows e Painel reduzido/relógio.

O recorte completo continua local, sem telemetria, servidor próprio, helper privilegiado ou custo de serviço exigido pelo monitor. Fora do escopo inicial: Jira Data Center, formulários de edição, WSL, agentes exclusivos de IDE/app, outras arquiteturas Windows, Store/MSIX e garantia de execução sob qualquer política de segurança. O mapa detalha essas exclusões.

## Núcleo e adaptadores

Manter uma aplicação: um núcleo Rust, uma UI Svelte e os contratos de domínio compartilhados. Isolar implementações Linux/Windows e Trello/Jira nas fronteiras em que os fatos realmente variam. A decisão durável está no [ADR de núcleo compartilhado](../../docs/adr/0007-nucleo-compartilhado-e-adaptadores-nativos.md).

| Fronteira | Responsabilidade |
| --- | --- |
| Domínio compartilhado | Estado dos Módulos, alertas, operações, contexto de Tarefas, políticas de privacidade, projeções de persistência e regras de apresentação. |
| Portas de E/S | Contratos pequenos para HTTP, arquivos, processos, cofre, relógios, notificação, sessão e janela; manter as portas existentes quando suficientes. |
| Adaptadores nativos | Observações e ações reais de cada SO, convertidas aos tipos compartilhados; código específico sem escapar das portas. |
| Adaptadores de provedor | Autenticação e semântica de Trello/Jira, paginação, leitura e ações; usam as portas HTTP/cofre existentes. |
| Composição | Seleciona adaptadores por alvo e injeta dependências; não decide regras do domínio dentro de callbacks de plataforma. |
| UI | Renderiza snapshots e capacidades, solicita ações tipadas, conserva foco de controles e estado efêmero de formulário; não coleta dados do host ou dos serviços. |

Uma organização possível é `src-tauri/src/platform/{linux,windows}` para concretos nativos e uma área `tasks` para coordenação e os dois provedores. A responsabilidade é obrigatória; o nome exato dos arquivos pode acompanhar os módulos existentes. Não criar um sistema genérico de plugins nem duplicar toda a aplicação por SO.

Separar dependências no `Cargo.toml`, módulos, factories, binários auxiliares e testes por alvo. D-Bus, GTK, Secret Service, sockets Unix e parsers `/proc`/`/sys` pertencem ao caminho Linux. Windows não emula `/proc`: fornece observações equivalentes do domínio quando disponíveis. O SNI do GNOME e sua decisão de posicionamento continuam próprios do Ubuntu; o Indicador Windows tem adaptador nativo.

O seam uniforme continua sendo o conjunto de portas injetáveis, não um trait gigante nem um segundo caminho especial para testes. Parsers Linux conservam fixtures de saídas Linux abaixo desse adaptador. Windows ganha capturas sanitizadas próprias; regras comuns recebem observações normalizadas, com procedência registrada.

## Publicação de estado e operações

O protocolo núcleo–UI deve separar quatro identidades:

- `runtimeId`: identifica somente a execução atual do núcleo; obtido no bootstrap da UI, sem persistência ou identidade de conta.
- `revision`: contador crescente por Módulo nessa execução, incrementado em toda publicação, inclusive limpeza, erro e ausência de leitura.
- `contextGeneration`: geração do contexto de dados; em Tarefas muda ao invalidar provedor, conta/site ou quadro. Não reutilizar a geração de conectividade/suspensão para essa finalidade.
- `updatedAt`: instante civil da última leitura válida, possivelmente nulo; serve para apresentação/idade, nunca para ordenar publicações.

O núcleo rejeita respostas de geração antiga antes de atualizar estado, cache ou operação. A UI aceita a publicação mais recente da execução corrente e ignora revisões antigas, inclusive na corrida entre snapshot inicial e eventos. Uma limpeza com `updatedAt: null` deve aparecer imediatamente. Retroceder a hora do SO não pode congelar snapshots.

Comandos de ação levam a geração esperada e são revalidados no núcleo. Uma operação é identificada por `(module, contextGeneration, id)` na execução corrente. Preservar as origens `user`, `automatic`, `resume` e os estados úteis já existentes; acrescentar resultado tipado `unconfirmed` para escrita remota sem resposta conclusiva. Cada estágio conserva a identidade da operação até resultado terminal. Cancelar espera local não comprova cancelamento no serviço.

Snapshots usam tipos explícitos por Módulo e um envelope comum. Reduzido e completo recebem o mesmo estado, sem um novo polling de rede ou cópia de credenciais. Capacidades informam disponibilidade e motivo seguro; distinguir recurso não suportado, hardware ausente, erro de leitura e valor válido. Não converter indisponibilidade em zero, quadro vazio ou “Sem alertas”. Campos obrigatórios do mínimo Windows não podem ser reclassificados silenciosamente como opcionais.

## Coordenação de Tarefas

O coordenador mantém um provedor ativo e uma geração de contexto. Cada adaptador oferece seleção de quadros, leitura paginada do quadro, ações disponíveis por item e execução de uma ação identificada. O contrato comum preserva campos próprios necessários: mover para lista Trello não é executar transição Jira; coluna, estado e ação não são intercambiáveis.

No Jira, respeitar o contexto real do quadro e a paginação completa. Não inferir modalidade Scrum/Kanban pelo tipo `simple` nem substituir o quadro por consulta de projeto. Consultar transições para o item; quando houver exigência de campo não atendido, apresentar a limitação e abrir o Jira. Autorização, validação e confirmação final continuam sendo do serviço. Os detalhes das APIs, autenticação e limitações estão na [pesquisa Jira](research/jira-cloud.md); confirmar os contratos vigentes ao implementar.

Antes de trocar contexto ou desconectar, aguardar escrita pendente até resposta ou timeout limitado. Em resultado não confirmado, não reenviar; a origem exige releitura válida antes de outra escrita. Reiniciar também exige primeira leitura válida antes de habilitar ações, cobrindo a perda do estado volátil de resultado incerto. Ausência de conectividade mantém ações indisponíveis.

### Troca e desconexão recuperáveis

A limpeza é uma operação com resultado verificável. A porta de persistência deve devolver erro estruturado de remoção, em vez de descartá-lo. Usar este protocolo:

1. Registrar duravelmente uma intenção mínima de limpeza antes de concluir qualquer troca. A intenção contém apenas tipo de ação, provedor(es) e etapa; não inclui credencial, site, conta, quadro ou conteúdo. Se essa gravação falhar, não iniciar nem declarar concluída a troca.
2. Invalidar a geração da origem, pausar sua coleta e ocultar/descartar suas leituras. Limpar memória e o cache permitido em disco. Respostas antigas não podem repovoá-los.
3. Se a limpeza falhar, manter a intenção, origem oculta/pausada e destino inativo. Exibir falha segura e permitir nova tentativa. No bootstrap, processar a intenção antes de restaurar cache ou iniciar coletas.
4. Após a limpeza, gravar atomicamente a seleção final e remover a intenção; só então iniciar o destino. Falha nessa gravação conserva a pendência. Repetir a limpeza deve ser seguro.

Desconexão segue recuperação equivalente, incluindo remoção da entrada do cofre e do contexto. Cofre e arquivo não formam uma transação atômica: registrar a etapa pendente e tratar exclusões repetidas como idempotentes. Só confirmar quando as remoções exigidas tiverem sucesso; não reinserir credencial automaticamente nem afirmar revogação remota.

Trocar preserva credenciais e quadro de cada provedor no cofre; desconectar os apaga. Jira nunca grava leitura de itens em disco. O cache Trello conserva sua projeção permitida enquanto ativo, com limpeza obrigatória ao desativar. Preferências e intenção de limpeza não podem virar um cache indireto de conteúdo ou identidade de conta.

## Cofre, hooks e notificações

Preservar o adaptador GNOME Keyring. No Windows, implementar a porta usando Windows Credential Manager, restrito ao usuário e à máquina, sem roaming. A rota inicial é um adaptador WinCred com persistência `CRED_PERSIST_LOCAL_MACHINE`; outra implementação só satisfaz o contrato mediante prova equivalente. Não aceitar backend mock, arquivo ou persistência com roaming por ser o default de uma biblioteca. Falha ou ausência de cofre recusa salvar e não cria fallback.

Capturas e diagnóstico do cofre registram somente categoria/resultado; nunca imprimem valores gravados ou lidos. O teste nativo usa uma credencial sintética isolada, confere atributos/escopo e a remove ao terminar. Um teste com fake comprova regras de recusa, mas não o cofre real.

Hooks e status line são instalados apenas após ação explícita do usuário, de forma aditiva e reversível. Implementar caminhos, quoting, transporte local e execução nativos, preservando configurações de terceiros e verificando remoção somente do trecho instalado pelo monitor. Falha no hook deixa a consulta periódica disponível. Histórico salvo sem processo vivo não é sessão ativa. Status line Claude integra o mínimo; OAuth pode ficar indisponível no Windows conforme o contrato aprovado, sem renovar token.

Notificações usam metadados e preferências do usuário. O adaptador Windows precisa validar entrega e ativação tanto pelo banner quanto pela Central de Notificações, inclusive com Painel fechado. O clique abre Agentes como Apresentação contextual sem regravar Âncora. A rota unpackaged descrita na [pesquisa de assinatura](research/windows-signing.md) é candidata; o nome de um plugin Tauri, um mock ou a existência de um toast não comprovam ativação funcional. Se a rota falhar, investigar outra implementação de custo zero antes de alterar o requisito.

## Estado da janela e relógios

O controlador do Painel vive fora da webview, pois fechar a janela não encerra o app e sua recriação não pode perder preferências ou operações. Separar:

| Persistido, apenas preferências | Somente na execução |
| --- | --- |
| `preferredMode`, preferência de fixação, tamanho de cada modo e posição Windows | Modo efetivamente apresentado, rota contextual e retorno, foco, diálogo, resultado de aplicar fixação e geometria atual validada |

O botão explícito de alternar tamanho muda `preferredMode`, encerra a apresentação temporária e preserva Âncora/dados/operações. Entrar em Configurações, detalhes ou notificação não o modifica: abre o completo com retorno à apresentação anterior. Navegação entre rotas contextuais não cria uma pilha ilimitada de retornos. Resolver diálogos conforme o fluxo existente antes de trocar apresentação; não persistir rascunhos com credenciais. Fechar encerra a rota contextual; nova abertura restaura o modo preferido.

O Painel permanece aberto ao perder foco em qualquer tela ou modo, conforme a [revisão explícita aprovada durante o aceite de Tarefas](issues/10-panel-explicit-close.md) e a matriz atualizada do [contrato do Painel](issues/06-compact-panel.md). X e Escape fecham o Painel; Escape resolve primeiro um diálogo interno. A fixação efetivamente aplicada continua controlando a ordem de janelas, sem determinar fechamento. Falha de topmost não aparece como sucesso; Wayland declara indisponibilidade. Notificação pode solicitar abertura/foco uma vez, sem retomada periódica de foco.

Restaurar e validar o tamanho próprio de cada modo em unidades lógicas. No Windows, ajustar posição/tamanho à área útil ao mudar monitor, resolução ou DPI; no Ubuntu, respeitar posicionamento do compositor. Medir o mínimo nativo com fontes e escalas suportadas: cabeçalho, retorno/fechar e relógio acessíveis, corpo rolável verticalmente se necessário, sem corte de ações ou rolagem horizontal dos resumos. A referência de aproximadamente 380 × 620 px não é limite rígido comprovado.

Separar na porta de relógio o tempo civil/local e a medição monotônica de duração. Agendamentos, timeout e backoff não dependem de saltos da hora civil. Na retomada, reavaliar prazos, conectividade e bases de contadores; não presumir que todos os SOs incluem suspensão da mesma forma em seu relógio monotônico.

O Relógio do Painel lê hora e fuso atuais ao abrir/retomar/voltar a ficar visível e nominalmente a cada segundo enquanto visível. Não soma ticks nem conserva formatador com fuso obsoleto. Não faz rede, NTP ou alteração da hora do SO; não publica segundos como histórico de Módulo. Compartilhar esse comportamento entre os dois modos, sem anúncio acessível a cada segundo.

## Migração e preservação de dados

Versionar o schema de preferências e migrar a identidade de Módulo `trello` para `tasks`, incluindo Âncora e referências de cache/eventos. Instalação existente mantém Trello selecionado, quadro e credenciais no cofre, sem reconexão desnecessária. Instalação nova conserva o fluxo de conexão explícita.

Aplicar defaults localizados para campos ausentes/inválidos; não apagar todas as preferências porque um novo campo é inválido. Preservar tema, movimento reduzido, visibilidade, ordem e demais escolhas válidas. Schema futuro desconhecido não deve ser sobrescrito por defaults; informar incompatibilidade e preservar o arquivo. Gravação por arquivo temporário e substituição atômica, com falhas observáveis.

Manter a entrada Trello existente ou migrá-la dentro do cofre com confirmação antes de remover a origem. Contexto Jira também permanece no cofre. Não exportar credenciais entre Linux e Windows. Geometria inválida se recupera com valores seguros; pendência de limpeza impede restaurar dados da fonte afetada antes de qualquer leitura/coleta.

## Sequência e evidências exigidas

Cada etapa entrega um incremento verificável e preserva Ubuntu. A ordem abaixo é de implementação; os tickets do mapa são decisões já tomadas, não tickets de execução.

| Etapa | Resultado | Evidência para avançar |
| --- | --- | --- |
| Base e riscos Windows | Composição por alvo, executável nativo, ciclo Indicador/Painel, cofre, hooks e notificação com ativação. | Build nativo nos dois SOs; teste real do cofre local sem roaming; instalação/remoção aditiva dos hooks; evento real das CLIs e clique de notificação funcional. Falha obrigatória bloqueia a etapa. |
| Tarefas/Jira | Identidade comum, migração Trello, conexão Jira, quadro completo, transições e troca recuperável. | Fixtures reais sanitizadas dos serviços; testes de geração/atrasos/timeout/limpeza/reinício; aceite de leitura e ação com conta de teste autorizada nos dois SOs, sem alteração em itens reais sem autorização. |
| Restante do mínimo Windows | Limites de IA, sessões vivas, GitHub, Tarefas, Rede e Sistema com o mínimo aprovado. | Capturas Windows sanitizadas, normalização e testes; execução nativa com suspensão/retomada, offline, concorrência e ausência de hardware. Toda diferença opcional aparece com motivo. |
| Painel e relógio | Reduzido aprovado, apresentação contextual, fixação, geometrias, hora local e acessibilidade. | Snapshots/UI, relógios injetados e provas nativas de foco/topmost/DPI/área útil, retorno e persistência; Ubuntu/Wayland preservado. |
| Distribuição | EXE por usuário, WebView2, autostart opcional silencioso, instalação/atualização/remoção. | Instalação real de release sem feature de fixtures e sem administrador no fluxo normal; execução única, ativação de notificação instalada e verificação Ubuntu deb/AppImage. |

A publicação só ocorre quando o mínimo completo e os critérios dos contratos de produto passarem. Não publicar uma prova parcial como suporte oficial Windows. Se uma limitação exigir reduzir o mínimo ou mudar um comportamento aprovado, abrir uma nova decisão no mapa e obter resposta do usuário; medidas de tamanho, nomes de bindings e organização de arquivos que respeitem o contrato são decisões normais de implementação.

## Validação para implementação

Automação comum: portas com fixtures reais sanitizadas e variantes sintéticas identificadas, snapshots de UI e testes das regras. Provas nativas: Windows 11 x64 e Ubuntu/GNOME/Wayland em ambiente real ou VM apropriada, com sessão gráfica/cofre necessários. Cross-compilation, jsdom e fakes não substituem aceitação nativa. Ausência desses recursos em CI deve resultar em etapa nativa separada, rastreável e exigida antes de publicar, nunca em sucesso simulado.

Estender a CI Ubuntu atual para Windows nativo e dependências condicionais. Conservar as verificações aplicáveis existentes: `npm run check`, `npm test`, `npm run build`, `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` e build Tauri. Exercitar fixtures de desenvolvimento onde aplicáveis e manter sua exclusão da release; captura GTK e testes exclusivos de SO devem ser condicionais, não simplesmente removidos para compilar Windows.

| Cobertura | Casos mínimos |
| --- | --- |
| Privacidade | Cada guardrail de [AGENTS.md](../../AGENTS.md) com teste: sem fallback de credencial, sem conteúdo em logs/cache/notificações, hooks só por clique/aditivos/reversíveis, token Claude em memória e sem renovação. Sentinelas sintéticas em sucesso, erro, timeout e respostas tardias. |
| Estado e contexto | Resposta antiga, bootstrap concorrente com evento, limpeza com data nula, relógio retrocedendo, comando com geração antiga, troca durante escrita e resultado não confirmado sem reenvio. |
| Recuperação | Falha em cada etapa da intenção durável, cache não removível, cofre indisponível, commit de preferência falho, interrupção/reinício, desconexão parcialmente concluída e repetição idempotente. |
| Migração | Preferências legadas, Âncora Trello, conexão/quadro existentes, um campo inválido, schema futuro, arquivo ilegível e geometria fora da área útil; preservar o restante quando possível. |
| Agentes | Processo vivo versus histórico, estados existentes, caminhos com espaços, eventos duplicados/atrasados, opt-in/reversão preservando terceiros, status line, fallback periódico e suspensão. |
| Janela e relógio | Modo explícito versus contextual, retorno, notificação sem alterar Âncora, blur, diálogo/Escape, autostart silencioso, posição/DPI/monitor, teclado/temas; mudança de dia/fuso/horário de verão e ajustes para frente/trás por fonte injetada. |
| Instalador e política | Usuário comum, WebView2 presente/ausente, atualização sem perder preferências/cofre, desinstalação sem remover configurações de terceiros, EXE sem assinatura, avisos/bloqueios e orientação antes de executar o instalador. |

O canal direto não exige comprar certificado ou assinar o EXE inicialmente. Documentar no download que SmartScreen, Smart App Control ou política gerenciada podem advertir/bloquear; não prometer execução universal nem exigir desabilitar proteção, remover marca de origem ou instalar confiança artificial. Checksums/atestação de build ajudam a verificar origem e integridade, mas não substituem Authenticode. Store/MSIX permanece alternativa futura, não requisito oculto.

Capturas futuras seguem [sanitização](../../docs/agents/sanitizacao.md): projeção e substituição antes de gravar/imprimir, incluindo nomes, redes, caminhos e metadados. Registrar procedência, cenário, campos substituídos, variantes sintéticas, resultado e limitações, sem valores privados nem mapa de substituição. Revalidar fontes externas ao implementar; relatórios existentes não são testes de integração.

## Estado desta especificação

Concluídos: decisões do usuário, auditoria de código, pesquisas e protótipo descartável aprovado. A composição visual está preservada na branch local indicada no [ticket do Painel](issues/06-compact-panel.md); não reutilizar seus mecanismos simulados em produção nem publicar branches `research/*` ou `prototype/*`.

Pendentes para a implementação: mudanças no app, builds por alvo, medições nativas, instalação real, testes e aceite dos critérios acima. A spec v1 referenciada em `.scratch/kvojps-monitor/spec.md` está ausente nesta árvore; esta especificação não a reconstrói. Evidências disponíveis: código atual, guias, ADRs e pesquisas vinculadas pelo mapa.
