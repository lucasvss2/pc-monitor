# Fixtures Jira Cloud

Configuração e primeira página de itens: projeções de leituras reais do serviço via conector Atlassian em 04/11/2026, com somente os campos necessários ao contrato. O conector normaliza a resposta; estes arquivos não são uma captura byte a byte do HTTP. Transições foram lidas com expansão de campos, sem executar nenhuma. O quadro de origem não é identificado aqui.

Sanitização em memória antes da gravação: site/conta/IDs/chaves, nomes de quadro/colunas/estados/tipos/prioridades/responsáveis e conteúdo dos itens substituídos por exemplos. IDs de status preservam as relações entre colunas e itens. Corpos de descrição, comentários, anexos, e-mails, credenciais e links privados foram excluídos. Datas presentes seriam substituídas por uma data sintética fixa. Nenhum mapa de substituição é publicado.

`boards.json` conserva os campos do quadro selecionado com aliases; paginação de um quadro nessa fixture é uma variante sintética. `myself.json`, `issues-last-page.json` e `required-transition.json` são variantes inteiramente sintéticas para identidade, fim de paginação/campos opcionais e formulário obrigatório. A primeira página de itens preserva o indicador de continuidade; o token de próxima página é um alias sintético. Erros e sentinelas adicionais nos testes também são sintéticos.

Referências revalidadas em 04/11/2026: [quadro e paginação enhanced](https://developer.atlassian.com/cloud/jira/software/rest/api-group-board/), [transições](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/), [autenticação pessoal](https://developer.atlassian.com/cloud/jira/platform/basic-auth-for-rest-apis/). O conector deste chat não valida a autenticação própria do aplicativo. Não foi feita escrita no quadro real.
