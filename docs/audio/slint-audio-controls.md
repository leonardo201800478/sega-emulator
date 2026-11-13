# Controles de áudio e DSP do desktop Slint

## Saída de áudio

O menu **Áudio** oferece volume mestre de 0 a 100%, mute independente, atalhos de 50%/70%, estado da saída e persistência em `%APPDATA%/Sega Emulator/slint_ui_settings.txt`.

As alterações são encaminhadas por comandos coarse-grained ao `SmsTemporalWorker`. Volume e filtros não reiniciam CPAL, não avançam nem pausam a máquina e não alteram PSG, CPU, VDP, MachineScheduler ou clocks.

## Processamento DSP opcional

O submenu de som oferece:

- **Preset SMS1 — experimental:** ativa passa-altas de 20 Hz e passa-baixas de 18 kHz.
- **Passa-altas / acoplamento AC:** habilitação independente e ajuste de 5 a 500 Hz.
- **Passa-baixas:** habilitação independente e ajuste de 4 a 20 kHz.
- **Desativar todos os filtros:** retorna ao caminho digital sem processamento analógico adicional.

O preset é uma aproximação de saída, não uma reconstrução elétrica calibrada de uma revisão específica. O manual de serviço do Master System indica faixa de saída de 20 Hz a 20 kHz e nível máximo de 300 mVp-p no pino 10 do IC5; também lista Q48, C49, R47 e R46 entre os pontos a verificar em falhas de áudio. Isso não fornece a função de transferência completa, impedância de carga nem a constante RC efetiva da placa. Portanto, 20 Hz e 18 kHz são valores experimentais selecionados para permitir avaliação auditiva, não valores medidos de corte dos componentes.

## Caminho de sinal e invariantes

`PSG → mixer → AnalogOutputProcessor opcional → FIR/resampler → PCM sink`

- O processador opera na taxa PCM interna de 48 kHz, antes da conversão para a taxa negociada com o dispositivo.
- HPF e LPF são filtros IIR de primeira ordem, com estado persistente entre blocos.
- Atualizar a configuração recria apenas o estado dos filtros; não recria CPAL.
- Pause/Resume preserva a história dos filtros. Reset/Load ROM reinicia o estado DSP; blocos descartados pelo debugger não contaminam o próximo trecho audível.
- A configuração desativada é bypass: não acrescenta filtragem ao caminho original.
- Nenhum filtro é executado por callback de instrução ou por callback do dispositivo de áudio.

## Validação

Testes unitários cobrem validação de frequências, rejeição de DC pelo HPF, bypass exato, perfil experimental e continuidade entre blocos. Os workflows da PR devem validar Slint, Clippy, testes e build Windows x64. A comparação auditiva com hardware real e captura de resposta em frequência continuam pendentes; não são declaradas como PASS.
