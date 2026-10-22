# Inventário local de firmware e ROMs de referência

## Objetivo

Registrar os arquivos que existiam em `src/firmware/` para orientar testes e integração futuros sem distribuir imagens de firmware/ROM no repositório. Os arquivos listados foram removidos do histórico atual da branch `main` por um novo commit; commits anteriores ainda podem conter os blobs, pois o histórico não foi reescrito.

## Onde manter as cópias locais

Antes de sincronizar a remoção do GitHub, copie os arquivos que deseja preservar de:

`C:\Users\leost\sega-emulator\src\firmware\`

para uma pasta local ignorada pelo Git, recomendada:

`C:\Users\leost\sega-emulator\local-assets\firmware\`

O `.gitignore` ignora tanto `src/firmware/` quanto `local-assets/firmware/`. Não execute `git pull` antes de fazer a cópia de segurança: os arquivos atualmente rastreados em `src/firmware/` serão removidos da árvore de trabalho ao receber o commit de exclusão.

## Inventário registrado

| Arquivo | Sistema / finalidade provável | Uso futuro |
|---|---|---|
| `SF-7000 IPL [v0.61].bin` | SF-7000, carregador/firmware IPL | Apenas em testes da expansão SF-7000; não necessário para a CPU Z80 ou para iniciar cartuchos SG-1000. |
| `SF-7000 Initial Program Loader ROM.sc` | Imagem ROM associada ao SF-7000 | Testes de integração da expansão, após confirmar origem e formato. |
| `sf7000.rom` | Imagem ROM associada ao SF-7000 | Testes de integração da expansão, após confirmar origem e formato. |
| `sf7000_rom.sc.asm` | Fonte assembly relacionada à ROM do SF-7000 | Referência para estudar o software da expansão; verificar licença e procedência antes de reutilizar. |
| `sf7000_rom.sc.font.asm` | Fonte assembly/dados de fonte relacionados ao SF-7000 | Referência auxiliar; verificar licença e procedência antes de reutilizar. |
| `eu_sms.bin` | BIOS europeia do Master System (identificação pelo nome) | Testes futuros do Master System; não usar no marco atual do SG-1000. |
| `us_sms.bin` | BIOS norte-americana do Master System (identificação pelo nome) | Testes futuros do Master System. |
| `jp_sms.bin` | BIOS japonesa do Master System (identificação pelo nome) | Testes futuros do Master System. |
| `gg.bin` | BIOS provável do Game Gear (identificação pelo nome) | Testes futuros do Game Gear. |
| `md.bin` | ROM/firmware de Mega Drive conforme nome local; conteúdo não auditado nesta tarefa | Identificar e validar antes de qualquer teste futuro de Mega Drive. |
| `eu_mcd1_9210.bin` | BIOS europeia do Sega CD, conforme nome local | Testes futuros do Sega CD. |
| `us_scd1_9210.bin` | BIOS norte-americana do Sega CD, conforme nome local | Testes futuros do Sega CD. |
| `jp_mcd1_9111.bin` | BIOS japonesa do Sega CD, conforme nome local | Testes futuros do Sega CD. |
| `32X_G_BIOS.bin` | BIOS do componente gráfico do 32X, conforme nome local | Testes futuros do 32X. |
| `32X_M_BIOS.bin` | BIOS do componente principal do 32X, conforme nome local | Testes futuros do 32X. |
| `32X_S_BIOS.bin` | BIOS do componente secundário do 32X, conforme nome local | Testes futuros do 32X. |

As descrições acima são inferidas principalmente dos nomes dos arquivos, não de uma auditoria binária. Antes de usar qualquer arquivo, confirme tamanho, SHA-256, procedência, licença e compatibilidade com o hardware/revisão alvo.

## Política de testes

- O marco atual é o núcleo Z80 e a memória do SG-1000. Esses arquivos não são dependências dos testes unitários atuais.
- Priorize testes sintéticos e suítes de teste independentes; use homebrew e dumps obtidos legalmente para testes de integração.
- O SG-1000 original inicia o programa do cartucho e não exige uma BIOS de console separada.
- Não adicione firmware/BIOS protegidos por direitos autorais ao Git, releases, pacotes ou artefatos de CI.
- Esta documentação registra nomes e finalidade provável; não armazena as imagens nem comprova sua legitimidade.
