# GPU no Módulo Sistema Windows

Status: ready-for-human

Implementar uso por GPU e temperatura NVIDIA, preservando o app sem assinatura própria, processo comum sem elevação e política de execução intacta. Usar apenas Windows e o driver gráfico já instalado. Investigar a fonte CPU AMD sem instalar componente privilegiado; documentar indisponibilidade quando não houver sensor suportado.

## Tarefas

1. [Coleta e apresentação GPU](issues/01-gpu-metrics.md).
2. [Fonte CPU AMD](issues/02-amd-cpu-source.md).
3. [Documentação das adaptações Windows](issues/03-windows-documentation.md).
4. [Validação gráfica Windows](issues/04-native-validation.md).

Os percentuais de progresso devem distinguir implementação, checks automatizados e aceite gráfico. Pesquisa de fonte não equivale a suporte implementado. O portátil anterior e as configurações continuam preservados; cópias de teste obsoletas devem ser removidas.

Estimativa em 08/11/2026: cerca de 90% desta frente, com implementação/checks e documentação concluídos, pesquisa AMD encerrada com sensor não suportado nesta máquina, e validação/aceite gráficos nativos pendentes. Os 399 testes UI aprovados não representam conclusão global. CI da candidata `439393f` aprovada nos dois alvos.
