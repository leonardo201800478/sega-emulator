# OUT-B — migração inicial do WASAPI para PCM frame FIFO

**Status:** implementação parcial em branch; aguarda CI Windows.
**Base empilhada:** PR #649 (`feat/audio-pcm-frame-conversion-20261108`).
**Escopo:** migrar somente o transporte producer/consumer do WASAPI para `PcmFrameProducer` / `PcmFrameConsumer`.

## Implementação

- A fila WASAPI agora transporta `StereoFrame = [i16; 2]` em vez de `Vec<u8>`.
- A capacidade é calculada em frames: `buffer_size * queue_blocks`.
- O produtor converte PCM16 mono para um lote de frames estéreo antes de publicar; a alocação ocorre apenas no lado produtor.
- O worker WASAPI é o único consumidor e usa `read_frames` para consumir frames, preencher somente o sufixo ausente com silêncio e detectar encerramento do produtor após drenar dados já publicados.
- O formato WASAPI é validado como estéreo PCM16, com `block_align = 4` bytes por frame.
- O scratch de frames e o staging `VecDeque<u8>` são alocados uma vez por stream e reutilizados no loop de eventos.
- A recuperação atual do endpoint e o modo Event-Driven foram preservados.

## Limites explícitos

- XAudio2 permanece em `RingBuffer<Vec<u8>>`; não faz parte desta alteração.
- Não altera `PcmSink`, `NativeAudioSession`, DSP/resampler, Scheduler, worker SMS, controles Pause/Stop/Resume/StepFrame ou GUI.
- Não declara OUT-B completa nem validação física de áudio.
- O contador público `dropped_block_count` continua contando chamadas de escrita que sofreram alguma perda; a FIFO mantém a contagem exata de frames descartados em suas métricas.

## Validação necessária

- [ ] CI Windows no HEAD exato: fmt, Clippy com `-D warnings` e testes.
- [ ] Confirmar compilação do código condicionado a Windows e os testes de conversão/fila.
- [ ] Testes direcionados para lote parcial, overflow, underflow e fechamento/drain.
- [ ] Validação física WASAPI em hardware real permanece pendente.
