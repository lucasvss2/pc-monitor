# Desligar início automático falha no Windows

Status: ready-for-agent
Assignee: Codex
Parent: ../spec.md

## Reprodução e limpeza imediata

No EXE `1d38623`, a opção Iniciar com o login estava desligada. Clicar para ligar criou somente HKCU Run/kvojps-monitor apontando para o EXE de teste com --autostart. Clicar para desligar exibiu Não foi possível alterar a inicialização com o login e deixou a entrada ativa.

A preferência original foi restaurada imediatamente por remoção somente desse valor, após verificar que seu comando correspondia exatamente ao executável temporário deste teste. Nenhuma outra entrada Run foi alterada; a consulta confirmou ausência do valor em seguida. A interface não foi usada como evidência de remoção bem-sucedida.

## Causa e regressão

O código da versão windows-registry 0.6.1 confirma que Key.open solicita somente leitura. O adaptador usava esse handle para remove_value, operação que exige escrita. A criação funciona porque create abre com escrita. A remoção direta pelo usuário de teste funcionou, portanto o erro não comprova falta de permissão do usuário na chave.

O adaptador recebe internamente o caminho da chave para permitir um teste nativo usando uma chave HKCU sintética isolada, sem tocar Run. O teste percorre escrita, leitura, remoção repetida e preservação de outro valor sintético, com limpeza RAII. A correção só será aplicada após observar essa regressão falhar na CI Windows.

## Critérios

- Remoção abre a chave com permissão necessária, sem criar chave ausente nem apagar valores de outros apps.
- Teste nativo passa e remove a chave de teste mesmo em falha.
- Repetir ligar/desligar na UI e confirmar estado original desligado.
- Abertura --autostart silenciosa não cria segunda instância; logon real continua separado do teste de preferência.

## Comments

A [CI de ebd962c](https://github.com/lucasvss2/pc-monitor/actions/runs/41850404754) confirmou a regressão no Windows: a primeira remoção da chave sintética retornou startup setting unavailable; 241 outros testes passaram. O teste não acessou Run. A correção abre somente a chave solicitada com OpenOptions.write e mantém o tratamento idempotente de valor/chave ausente, sem criar chave durante remoção.
