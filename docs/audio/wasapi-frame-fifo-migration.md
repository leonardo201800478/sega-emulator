# OUT-B — migração inicial do WASAPI para PCM frame FIFO

**Status:** transporte WASAPI migrado e validado por CI Windows; OUT-B permanece parcial.
**Integração:** PR #650, merge commit `d8e1b657354be1096973c321ee6c44cb3b40dc29`.
**HEAD validado:** `7829b1f3adf38889b72514db41ad1301fef1682a`.
**Escopo:** migrar somente o transporte producer/consumer do WASAPI para `PcmFrameProducer` / `PcmFrameConsumer`.

## Implementação

- A fila WASAPI transporta `StereoFrame = [i16; 2]` em vez de `Vec<u8>`.
- A capacidade é calculada em frames: `buffer_size * queue_blocks`.
- O produtor converte PCM16 mono para um lote de frames estéreo antes de publicar; a alocação ocorre apenas no lado produtor.
- O worker WASAPI é o único consumidor e usa `read_frames` para consumir frames, preencher somente o sufixo ausente com silêncio e detectar encerramento do produtor após drenar dados já publicados.
- O formato WASAPI é validado como estéreo PCM16, com `block_align = 4` bytes por frame.
- O scratch de frames e o staging `VecDeque<u8>` são alocados uma vez por stream e reutilizados no loop de eventos.
- A recuperação atual do endpoint e o modo Event-Driven foram preservados.

## Limites explícitos

- XAudio2 permanece em `RingBuffer<Vec<u8>>`; sua migração será um incremento separado, após análise formal de ownership/lifetime dos buffers submetidos.
- Não altera `PcmSink`, `NativeAudioSession`, DSP/resampler, Scheduler, worker SMS, controles Pause/Stop/Resume/StepFrame ou GUI.
- Não declara OUT-B completa nem validação física de áudio.
- O contador público `dropped_block_count` continua contando chamadas de escrita que sofreram alguma perda; a FIFO mantém a contagem exata de frames descartados em suas métricas.

## Validação CI — HEAD acima

- [x] Windows x64: `cargo fmt --manifest-path native/audio_output/Cargo.toml -- --check` — PASS.
- [x] Windows x64: Clippy com `--all-targets --target x86_64-pc-windows-msvc -- -D warnings` — PASS.
- [x] Windows x64: testes do workspace standalone `native/audio_output` — PASS, 16 aprovados, 0 falhas.
- [x] Higiene do patch de documentação — PASS.
- [x] Scheduler deterministic stress, perfil curto — PASS (executado por acionamento explícito; não é teste de áudio).
- [x] Gate Rust quality and unit regression wrapper — PASS.
- [ ] Regressão global do workspace — NOT RUN; o classificador de impacto não a selecionou para esta alteração.
- [ ] Validação física WASAPI em hardware real — NOT RUN.

## Próximos incrementos

1. XAudio2: manter memória própria de cada `XAUDIO2_BUFFER::pAudioData` até `OnBufferEnd` ou parada/destruição segura da voz; nunca submeter ponteiro diretamente para memória reutilizável da FIFO.
2. OUT-C: adaptação Event-Driven WASAPI em incremento próprio, se necessária após auditoria.
3. OUT-E: políticas Pause/Stop/Resume/StepFrame, mute/flush/epoch/prebuffer, sem misturar com esta migração.
4. OUT-G: validação física em dispositivo Windows real.

A CI comprova compilação Windows, lint e testes unitários do crate; não comprova saída audível em hardware físico.
