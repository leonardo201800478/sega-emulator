# SMS homebrew e diagnostic ROMs

Este diretório organiza ROMs de diagnóstico/homebrew fornecidas para testes físicos e inspeção do Sega Master System.

## Instalação local

Copie os arquivos .sms para:

    tests/sms/roms/homebrew/

Os binários são ignorados pelo Git intencionalmente. O repositório mantém apenas este catálogo e os SHA-256 para identificar exatamente as imagens usadas nos testes. Compare o hash local com manifest.toml antes de executar uma ROM.

## Conteúdo catalogado

O catálogo contém 17 ROMs. As categorias abaixo são inferidas dos nomes dos arquivos e não substituem a documentação original:

- VDP/vídeo: V Counter, Scroll, Palette, Nine Pixels e Zoom Effect.
- Sprites/prioridade: Interactive Sprite Test, SMS Sprite Test e Sprite and Priority Test.
- Interrupções: Interrupt Test.
- Diagnóstico/utilitários: SMSTestSuite e PFR Detect.
- Workloads/demonstrações: SMS Chip-8, SMS Power Demo e Anniversary Intro.
- Áudio: SMS VGM player.
- CPU/Z80: as duas imagens ZEXALL.

## Integridade observada no ZIP recebido

- 17 arquivos .sms.
- 16 arquivos de 32 KiB e um arquivo de 128 KiB.
- 16 imagens apresentam TMR SEGA no offset 0x7FF0.
- SMS Power Demo Build 31 não apresenta essa assinatura nos offsets comuns verificados (0x1FF0, 0x3FF0, 0x7FF0 e 0xFFF0). Isso é um achado para investigação, não uma conclusão de corrupção.
- Tamanhos e hashes individuais estão em manifest.toml.

## Licença e redistribuição

O ZIP não contém arquivos de licença, fontes, URLs de origem ou declarações de permissão. O sufixo (PD) presente em vários nomes não é evidência suficiente para concluir que a obra está em domínio público. Por isso, os binários não são versionados nem redistribuídos neste repositório até que a licença/proveniência de cada ROM seja verificada.

## Regras de validação do emulador

- A presença de uma ROM e a correspondência de SHA-256 provam apenas identidade/integridade do arquivo, não a correção do emulador.
- Não interpretar resultados de ROMs Z80 como evidência conclusiva antes do fechamento demonstrável da etapa E7.
- Não iniciar E8 nem executar ZEXALL antes do fechamento de E7. A inclusão destas imagens no catálogo não autoriza antecipar essa etapa.
- Testes físicos devem registrar ROM/hash, sistema/região, revisão de hardware, comportamento esperado, comportamento observado e evidência visual/temporal.
- Testes de VDP/counters, interrupções, sprites, áudio e pacing devem ser avaliados separadamente; uma demo visual não é, por si só, um oráculo de precisão temporal.
