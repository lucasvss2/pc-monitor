# Reposicionar a janela pelo cabeçalho

Status: resolved
Assignee: Codex / usuário
Parent: ../spec.md

## Escopo e reprodução

Após confirmar X/Escape e a troca de provedores no Windows 11, o usuário informou que não consegue reposicionar a janela e pediu o ajuste. A janela é sem decoração nativa; o cabeçalho desenhado pela aplicação não tinha região de arraste nem a permissão Tauri correspondente.

Este é um ajuste declarativo da integração com o gerenciador de janelas, sem regra nova de coleta, credencial ou geometria persistida. A reprodução é o relato na janela nativa; jsdom não implementa mover uma janela de SO. Não adicionar teste que apenas espelhe a presença de um atributo: as verificações automáticas validam UI/manifesto e preservam interações; o veredito de arraste exige o teste assistido no Windows. Não criar helper alternativo de automação nem pedir coordenadas globais ao Wayland.

## Implementação

Usar data-tauri-drag-region no cabeçalho e em seus descendentes não interativos, incluindo a assinatura e os espaços livres. Botões continuam sem a marcação. Capability limitada à janela panel, com start-dragging e internal-toggle-maximize para os gestos nativos de arraste/duplo clique, sem liberar o conjunto inteiro de permissões de janela. Cursor e dica explicam onde arrastar. A memória de posição entre reaberturas/reinícios continua no plano de geometrias do Painel; este ajuste habilita o movimento atual.

Fonte primária consultada em 04/11/2026: [customização de janela Tauri v2](https://v2.tauri.app/learn/window-customization/). Atributo de arraste se aplica ao elemento marcado; botões e entradas não são transformados em área de arraste. Validação e artefato nativo ainda em preparação. Usar a conexão Jira já salva, sem pedir/recriar token e sem alterar itens remotos.

## Validação em andamento

Código c53247a, CI 41923314348. Localmente, Svelte sem erros/avisos, build UI e 58 testes de interação do Painel passaram. O runner Windows passou todas as etapas, com 391 testes UI, 269 unitários Rust, cofre separado, integrações/fixtures e build de produção. Executável experimental conferido: SHA-256 c87f7c5a6ab74d6fc4a88fb741ca4699af557076e21b34ab1aa19ce36ac1280c, sem assinatura. O arraste real não é declarado aprovado pela presença do atributo nem pelo cursor; aguarda a janela nativa e a confirmação assistida do usuário.

A CI terminou com sucesso também no Ubuntu, incluindo o build de produção. O arquivo novo está em pasta temporária própria, separado da versão 5b4034d ainda em uso. Solicitado ao usuário encerrar a versão anterior e autorizar a abertura do EXE conferido, conforme a confirmação imediata exigida pela skill computer-use. Não iniciar segunda versão por cima da instância antiga; conferir o caminho do processo antes do aceite e remover o EXE obsoleto após a substituição. A conexão Jira deve reaparecer sem pedir token após esse reinício, e o teste de arraste deve conferir também os botões do cabeçalho.

O usuário confirmou encerramento e autorizou a abertura de c53247a. Hash conferido novamente e ausência de instância antiga verificada antes da execução; depois, uma única instância do caminho esperado foi confirmada. A automação novamente não expôs janela alvo, portanto o teste segue assistido. Cópia local obsoleta de 5b4034d removida após conferir hash, ausência de processo usando-a e limites da pasta temporária. Nenhuma chave do Registro foi excluída; associações usadas pela versão atual são preservadas.

O usuário confirmou na janela nativa que o arraste moveu a janela e que o Jira voltou conectado sem pedir token. Isso também confirma leitura da conexão salva após encerrar um processo e iniciar a versão seguinte. Teste dos botões Configurações/X após o arraste ainda em confirmação. A posição não foi gravada como preferência, conforme o recorte deste ajuste.

## Resultado

Aceite assistido concluído no Windows 11: arraste move a janela; Jira reaparece conectado após substituição/reinício do processo; Configurações, X e reabertura pelo Indicador funcionam depois do movimento. Confirmações fornecidas pelo usuário, sem automação de autenticação ou capturas novas. CI Ubuntu/Windows aprovada, sem alteração de proteções. Cópia local e artefato remoto obsoletos de 5b4034d removidos; apenas a versão c53247a continua em uso. Metadados de seleção continuam sem conta/quadro/conteúdo, sem limpeza pendente e sem cache Jira em disco. As associações do aplicativo em uso foram preservadas; não houve nova tentativa de exclusão do Registro.

Limites: o movimento atual foi verificado; memória de posição/geometrias e aceite gráfico Ubuntu continuam nas etapas aprovadas. Nenhuma transição remota em itens foi solicitada nesta rodada.
