# Qual horário o relógio do Painel deve apresentar?

Type: grilling
Labels: wayfinder:grilling
Status: resolved
Assignee: Codex / system_clock
Parent: ../map.md
Blocked by: 02

Prerequisites: [Quais capacidades nativas do Windows substituem os adaptadores Linux?](02-windows-platform.md)

## Question

O que significa exibir um relógio sincronizado com o Windows no Painel, e qual é o comportamento esperado no modo completo e reduzido e no Ubuntu?

Validar a proposta de acompanhar hora e fuso locais do sistema, sem alterar o sistema nem ter NTP próprio. Decidir formato 12/24 h, segundos/data, frequência de atualização, mudança manual de hora/fuso, horário de verão, suspensão/retomada, visibilidade e acessibilidade. Definir casos de aceite com relógio injetável e evitar timers acumulativos que atrasem após suspensão.

## Comments

### Rodada confirmada — 04/11/2026

Ticket reivindicado por Codex. O usuário confirmou individualmente as três recomendações:

- Acompanhar automaticamente hora e fuso configurados no Windows, incluindo ajustes manuais e horário de verão; adotar o mesmo princípio no Ubuntu. Sincronização externa permanece a cargo do sistema.
- Exibir hora de 24 horas com segundos e data em pt-BR, como no exemplo sintético `14:32:08 · 04/11/2026`.
- Exibir nos modos completo e reduzido, em área global do Painel, sem criar Módulo nem alterar a Âncora. Posição exata será decidida no protótipo.

## Answer

Contrato confirmado pelo usuário em 04/11/2026.

### Fonte e significado

O **Relógio do Painel** acompanha a hora civil e o fuso locais configurados no sistema operacional, tanto no Windows quanto no Ubuntu. Mudanças manuais, ajuste automático pelo sistema e horário de verão se refletem na exibição. Não há fuso próprio selecionável no app, sincronização NTP própria ou alteração da hora/configuração do sistema pelo monitor.

Isso significa refletir o horário configurado, não verificar sua exatidão contra uma fonte externa. O relógio funciona sem conexão de rede. Seu horário não representa a data da última atualização de um Módulo; leituras antigas continuam com sua indicação própria de idade.

### Formato e presença

Formato inicial fixo em pt-BR: `HH:mm:ss` de 24 horas e data `dd/MM/yyyy`, com zeros à esquerda. Exemplo sintético: `14:32:08 · 04/11/2026`. O separador e a distribuição em uma ou duas linhas podem ser ajustados na composição visual, mantendo hora, segundos e data completos.

O relógio fica visível nos modos completo e reduzido, na área global do Painel. Não é Módulo, não participa da Âncora e não provoca mudança de seleção. A posição, tamanho e hierarquia visual serão aprovados em [Como deve funcionar a versão reduzida e a opção de manter no topo?](06-compact-panel.md). Não há opção de ocultar ou de personalizar formato nesse recorte inicial.

### Atualização e ciclo de vida

Enquanto visível, atualizar em cadência nominal de um segundo, sempre a partir de uma nova leitura do horário real do sistema, sem acumular segundos artificiais a cada callback. A execução de um timer não é uma garantia de agendamento exato; atraso de renderização não pode transformar-se em atraso permanente do horário mostrado.

Ao abrir, voltar a ficar visível ou retomar após suspensão, reler hora e fuso antes de apresentar a exibição atualizada. Refletir mudança de hora/fuso com o Painel aberto, invalidando informações de formatação que possam conservar o fuso anterior. Fechado ou oculto, o Painel não exige renderização por segundo; a reabertura usa uma leitura atual, sem reproduzir os ticks perdidos.

Virada do dia, mês e ano e mudanças de horário de verão devem derivar do mesmo instante e fuso usados para formatar a hora. Nenhum fuso brasileiro ilustrativo ou horário de fixture pode virar constante de produção. A hora civil do relógio não deve ser usada como contagem acumulativa de duração; os efeitos de saltos de hora sobre polling, backoff e taxas entram na escolha de portas/relógios em [Quais fronteiras e evidências tornam a implementação pronta para começar?](08-architecture-validation.md).

### Acessibilidade e privacidade

Hora e data precisam ter texto/nome acessível e contraste/legibilidade verificáveis nos dois modos. A atualização dos segundos não deve anunciar automaticamente a hora a cada segundo em leitor de tela; a informação permanece consultável sob demanda.

Atualizar o relógio não faz HTTP, coleta dados de contas ou dispara atualização de Módulos. Não exige logs, cache de horários exibidos, notificações ou persistência de histórico. Fixtures de teste usam instante/fuso sintéticos declarados, sem capturar configurações pessoais do host.

### Critérios de aceite para implementação

1. Com instante e fuso injetados no seam existente, a UI apresenta `HH:mm:ss` e `dd/MM/yyyy` de forma determinística, em pt-BR e 24 horas, incluindo zeros à esquerda e meia-noite `00:00:00`.
2. O relógio aparece nos modos completo e reduzido; nenhuma atualização altera Âncora, Módulo selecionado ou foco, e posição/hierarquia seguem o protótipo aprovado.
3. Tick atrasado usa uma nova leitura real: avançar o relógio de teste por vários segundos não exige executar cada callback perdido para recuperar o horário correto.
4. Abrir/retomar após longo período oculto ou suspenso mostra hora, data e fuso atuais, sem valor anterior persistido nem rajada de ticks acumulados.
5. Ajustar hora para frente/trás e trocar o fuso com o Painel aberto é refletido na exibição, sem reiniciar o app ou conservar o fuso de um formatador antigo.
6. Casos sintéticos cobrem virada de dia/mês/ano e entrada/saída de horário de verão em fuso que tenha essa regra, inclusive salto ou repetição de hora local. Windows e Ubuntu seguem a configuração de seu próprio sistema.
7. Atualizações em segundo plano não geram consultas externas, logs de horários ou notificações; atualizar o relógio não atualiza dados dos demais Módulos. Sem acesso à rede, o relógio continua funcional.
8. Snapshots e verificação visual/acessível cobrem ambos os modos, leitura completa de hora/data e ausência de anúncio automático por segundo. Testes não dependem da hora ou do fuso reais da máquina executora.

### Contexto e validação desta etapa

- [GLOSSARY.md](../../../GLOSSARY.md): definição do Relógio do Painel.
- [Plataforma Windows nativa: capacidades, limites e verificações](../research/windows-platform.md), seção “Modo reduzido e relógio local”: evidências sobre fontes do sistema e mudanças de hora/fuso/retomada.
- [Como deve funcionar a versão reduzida e a opção de manter no topo?](06-compact-panel.md): próximo trabalho visual, agora desbloqueado.

Revisão desta etapa: respostas explícitas do usuário, pesquisa já disponível e revisão de texto/vínculos. Apenas documentação foi alterada; os critérios são requisitos futuros, sem implementação, coleta do host ou testes de execução nesta decisão. Não foi criado ADR separado: formato e fonte do relógio não acrescentam um trade-off arquitetural durável que justifique duplicar este contrato.
