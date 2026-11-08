# Fixtures de Sistema

Leituras de `/proc`, hwmon, statvfs e UPower usadas pelas portas injetáveis.

`gpu-engines.json` é uma variante sintética do formato de instâncias observado em uma consulta local de `Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine` em 08/11/2026 e documentado pelo Windows. PIDs, LUIDs e percentuais são exemplos deliberados, sem relação com processos ou atividade reais. Inclui processos no mesmo motor, motores paralelos, nós físicos distintos e dois adaptadores para verificar agregação e isolamento. Nenhuma saída bruta foi gravada. As sondagens nativas NVIDIA e AMD publicam somente disponibilidade/resultado, conforme as pesquisas em `.scratch/jira-windows/research/`.

Em `notebook/proc/self/mountinfo`, o formato e as classes de sistemas de arquivos da captura foram mantidos; IDs de montagem/dispositivos, nomes de pacotes snap e caminhos de namespace foram substituídos por exemplos. A topologia publicada não deve ser usada para identificar o host de origem.
