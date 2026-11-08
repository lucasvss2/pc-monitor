# GPU no Módulo Sistema Windows

Status: resolved

Implementar uso por GPU e temperatura NVIDIA, preservando o app sem assinatura própria, processo comum sem elevação e política de execução intacta. Usar apenas Windows e o driver gráfico já instalado. Investigar a fonte CPU AMD sem instalar componente privilegiado; documentar indisponibilidade quando não houver sensor suportado.

## Tarefas

1. [Coleta e apresentação GPU](issues/01-gpu-metrics.md).
2. [Fonte CPU AMD](issues/02-amd-cpu-source.md).
3. [Documentação das adaptações Windows](issues/03-windows-documentation.md).
4. [Validação gráfica Windows](issues/04-native-validation.md).

Os percentuais de progresso devem distinguir implementação, checks automatizados e aceite gráfico. Pesquisa de fonte não equivale a suporte implementado. O portátil anterior e as configurações continuam preservados; cópias de teste obsoletas devem ser removidas.

Estimativa em 08/11/2026: cerca de 90% desta frente, com implementação/checks e documentação concluídos, pesquisa AMD encerrada com sensor não suportado nesta máquina, e validação/aceite gráficos nativos pendentes. Os 399 testes UI aprovados não representam conclusão global. CI da candidata `439393f` aprovada nos dois alvos.

Após a abertura autorizada, a prova gráfica nativa passou e a versão portátil estável foi preservada com o perfil anterior. Estimativa atualizada: cerca de 95%; resta o aceite visual final solicitado ao usuário. O incremento não encerra as pendências globais da frente Windows, como suspensão física/login real e o bloqueio do instalador NSIS.

Aceite em 08/11/2026: o usuário confirmou a apresentação correta e legível do uso das duas GPUs e da temperatura NVIDIA nos modos completo e reduzido e aprovou explicitamente o resultado. Esta frente GPU/documentação está 100% concluída dentro do escopo: implementação, pesquisa AMD, documentação, checks da candidata e validação/aceite nativos. Isso não representa 100% da frente Windows; as pendências globais acima continuam abertas. A investigação AMD foi concluída com indisponibilidade comprovada do sensor neste driver, sem implementar temperatura CPU.
