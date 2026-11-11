# OUT-B — integração dos backends nativos com PCM frame FIFO

**Status:** IMPLEMENTADA E INTEGRADA.
**PR:** [#654](https://github.com/leonardo201800478/sega-emulator/pull/654), squash merge.
**Merge commit:** `0a7a0579d01277fcccc21f6ad45190891604919d`.
**HEAD validado:** `3f2d4ebf3e4514ae2f8497bcae6f4a8521313cd9`.
**CI:** workflow #5678, conclusão `success` no HEAD exato.
**Limite:** reprodução física após esta migração específica permanece **NOT RUN**.

## 1. Estado integrado

- OUT-A fornece `PcmFrameProducer`, `PcmFrameConsumer` e `StereoFrame = [i16; 2]` em FIFO SPSC limitada.
- WASAPI e XAudio2 consomem a mesma FIFO frame-based.
- `NativeAudioSession` continua publicando PCM16 mono via `PcmSink::write_pcm(Vec<u8>)`.
- A conversão mono → estéreo permanece na fronteira de saída; PSG, mixer e DSP não foram reestruturados.
- `rtrb` permanece na linha 0.3, lockfile em 0.3.6; nenhuma atualização de dependência foi incluída.

## 2. Invariantes da FIFO compartilhada

1. Exatamente um produtor e um consumidor por endpoint SPSC.
2. Unidade de transporte: frame estéreo PCM16 intercalado [L, R], quatro bytes por frame.
3. Overflow drop-newest, contabilizado por frame; frames já enfileirados nunca são sobrescritos.
4. Leitura não bloqueante e sem alocação; underflow preenche somente a região faltante com silêncio.
5. A fila não possui clock e não controla o MachineScheduler.
6. O produtor converte PCM16 mono para frames estéreo em lotes fixos de até 256 frames.
7. Nenhuma memória reutilizável da FIFO pode ser entregue como `pAudioData` ao XAudio2.

## 3. XAudio2 — staging e ownership

- Capacidade da FIFO: `buffer_size × queue_blocks` frames.
- O worker é o único consumidor e mantém pool fixo de staging.
- Cada lote submetido é copiado para uma alocação própria e estável de `Vec<StereoFrame>`.
- `pAudioData` aponta para o staging, nunca para a FIFO/scratch reutilizável.
- Buffers permanecem vivos até o worker observar `OnBufferEnd` no playback FIFO normal ou até `DestroyVoice` no encerramento.
- `OnBufferEnd` só incrementa sinalização atômica; `OnVoiceError` sinaliza falha atomicamente.
- Callback não aloca, bloqueia, registra logs, executa I/O ou libera buffers.
- Falha de `SubmitSourceBuffer` faz rollback do staging rejeitado.
- Flush não faz reclaim individual por ordem de callback; os buffers permanecem vivos até a barreira de segurança documentada.

## 4. Critérios de aceitação

- [x] WASAPI usa a FIFO frame-based.
- [x] XAudio2 usa `PcmFrameProducer/PcmFrameConsumer`.
- [x] Conversão mono PCM16 → estéreo em lotes fixos de 256 frames.
- [x] Contagem exata de frames descartados pela política drop-newest.
- [x] Host não-Windows do crate standalone compilado e testado.
- [x] Staging XAudio2 estável antes de `SubmitSourceBuffer`.
- [x] Callback restrito a sinalização atômica.
- [x] Rollback de falha de submissão.
- [x] Reclaim do playback normal baseado em `OnBufferEnd`.
- [x] Buffers e callback preservados durante o lifetime da source voice.
- [x] Gates Windows e Linux específicos de áudio: PASS.
- [x] Gates do workspace raiz: PASS.
- [ ] Reprodução física no Windows após o HEAD migrado: NOT RUN.

## 5. Validação — CI #5678

HEAD: `3f2d4ebf3e4514ae2f8497bcae6f4a8521313cd9`.

- Windows audio-output fmt: PASS.
- Windows audio-output Clippy `-D warnings`: PASS.
- Windows audio-output tests: PASS — 23 passed, 0 failed.
- Linux audio-output fmt: PASS.
- Linux audio-output Clippy `-D warnings`: PASS.
- Linux audio-output tests: PASS — 23 passed, 0 failed.
- Workspace root fmt: PASS.
- Workspace root Clippy `-D warnings`: PASS.
- `cargo test --workspace --locked`: PASS.
- Windows SDL3, D3D12 compile e PowerShell: PASS.
- Documentação/higiene e quality gate: PASS.
- Reprodução física em endpoint Windows: NOT RUN.

A CI comprova compilação e testes nos hosts configurados. Não comprova saída audível no hardware físico.

## 6. Escopo excluído

OUT-E (mute/flush/epoch/prebuffer e política de StepFrame), OUT-F (telemetria/tuning) e OUT-G (validação física pós-migração) permanecem etapas independentes. Esta integração não alterou Scheduler, PSG, VDP, runtime, DSP ou GUI.

## 7. Referências

- [XAUDIO2_BUFFER](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/ns-xaudio2-xaudio2_buffer).
- [IXAudio2VoiceCallback::OnBufferEnd](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voicecallback-onbufferend).
- [IXAudio2Voice::DestroyVoice](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voice-destroyvoice).
- [IXAudio2SourceVoice::FlushSourceBuffers](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voice-flushsourcebuffers).
- [XAudio2 Callbacks](https://learn.microsoft.com/en-us/windows/desktop/xaudio2/xaudio2-callbacks).
- `windows = 0.62.2`; `rtrb = 0.3` (lockfile 0.3.6).
