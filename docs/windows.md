# Uso no Windows 11

O alvo desta adaptação é Windows 11 x64, com execução na conta comum. O núcleo Rust e a interface Svelte são compartilhados com Ubuntu; integrações nativas substituem os serviços específicos do Linux. O suporte segue experimental enquanto os [aceites pendentes](windows-validacao.md) não forem concluídos. Uma implementação na árvore de desenvolvimento não significa que as mesmas funções existam nos pacotes de releases anteriores.

## Distribuição e requisitos

O app não exige assinatura Authenticode própria nem administrador no fluxo normal. Precisa de WebView2 e, para métricas específicas, dos drivers de hardware já instalados. Nenhum coletor instala driver, serviço privilegiado, certificado ou SDK.

- **ZIP portátil:** o artefato `windows-foundation-experimental-unsigned` da CI contém `kvojps-monitor.exe`. Extraia em pasta estável, execute e abra o Painel pelo Indicador perto do relógio. Não há desinstalador. A pasta do EXE não isola o perfil: o app usa as preferências e o cofre da sua conta.
- **Instalador EXE:** o artefato `windows-x64-installer-unsigned` instala por usuário e verifica WebView2. Sua remoção preserva preferências/credenciais por padrão. Consulte o [roteiro de instalação](windows-validacao.md#instalador-exe-por-usuário).

O Controle Inteligente de Aplicativos pode bloquear um arquivo novo sem assinatura. Na validação de `26c51bc`, o instalador NSIS foi bloqueado e o portátil da mesma revisão foi permitido com a proteção ativa; isso não garante aceitação de toda versão ou máquina. Não desative proteções nem remova marcas de origem para instalar. A atestação/checksum de um artefato comprova sua identidade, sem substituir a política do Windows.

Antes de trocar de versão ou mover o EXE, encerre por **Indicador → Sair**. Preserve a cópia anterior até validar a nova. Hooks, início automático e notificações podem conter referências ao caminho do programa: mantenha o novo EXE em um local estável e use os controles do app para conectar/desconectar recursos. Não edite arquivos de configuração de outros programas manualmente para fazer a troca. Remova cópias de teste e instaladores obsoletos depois de conferir os registros em uso; preserve as preferências e credenciais úteis.

## O que foi adaptado

| Área | Fonte e comportamento Windows |
| --- | --- |
| Indicador e Painel | Clique esquerdo abre/ativa; direito abre o menu. X/Escape fecham o Painel. Clicar fora o mantém aberto. Modo reduzido, fixação e geometrias são lembrados. |
| Relógio | Hora e fuso do próprio Windows, sem sincronizador externo do app; formato pt-BR, 24 h com segundos e data. |
| Credenciais | Windows Credential Manager, restrito ao usuário e à máquina. Sem cofre real, salvar é recusado. |
| Codex | CLI nativo e app-server para limites; consulta periódica de sessões com evidência de processo vivo. Histórico isolado não basta para sessão ativa. |
| Claude Code | Status line após conexão explícita, com encadeamento e restauração da configuração anterior. OAuth opcional permanece indisponível neste Windows. |
| Hooks | Comandos Windows e pipe local. Instalação só após clique, aditiva e reversível. WSL e sessões exclusivas de apps/IDEs não estão cobertos pelo mínimo. |
| Notificações | Banner e Central com ativação para Agentes. Entrega depende também das preferências e políticas de notificações do Windows. |
| Tarefas/GitHub | Integrações HTTP compartilhadas; somente o provedor de Tarefas selecionado coleta. Jira exige conexão da própria conta no app. |
| Rede | Rota/interface ativa, contadores totais, ICMP e teste de velocidade. Sem tráfego por aplicativo ou detalhes de segurança avançados nesta versão. |
| Sistema | `GetSystemTimes`, `GlobalMemoryStatusEx`, discos fixos e bateria pelas APIs Windows. Ausência de bateria difere de falha; Swap continua métrica exclusiva do Linux. |
| GPU, novo incremento | DXGI identifica cada adaptador; PDH lê uso dos motores; NVML do driver já instalado fornece temperatura NVIDIA. Compilação e aceite deste incremento são registrados no [tracker GPU](../.scratch/windows-gpu/spec.md). |

## Uso e temperatura das GPUs

Cada GPU tem sua própria leitura. Para o percentual, o coletor soma processos que usam o mesmo motor e apresenta o motor mais ocupado, sem somar motores paralelos. Isso evita percentuais artificiais acima de 100% e mantém visíveis cargas de vídeo/compute além de 3D. O valor pode diferir de outras ferramentas por intervalo e definição de uso.

A temperatura NVIDIA vem de `nvml.dll` já instalada em System32, carregada sem procurar DLLs no PATH ou na pasta de trabalho. A associação usa o mesmo adaptador/localização PCI: o uso de uma GPU não deve ser acompanhado da temperatura de outra. NVIDIA ausente, biblioteca ausente, sensor não suportado e falha de leitura têm indicação explícita. A rota inicial não promete temperatura AMD/Intel.

Na primeira amostra ou após retomar a coleta, GPU % pode aparecer como **Medindo…**. Ausência ou erro não significa 0% ou 0 °C; zero só é publicado quando uma leitura válida o informa. As GPUs aparecem no Sistema completo e no resumo reduzido. A coleta acompanha o ciclo de dois segundos de Sistema enquanto ele estiver visível; ao ocultar, pausa, e ao retomar descarta a referência anterior dos contadores. O consumo de uma GPU adormecida depende também do driver e precisa de avaliação por hardware.

### Temperatura da CPU AMD

O WMI padrão não forneceu uma leitura de CPU na máquina sondada. A API oficial ADL do componente AMD instalado contém um sensor CPU (PMLog 32), mas a consulta aos adaptadores enumerados não informou suporte a ele. Nenhum driver ou serviço adicional foi instalado. Assim, a temperatura CPU continua explicitamente indisponível nessa configuração; não usamos temperatura de GPU ou zona ACPI como substituta.

Os detalhes e as fontes primárias estão nas pesquisas de [GPU/temperaturas](../.scratch/jira-windows/research/windows-gpu-thermal.md) e [CPU AMD](../.scratch/jira-windows/research/windows-amd-cpu-temperature.md). Suporte em outro driver/modelo depende de nova prova de capacidade e permissões.

## Privacidade e validação

Identificadores PCI/LUID e instâncias de contadores são usados somente em memória para associação. Snapshots de GPU contêm rótulo ordinal/fabricante, estados e valores agregados; não publicam PID, UUID, endereço PCI, nomes de processos ou conteúdo do usuário. Não há telemetria nem servidor próprio.

As fontes de CPU/RAM/GPU passam pela porta injetável `Readings`; regras e UI são testadas com fixtures sanitizadas ou variantes sintéticas identificadas. A CI Windows valida compilação/APIs disponíveis no runner e preserva os checks Ubuntu. Abertura real, hardware NVIDIA e preservação do perfil exigem [validação gráfica](windows-validacao.md), separada dos testes automáticos. Reconexão, suspensão física, login real e demais cenários adiados continuam pendentes até seu aceite específico.
