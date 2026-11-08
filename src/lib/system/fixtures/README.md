# Snapshots de Sistema

Os snapshots existentes de notebook/desktop são gerados pelos testes Rust de Sistema. `windows-gpu.json` é uma variante inteiramente sintética do contrato de snapshot, criada para verificar dois adaptadores com capacidades térmicas distintas. Valores, rótulos e armazenamento são exemplos, sem dados do equipamento do usuário. As relações e estados seguem os tipos serializados pelo núcleo; não é captura de hardware nem comprovação de integração nativa. A fonte dos nomes de contador e a proveniência dos testes de agregação estão em `src-tauri/fixtures/system/README.md`.
