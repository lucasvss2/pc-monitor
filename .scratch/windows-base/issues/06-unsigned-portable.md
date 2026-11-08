# Alternativa portátil ao instalador bloqueado

Status: resolved
Assignee: Codex / usuário
Parent: ../spec.md

## Escopo

Investigar uma distribuição sem assinatura que respeite as proteções atuais do Windows 11. Preservar o portátil anterior, as preferências e as credenciais. Não prometer execução de código recusado pelo Controle Inteligente de Aplicativos.

## Diagnóstico

O usuário reproduziu o bloqueio do instalador da revisão `26c51bc`. Sete eventos 3077 identificam o EXE NSIS, a mesma política `VerifiedAndReputableDesktop` e status `0xc0e90002`. O hash do artefato oficial corresponde ao arquivo bloqueado. A repetição do instalador não foi automatizada, pois poderia instalar o produto caso a avaliação mudasse; os eventos correlacionados são a evidência do bloqueio anterior à entrada no programa.

Hipóteses avaliadas: bloqueio restrito ao NSIS; recusa também do aplicativo; divergência do artefato baixado. A divergência foi descartada pela comparação de hashes. O EXE portátil da mesma CI foi permitido em subprocesso silencioso com stdin vazio e argumentos `hook unsupported unsupported`, código zero, sem stdout/stderr. Isso separa os arquivos avaliados pelo Windows, mas não comprova inicialização gráfica completa.

## Alternativa preparada

Usar o ZIP portátil já produzido pela CI para esta revisão, sem NSIS, com WebView2 existente. Conferido o digest do ZIP na API do GitHub e o hash do EXE; nenhuma assinatura, política, marca de origem ou configuração de segurança foi alterada. Não há instalador alternativo nem certificado autoassinado.

## Validação gráfica

Após confirmação de execução, a interface abriu a revisão correta, verificada pelo hash. A primeira tentativa resolveu a identidade registrada para uma revisão anterior e foi descartada; só o processo introduzido pela tentativa foi encerrado. A abertura direta pelo identificador de processo iniciou o candidato correto. O Painel abriu após uma segunda ativação, com uma única instância, relógio ampliado avançando, conexão Jira retomada e Configurações com retorno ao reduzido.

A mesma cópia foi preservada em pasta estável por usuário, fora de Temporários, e reaberta pela interface. O Controle Inteligente permaneceu `On`. Conferidos os campos de aparência, notificações, Âncora, modo e fixação entre a promoção e a reabertura; o hash do portátil anterior foi preservado. O hash integral original das preferências mudou durante as inicializações e não é apresentado como igualdade byte a byte. Somente a pasta isolada do diagnóstico e seu manifesto foram removidos, após conferir processos e referências. O EXE estável em uso e o portátil anterior continuam disponíveis.

## Aceite do usuário — 07/11/2026

O usuário confirmou fechamento pelo X e reabertura pelo Indicador da cópia estável, com o relógio ampliado, Jira conectado e sem aviso de bloqueio, aprovando explicitamente o resultado. O recorte de alternativa portátil e apresentação do relógio está concluído nesta máquina.

O instalador NSIS continua bloqueado: a alternativa aceita é o ZIP portátil, sem garantia de execução em outras máquinas ou após mudanças na avaliação do Windows. O resultado não encerra os demais aceites da frente Windows.

Evidências e instruções: [validação Windows](../../../docs/windows-validacao.md#zip-portátil-sem-assinatura). O usuário aprovou o visual do relógio no navegador e depois a reabertura da versão nativa.
