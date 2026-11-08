# Coleta e apresentação GPU

Status: ready-for-agent
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
