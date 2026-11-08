# Coleta e apresentação GPU

Status: resolved
Assignee: Codex
Parent: ../spec.md

## Critérios

- GPU % por contadores Windows, agregada por adaptador e motor; não somar motores paralelos.
- Temperatura NVIDIA pela NVML já instalada em System32, associada ao mesmo adaptador pela localização PCI, sem DLL privada ou busca no PATH.
- Nenhum driver/serviço instalado, elevação ou mudança nas proteções.
- GPU ausente, medindo, indisponível e falha distintos de zero válido.
- Coleta pela porta `Readings`, somente com Sistema visível, reinício das amostras após retomada.
- UI completa por GPU e resumo no reduzido; CPU/RAM continuam utilizáveis quando GPU falhar.
- Testes de agregação, privacidade e ciclo de vida; fixtures públicas sanitizadas ou sintéticas identificadas.

## Comments

Implementação iniciada em 08/11/2026 na branch `codex/windows-gpu-metrics`.

A revisão `439393f` passou integralmente na [CI nativa](https://github.com/lucasvss2/pc-monitor/actions/runs/42499337778) Ubuntu/Windows, incluindo 399 testes UI, testes de agregação/privacidade/ciclo de vida, build e instalação/remoção por usuário. A fonte usa as DLLs já instaladas; o teste local sem GUI do EXE sem assinatura terminou com código zero e sem saída, sem elevação e com Controle Inteligente ativo. Abertura e aceite gráficos do coletor real continuam no ticket 04.

A validação assistida abriu a revisão correta no Windows 11, confirmou uso dos dois adaptadores e temperatura somente na NVIDIA, comparação com a ferramenta do driver, estados de medição após retorno e leitura retomada após X/reabertura. A implementação está concluída. O ticket 04 mantém separado o aceite final da apresentação pelo usuário.
