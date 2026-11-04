# Painel Windows não abre pelo Indicador

Status: ready-for-human
Assignee: maintainer
Parent: ../spec.md

## Sintoma e reprodução

Em 04/11/2026, o EXE experimental da CI `0e56529`, versão 0.1.1, foi iniciado em Windows x64 26H2, build 26300.9711. Hash validado antes de executar. O usuário confirmou Indicador visível e clique esquerdo sem abertura do Painel. Segunda execução também deixou o Painel ausente; uma única instância permaneceu ativa, respondendo, com janela oculta de controle de instância.

O loop executado pela automação usa `sky.launch_app` no artefato já validado e depois `sky.list_windows`, exigindo exatamente uma janela com título `kvojps-monitor`. Resultado reproduzido: zero janelas visíveis, `FAIL: monitor panel is absent after activation`, 13,8 segundos. A confirmação humana do clique evita confundir somente uma limitação de inventário com o sintoma do app.

WebView2 instalado. Sem evento de crash nem bloqueio de Code Integrity correspondente nas consultas realizadas. stderr vazio na instância instrumentada apenas por redirecionamento externo. Isso não descarta políticas nem prova funcionamento da criação da webview.

## Hipóteses ordenadas

1. Callback de abertura cria a webview na thread incorreta e bloqueia; agendar na thread principal deve resolver se confirmado.
2. Perda de foco destrói a janela antes de exibir; a sequência de criação/carregamento/fechamento distinguirá o caso.
3. WebView2 falha ao criar a janela ou navegar; erro ou ausência de carregamento após build distinguirá o caso.
4. Trava de estado impede chegar ao builder; marcadores antes/depois do estado local distinguem o ponto de parada.

## Critérios

- Repetir o loop original e clique real com Painel visível; preservar instância única e Âncora.
- Teste de regressão no seam que reproduz a falha, se existir; não substituir por inspeção de strings.
- Remover todos os marcadores `[DEBUG-panel-1104]` após diagnóstico.
- Remover artefatos obsoletos, logs e instaladores temporários; não alterar proteções, hooks ou credenciais pessoais.
- Só então retomar os demais cenários do roteiro gráfico.

## Comments

Marcadores temporários registram somente etapa, evento de carregamento e presença na thread principal; nenhum caminho, URL ou conteúdo de sessão. Compilação continua pela CI devido ao bloqueio anterior dos build scripts locais. Nenhum cenário de notificação, cofre gráfico, início automático ou suspensão foi aprovado nesta rodada.

### Diagnóstico nativo

A CI de `461ad48` passou nos dois SOs. No Windows 11, a segunda abertura produziu, em ordem: `open entered main=true`, `state ready`, `build starting`, `build finished`, `close entered`. Nenhum evento de carregamento da página ocorreu antes do fechamento. A segunda execução encerrou e apenas uma instância permaneceu.

A criação não ficou presa no builder nesta reprodução. O caminho de perda de foco fechou a janela ainda oculta, antes do `PageLoadEvent::Finished` que a apresentaria. A correção em avaliação ignora `Focused(false)` no Windows enquanto a janela está invisível; eventos após apresentação conservam o comportamento existente. O restante dos callbacks não foi alterado.

Não há seam automatizado atual que atravesse criação WebView2, mensagens reais de foco e carregamento da página. Um teste unitário de booleanos não reproduziria a falha. A regressão nesta etapa é o loop nativo original, complementado por clique real e reabertura após perda de foco; a lacuna de automação gráfica permanece explícita.

### Resultado gráfico da correção

A [CI de a055512](https://github.com/lucasvss2/pc-monitor/actions/runs/41839042788) passou integralmente nos dois SOs. A repetição nativa mudou de zero para uma janela visível `kvojps-monitor`, com uma instância do processo. A automação leu a árvore de acessibilidade e inspecionou visualmente a apresentação das Configurações, sem conteúdo pessoal e sem gravar captura no repositório. A sequência observada passou por `build finished` sem o fechamento prematuro anterior.

O teste seguinte ativou uma janela existente do Firefox para causar perda de foco. A automação foi interrompida pelo mecanismo de Computer Use porque a aplicação de política de URLs não é suportada nesse navegador Windows. Nenhuma interação posterior pela automação foi realizada. Não há aceite completo de perda de foco/reabertura, clique real no Indicador após a correção, menu direito, hooks reais, banner/Central, cofre gráfico, início automático ou suspensão/retomada.

Os seis marcadores temporários foram removidos do código. O teste nativo anterior fornece o vermelho/verde para a abertura inicial; falta repetir os demais cenários e o EXE sem instrumentação numa próxima sessão. O ticket permanece `ready-for-human` para esse aceite, sem declarar a validação gráfica inteira concluída.

### Limpeza desta sessão

Instância de teste encerrada; nenhum processo do monitor permaneceu. Removidos os EXEs temporários das rodadas utilizadas, logs privados, instalador e instalação Rust isolada usados neste trabalho. Também foram removidos os registros COM/AppUserModelId cuja linha de ativação apontava exatamente para o EXE temporário descartado, depois de confirmar ausência de instância ativa. Nenhum registro de inicialização automática foi criado. Configurações pessoais de hooks e credenciais não foram alteradas.

Não restaram diretórios temporários `pc-monitor-*` utilizados nesta sessão entre os alvos conferidos. Código, tickets e dependências do projeto em uso foram preservados. O artefato da CI continua disponível conforme sua retenção; uma próxima validação deve baixar o artefato correspondente à fonte sem instrumentação.
