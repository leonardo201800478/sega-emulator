# Controles de áudio do desktop Slint

## Escopo

O menu **Áudio** expõe controles de saída para o runtime SMS atual:

- Volume mestre de 0 a 100%.
- Mute independente, preservando o nível mestre escolhido.
- Atalhos de volume em 50% e 70%.
- Estado de saída e retorno de erros da aplicação do volume.
- Persistência das preferências no arquivo `%APPDATA%/Sega Emulator/slint_ui_settings.txt`.

## Caminho de aplicação

O host chama `SmsTemporalWorker::set_audio_volume`, que envia um comando coarse-grained ao worker proprietário da máquina. O comando atualiza `NativeAudioSession::set_volume` no DSP/resampler de saída. A escala é linear (amostra multiplicada por volume/100) e o clamp PCM normalizado permanece na fronteira do resampler.

A mudança de volume não recria o stream CPAL, não avança nem pausa a máquina e não altera PSG, MachineScheduler, CPU, VDP ou clocks.

O mute aplica volume zero à saída e mantém o volume mestre armazenado para restauração. Volume e mute são aplicados ao criar uma nova sessão quando uma ROM é carregada.

## Valores padrão e compatibilidade

- Instalações sem a chave `audio_volume` usam 70%.
- `audio_volume` fora do intervalo 0–100 é ignorado e o padrão seguro é mantido.
- `audio_muted` ausente usa `false`.
- O formato de preferências existente é estendido sem invalidar as chaves antigas.

## Fora do escopo

Não há equalizador, bass boost, treble, filtro passa-baixa selecionável ou modelo analógico nesta entrega. A filtragem do SN76489 e a reconstrução FIR existentes permanecem inalteradas. Filtros adicionais só devem ser expostos após calibração documentada, comparação espectral e testes de regressão, evitando mascarar divergências na síntese do PSG.

## Validação necessária

Os testes automatizados cobrem a conversão de volume efetivo, o comportamento independente de mute e o valor padrão. Os workflows da PR devem validar compilação Slint/Rust e os gates Windows x64 aplicáveis. A audição em dispositivo físico continua sendo validação manual separada.
