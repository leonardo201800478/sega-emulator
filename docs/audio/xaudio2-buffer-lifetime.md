> **Registro histórico:** esta auditoria documenta os adaptadores WASAPI/XAudio2 do baseline anterior. Eles foram substituídos na candidata CPAL 0.18.2 da [PR #770](https://github.com/leonardo201800478/sega-emulator/pull/770). Preserve este conteúdo como histórico técnico; não use os backends antigos como especificação operacional. Consulte [saída física CPAL](cpal-output-backend.md).

# OUT-D — ownership e conclusão de buffers XAudio2

**Status:** contrato de lifetime e migração PCM FIFO integrados na `main`.
**PRs:** #652 (contrato), #653 (callback) e #654 (migração).
**Merge OUT-B:** `0a7a0579d01277fcccc21f6ad45190891604919d`.
**HEAD validado OUT-B:** `3f2d4ebf3e4514ae2f8497bcae6f4a8521313cd9`.
**CI:** #5678, SUCCESS no HEAD exato.
**Limite:** reprodução física após a migração OUT-B permanece NOT RUN.

## 1. Contrato Microsoft

1. A memória da estrutura `XAUDIO2_BUFFER` pode ser liberada após `SubmitSourceBuffer` retornar.
2. A memória apontada por `pAudioData` deve permanecer válida até `OnBufferEnd` ou até a voz ser destruída.
3. Callbacks XAudio2 executam na thread de processamento de áudio e precisam retornar rapidamente.
4. `FlushSourceBuffers` gera `OnBufferEnd` para buffers removidos, mas não garante ordem FIFO entre esses callbacks; se a voz estiver iniciada, o buffer atualmente em execução não é removido.
5. `DestroyVoice` aguarda a thread de processamento ficar ociosa; após seu retorno, a voz não executa callbacks nem lê dados de áudio.

Fontes:
- [XAUDIO2_BUFFER](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/ns-xaudio2-xaudio2_buffer).
- [IXAudio2VoiceCallback::OnBufferEnd](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voicecallback-onbufferend).
- [XAudio2 Callbacks](https://learn.microsoft.com/en-us/windows/desktop/xaudio2/xaudio2-callbacks).
- [IXAudio2SourceVoice::FlushSourceBuffers](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voice-flushsourcebuffers).
- [IXAudio2Voice::DestroyVoice](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voice-destroyvoice).

## 2. Implementação integrada

- O callback incrementa contador atômico; `OnVoiceError` sinaliza erro atomicamente.
- O worker é o único proprietário da coleção de buffers submetidos.
- Callback, estado COM e buffers permanecem vivos até `DestroyVoice`.
- O caminho de shutdown não tenta reclaim individual baseado na ordem dos callbacks de Flush.
- O adaptador COM foi criado com `IXAudio2VoiceCallback::new` do crate `windows = 0.62.2`.

## 3. Staging da FIFO PCM

1. Worker consome frames de `PcmFrameConsumer`.
2. Cada lote é copiado para uma alocação própria de `Vec<StereoFrame>` em pool fixo.
3. `pAudioData` aponta para staging estável, nunca para memória reutilizável da FIFO.
4. Buffers permanecem na coleção submitted até confirmação de conclusão no playback FIFO normal.
5. O worker devolve buffers concluídos ao pool; callback não manipula coleções.
6. Falha de `SubmitSourceBuffer` desfaz apenas o staging recém-inserido.
7. O produtor converte PCM16 mono em lotes de até 256 frames, sem vetor temporário proporcional ao tamanho do bloco.
8. Drop-newest mantém contagem exata de frames descartados.

## 4. Shutdown e falhas de inicialização

- Worker interrompe novas submissões.
- Executa `Stop` e `FlushSourceBuffers` conforme o shutdown existente.
- Não faz reclaim individual baseado na ordem de callbacks de Flush.
- Chama `DestroyVoice` antes de liberar staging ou callback.
- Após o retorno de `DestroyVoice`, libera coleção de staging e adaptador COM.
- Se a source voice foi criada e a inicialização subsequente falha, a voz é destruída antes do callback sair de escopo.

## 5. Validação OUT-B — CI #5678

HEAD: `3f2d4ebf3e4514ae2f8497bcae6f4a8521313cd9`.

- Windows audio-output fmt/Clippy/testes: PASS (23 testes).
- Linux audio-output fmt/Clippy/testes: PASS (23 testes).
- Workspace root fmt, Clippy `-D warnings`, `cargo test --workspace --locked`: PASS.
- Windows SDL3, D3D12 compile e PowerShell: PASS.
- Documentação/higiene e quality gate: PASS.
- Validação física de reprodução após migração: NOT RUN.

## 6. Limite para OUT-E

O contrato acima cobre o lifetime no playback normal e no shutdown. Não autoriza reclaim individual após Flush, nem define por si só a política de Pause/Stop/Resume. OUT-E precisa introduzir um protocolo de controle/epoch e uma barreira de conclusão própria, preservando todos os buffers até que a API confirme a conclusão segura ou até DestroyVoice.

## 7. Versões

- `windows = 0.62.2`.
- `wasapi = 0.26.0` no manifesto; 0.27.0 está publicado, mas não foi adotado nesta etapa.
- `rtrb = 0.3`, lockfile 0.3.6; nenhuma atualização foi feita nesta etapa.
