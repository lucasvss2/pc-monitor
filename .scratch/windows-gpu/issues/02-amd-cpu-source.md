# Fonte CPU AMD

Status: resolved
Assignee: Codex
Parent: ../spec.md

## Resultado

A pesquisa identificou o sensor oficial ADL PMLog 32 para CPU. A sondagem de leitura pela DLL AMD já instalada inicializou a API e confirmou a exportação, porém os adaptadores consultados não informaram suporte a esse sensor nesta máquina. Nenhum driver ou serviço foi instalado. CPU °C continua indisponível; não usar temperatura GPU/ACPI como substituto nem publicar zero artificial.

Fonte e limites em [pesquisa AMD](../../jira-windows/research/windows-amd-cpu-temperature.md). Isso conclui a investigação solicitada, sem declarar suporte de temperatura CPU.
