# Como deve funcionar a versão reduzida e a opção de manter no topo?

Type: prototype
Labels: wayfinder:prototype
Status: resolved
Assignee: Codex / compact_panel
Prototype branch: prototype/jira-windows-reduced-panel (local; sem push)
Prototype commit: 13b65b8
Prototype verdict commit: d9b1d39
Prototype guide: src/lib/compact-prototype/REDUCED-PROTOTYPE.md (na branch do protótipo)
Parent: ../map.md
Blocked by: 02, 05, 07

Prerequisites: [Quais capacidades nativas do Windows substituem os adaptadores Linux?](02-windows-platform.md); [Qual é o mínimo obrigatório do produto no Windows?](05-windows-minimum.md); [Qual horário o relógio do Painel deve apresentar?](07-system-clock.md)

## Question

Qual composição e interação tornam a versão reduzida do Painel útil, com botão para alternar tamanho e opção de manter a janela no topo?

Discutir com o usuário um protótipo descartável alimentado por exemplos sanitizados: Módulos visíveis, tamanho mínimo, informação prioritária, lugar do relógio, acesso a ações/Configurações, teclado/acessibilidade, clique fora, posição e múltiplos monitores/DPI, persistência das opções e retorno ao Painel completo. Decidir se manter no topo é independente do tamanho e quais garantias existem por plataforma; não prometer equivalência no Wayland sem evidência. A aprovação visual precisa vir do usuário.

## Comments

### Protótipo para avaliação — 04/11/2026

Criada a [Comparação das composições do Painel reduzido](http://127.0.0.1:1422/compact-prototype.html?variant=A), na rota de protótipo existente e com comutador inferior e parâmetro `variant=A|B|C`:

- **Núcleo de IA**: coluna estreita com IA, atenção, Tarefas e resumos de GitHub/Rede/Sistema; relógio global no rodapé. Proposta de 380 × 620 px lógicos, com crescimento conforme conteúdo.
- **Faixa de instrumentos**: composição horizontal, proposta de 1040 × 252 px lógicos, com todos os resumos lado a lado e relógio à esquerda.
- **Foco por Módulo**: proposta de 390 × 474 px lógicos, relógio/atenção globais e navegação entre Módulos.

Esses tamanhos são referências de discussão, não mínimos nativos aprovados. A visualização completa usa uma captura sanitizada já versionada como referência estática, com controles/relógio propostos ao redor; não é implementação interativa do Painel futuro. Fixação, clique fora e reabertura são simulações de navegador; não mudam janelas, configurações ou fontes reais.

O código descartável está preservado no commit indicado na branch local, sem push nem merge. O guia nessa branch documenta execução por `npm run prototype:reduced`, origem dos exemplos, controles e limites. O servidor Python padrão expõe apenas os assets públicos permitidos em loopback; não requer instalação npm, não lista diretórios e não expõe `.git`.

### Verificação do protótipo

- Inspeção visual das três estruturas e dos temas escuro/claro, com textos sintéticos e referência visual do app atual. Textos de leitura foram ajustados para legibilidade; medições de exemplo não são captura do host.
- Interações verificadas no navegador: alternância por botões/teclado/URL, navegação da opção Foco por Módulo, Âncora preservada ao alternar tamanho, fixação independente, perda de foco simulada, fechamento/reabertura, cenário sem leitura, offline e controle indisponível no Wayland; avanço do relógio sintético por botão.
- Sintaxe JavaScript/Python e whitespace conferidos. Sem testes automatizados, build Tauri, verificação nativa de topmost ou promessa de posicionamento/DPI já validado.
- Seis arquivos do protótipo revisados antes do commit: HTML de entrada, script de execução no package.json, servidor Python, guia, JS e CSS. Exemplos sintéticos; sem credenciais, dados pessoais, endereços privados, sessões reais ou caminhos do host. Autor/committer noreply verificados; nenhuma imagem nova ou binário foi incluído no commit.

### Rodada de escolha visual e comportamento — pendente

Foram apresentadas quatro escolhas ao usuário, ainda sem resposta:

- Escolher uma composição ou mistura; recomendação inicial é Núcleo de IA para manter os resumos no mesmo olhar.
- Fixação no Windows independente do tamanho; indisponibilidade explícita no Wayland.
- Reduzido permanecer aberto sem foco; completo livre fechar ao clicar fora; fixado permanecer aberto até fechamento explícito.
- Lembrar modo, fixação e tamanho por modo entre execuções; no Windows, também posição com recuperação à área útil. Primeiro uso completo/desfixado e autostart silencioso. No Ubuntu a posição cabe ao compositor.

Esses comportamentos continuam propostas. O ticket permanece reivindicado e sem resolução até a avaliação do usuário; nenhum layout foi promovido para a aplicação.

### Aprovação do usuário — 04/11/2026

O usuário confirmou as quatro recomendações: composição **A — Núcleo de IA**, fixação independente do tamanho, reduzido persistente sem foco/completo livre fechando ao clicar fora e preferências lembradas entre execuções, incluindo posição Windows recuperável à área útil. No Wayland, fixação indisponível e posição determinada pelo compositor. Primeiro uso completo/desfixado e autostart silencioso permanecem no contrato.

Veredito também registrado no guia da branch local, no commit indicado em `Prototype verdict commit`. As variantes avaliadas continuam preservadas no commit original; não houve push, merge ou promoção de código descartável para produção.

## Answer

Resolução confirmada pelo usuário em 04/11/2026, a partir da comparação visual e das quatro escolhas explícitas.

### Composição e acesso

Adotar **Núcleo de IA** como composição do Modo reduzido: coluna estreita com Limites de uso de Codex/Claude, atenção de Agentes, Tarefas e resumos de GitHub, Rede e Sistema. O Relógio do Painel permanece na área global inferior, com segundos e data conforme [Qual horário o relógio do Painel deve apresentar?](07-system-clock.md).

A hierarquia visual é a do protótipo aprovado: IA primeiro; contagens/atenção de Agentes; quadro de Tarefas com fonte identificada; resumos dos outros Módulos; relógio. Valores, contagens, nomes e estados do protótipo são sintéticos, não regras fixas de conteúdo. Resumos de Tarefas usam o quadro/contexto real selecionado, sem hardcodar os nomes de status do exemplo. Falta de leitura aparece como indisponibilidade, nunca zero inventado; ausência de atenção não mantém alerta de risco fictício.

O reduzido é uma apresentação do mesmo Painel e das mesmas leituras, não uma segunda instância dos coletores. Seus resumos dão acesso aos Módulos correspondentes no completo; operações detalhadas de Tarefas continuam nesse contexto completo e seguem o contrato aprovado. O cabeçalho mantém acesso a alternar tamanho, fixação, Configurações e fechar. Alternar tamanho preserva Âncora, fonte de Tarefas e operações; interação explícita com Módulo segue a semântica existente de Âncora.

### Tamanho e legibilidade

Referência visual: largura de aproximadamente **380 px lógicos** e altura de conteúdo em torno de **620 px**, sem transformar o mínimo CSS do protótipo em limite nativo rígido. A medição de navegador chegou a cerca de 626 px de altura com os rótulos ajustados. O completo conserva a composição atual de referência, com relógio e controles integrados na implementação posterior.

O menor tamanho suportado deve manter acessíveis cabeçalho, fechamento, retorno ao completo e relógio, com texto legível e sem rolagem horizontal dos resumos. Se conteúdo/tamanho de fonte ou área útil exigirem, o corpo dos resumos pode rolar verticalmente; relógio e controles globais continuam acessíveis. Não reduzir fonte ou cortar ações para fazer caber. Definir os limites nativos exatos a partir de medição com fontes/DPI dos alvos em [Quais fronteiras e evidências tornam a implementação pronta para começar?](08-architecture-validation.md), respeitando essa condição de aceite.

### Modo, fixação e perda de foco

Modo e Fixação do Painel são preferências independentes. No Windows, fixar funciona tanto no completo quanto no reduzido; alternar tamanho conserva a escolha. Fixação controla ordem acima de janelas comuns, não força foco nem significa travar a janela.

| Modo | Fixação Windows | Ao perder foco/clicar fora |
| --- | --- | --- |
| Reduzido | Desligada | Permanece aberto; pode ficar atrás de outras janelas. |
| Reduzido | Ligada | Permanece aberto e no topo de janelas comuns. |
| Completo | Desligada | Permanece aberto; pode ficar atrás de outras janelas. |
| Completo | Ligada | Permanece aberto e no topo de janelas comuns. |

Fechar explicitamente continua possível com fixação ativa. Escape fecha primeiro um diálogo interno, quando houver, preservando seu tratamento de teclado; sem diálogo, pode fechar o Painel. Reabrir pelo Indicador restaura a apresentação lembrada. Fixação não autoriza o app a retomar foco periodicamente enquanto o usuário trabalha em outro aplicativo. Erro ao aplicar topmost não pode aparecer como sucesso do controle.

No Ubuntu/GNOME/Wayland, “Manter no topo” fica explicitamente indisponível; não requer extensão nova nem afirma ter fixado sem suporte. Ambos os modos permanecem abertos ao perder foco. Posicionamento continua a cargo do compositor, conforme ADR 0001.

Revisão aprovada em 04/11/2026 durante a validação de Tarefas: [manter sempre aberto; fechar pelo X ou Escape](10-panel-explicit-close.md). A matriz acima incorpora essa resposta mais recente; ela substitui o fechamento do completo desfixado registrado nas rodadas originais deste ticket. A prévia aprovada anteriormente permanece como evidência histórica, não como autoridade sobre a regra revisada.

### Preferências, posição e início

Lembrar Modo, Fixação e tamanho de cada modo entre aberturas/reinícios. No Windows, lembrar também a posição e recuperá-la para a área útil quando monitor/escala/topologia mudarem, mantendo a janela e seus controles alcançáveis. Alternar tamanho não substitui a geometria lembrada do outro modo. Arrastar/redimensionar usa os mecanismos normais da janela, sem confundir botões do cabeçalho com região de arraste.

No Ubuntu, lembrar Modo e tamanhos respeitando a área útil, sem prometer coordenadas globais ou restaurar uma posição que o Wayland delega ao compositor. Preferência de movimento reduzido já existente é distinta do Modo reduzido e continua respeitada.

Primeira apresentação, sem preferência prévia, usa completo e desfixado. A inicialização automática permanece silenciosa: lembrar reduzido/fixado não abre a janela no login. Ao abrir explicitamente, restaurar as escolhas válidas. Estado ausente/corrompido ou geometria antiga não pode tornar os controles inacessíveis nem descartar as demais preferências do usuário.

Notificações mantêm o contrato de abrir Agentes sem alterar a Âncora. A arquitetura de apresentação distingue navegação contextual de uma troca explícita de Modo, preservando acesso de volta e sem regravar preferências de janela por acidente; a modelagem concreta está na [especificação consolidada](../spec.md), vinculada à decisão de arquitetura.

### Critérios de aceite para implementação

1. O reduzido corresponde à hierarquia Núcleo de IA aprovada, com IA/atenção/Tarefas e resumos de GitHub/Rede/Sistema, mais relógio global. Estados normal, sem leitura, offline e textos longos não cortam ações nem fabricam valores.
2. Alternar completo/reduzido conserva fonte, Âncora, leituras e operações, sem iniciar outro coletor; cada modo restaura seu próprio tamanho válido.
3. No Windows, aplicar/desaplicar fixação funciona nos dois modos e persiste ao alternar. Falha nativa é explícita; não há captura periódica de foco. No Wayland, o controle informa indisponibilidade real.
4. A matriz de perda de foco é exercitada nas janelas nativas, incluindo fechar/reabrir explicitamente, diálogo aberto e Escape. O Indicador e o restante do app continuam disponíveis após fechar o Painel.
5. Preferências são restauradas entre execuções; primeiro uso é completo/desfixado. Autostart com qualquer preferência lembrada continua sem apresentar o Painel até abertura explícita.
6. Em cenários sintéticos de monitores desconectados, resolução menor, escala/DPI diferentes e coordenadas fora da tela, a janela é recuperada à área útil com controles alcançáveis. Ubuntu continua sob posicionamento do compositor.
7. O menor tamanho adotado mantém cabeçalho/relógio e ações globais alcançáveis, corpo rolável se necessário e nenhum resumo com rolagem horizontal. Verificação visual nativa confirma fontes/escala e temas claro/escuro.
8. Teclado alcança os controles com rótulos e foco visíveis; foco acompanha a transição de tamanho de modo previsível; movimento reduzido e acessibilidade do relógio seguem os contratos existentes. O tamanho reduzido não exige anúncio de segundos nem mudança de Âncora.
9. Visibilidade/coleta refletem os resumos realmente expostos, sem tratar simples perda de foco como desligamento de todos os Módulos nem duplicar loops; suspensão/offline mantêm o contrato mínimo Windows/Ubuntu.
10. Preferências guardam apenas estado permitido da janela; fixtures/relatórios usam geometrias e identificadores sintéticos, sem capturar monitores, caminhos, contas ou conteúdo reais do host. Testes nas portas e snapshots distinguem comportamento do navegador de comprovação nativa.

### Evidência e contexto para retomada

- Fonte primária do desenho: branch local `prototype/jira-windows-reduced-panel`, commit `13b65b8`, rota `compact-prototype.html?variant=A`. Guia em `src/lib/compact-prototype/REDUCED-PROTOTYPE.md` nessa branch; executar `npm run prototype:reduced` sem dependências npm adicionais.
- Veredito preservado no commit local `d9b1d39`, sem push/merge. As variantes perdedoras são referência histórica, não alternativas de produção.
- [Painel reduzido e fixação independentes](../../../docs/adr/0006-painel-reduzido-e-fixacao-independentes.md): decisão durável de comportamento da janela.
- [GLOSSARY.md](../../../GLOSSARY.md): Modo do Painel e Fixação do Painel.

Validação desta etapa: aprovação explícita das quatro escolhas, inspeção visual e interações do protótipo, sintaxe e revisão dos arquivos/metadados publicados na branch local. A implementação nativa, persistência real e testes de DPI/topmost ainda são requisitos futuros; nenhum código descartável foi integrado à aplicação.
