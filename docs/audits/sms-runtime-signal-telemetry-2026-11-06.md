# SMS runtime — instrumentação de sinais VDP/PSG

Data: 2026-11-06

## Objetivo

Expor métricas do caminho de emulação temporal SMS sem enviar dados por instrução à GUI e sem alterar clocks, registradores ou estados do hardware.

## Métricas

A propriedade `TemporalSMSMachine.runtime_signal_metrics` retorna os valores coletados no último frame concluído:

| Campo | Significado |
|---|---|
| `non_black_pixels` | Pixels RGB com ao menos um canal diferente de zero no framebuffer VDP |
| `vdp_mode4_enabled` | Bit Mode 4 do registrador 0 |
| `vdp_display_enabled` | Bit Display Enable do registrador 1 |
| `vdp_nonzero_vram_bytes` | Bytes não nulos da VRAM |
| `vdp_nonzero_cram_bytes` | Bytes não nulos da CRAM |
| `psg_samples` | Amostras drenadas do SN76489 no frame |
| `psg_nonzero_samples` | Amostras com módulo maior que 1e-9 |
| `psg_peak_amplitude_micros` | Pico absoluto de amplitude multiplicado por 1.000.000 |

Os valores são inteiros na fronteira PyO3. As amostras PCM continuam exclusivamente no backend Rust.

## Interpretação

- `non_black_pixels = 0`, VRAM/CRAM zeradas e display desabilitado: o VDP ainda não recebeu configuração/dados relevantes; investigar boot Z80, mapa da ROM e escritas de I/O.
- VRAM/CRAM não nulas e display habilitado, mas pixels pretos: investigar renderização SMS1, endereçamento de tiles/paleta e estado de raster.
- Pixels não pretos no contador, mas janela preta: investigar transporte de frame, upload, swap chain e viewport do host.
- `psg_samples = 0`: investigar clock domain e geração de amostras do PSG.
- Amostras presentes, mas `psg_nonzero_samples = 0`: investigar inicialização/configuração do PSG e writes aos ports.
- Amostras não nulas e pico positivo, mas silêncio físico: investigar conversão PCM, fila e backend de saída.

Essas métricas localizam a fronteira de falha, mas não provam isoladamente que o jogo está correto ou que o dispositivo físico emitiu som.

## Custo e determinismo

A coleta é feita uma vez por frame concluído no worker Rust. Não cria callbacks por instrução, não altera Scheduler, clocks, CPU, VDP ou PSG e não participa da decisão de execução. O scan de framebuffer/VRAM/CRAM é observabilidade, não lógica de hardware.

## Limitações

A telemetria descreve somente o último frame concluído. Ainda não registra PC/opcode, histórico completo de portas, RMS por janela, estado da fila de áudio ou conteúdo do buffer entregue ao dispositivo. A validação visual/sonora com ROM real continua obrigatória.
