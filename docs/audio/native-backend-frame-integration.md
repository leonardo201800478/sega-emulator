# OUT-B — integração dos backends nativos com PCM frame FIFO

**Status:** Migração WASAPI/XAudio2 implementada e validada por CI #5676 no HEAD `5fe508f9acba9dd5b2b7b58835d93bef7481743c`; validação física de reprodução em dispositivo Windows permanece pendente.
**Base:** `604346b9abd8d7b9505ced8ae9007927f9e32535`.
**Escopo desta etapa:** completar a migração do transporte XAudio2 para a FIFO PCM estéreo compartilhada, sem alterar o runtime de áudio, DSP, Scheduler, PSG, VDP ou GUI.

## 1. Estado reconciliado com a main

- OUT-A fornece `PcmFrameProducer`, `PcmFrameConsumer` e `StereoFrame = [i16; 2]` em FIFO SPSC limitada.
- WASAPI já consome a FIFO frame-based; a migração foi integrada nas PRs anteriores.
- XAudio2 passou a usar `OnBufferEnd` como confirmação de conclusão na PR #653, mas ainda recebia `Vec<u8>` por uma segunda FIFO.
- `NativeAudioSession` continua publicando PCM16 mono via `PcmSink::write_pcm(Vec<u8>)`.
- A conversão mono → estéreo permanece na fronteira de saída. PSG, mixer e DSP não mudam.

## 2. Invariantes da FIFO compartilhada

1. Exatamente um produtor e um consumidor por endpoint SPSC.
2. Unidade de transporte: frame estéreo PCM16 intercalado [L, R]; um frame ocupa 4 bytes na representação XAudio2.
3. Overflow é drop-newest, contabilizado por frame; a FIFO nunca sobrescreve frames ainda não consumidos.
4. A leitura não espera e não aloca. Underflow da API de leitura preenche com silêncio somente a região ausente.
5. A fila não possui clock e não controla o MachineScheduler.
6. O produtor converte o PCM16 mono recebido do runtime para frames estéreo antes de publicar.
7. Nenhuma memória interna reutilizável da FIFO pode ser entregue como pAudioData ao XAudio2.

## 3. XAudio2 — modelo de staging e ownership

- A capacidade da FIFO é `buffer_size × queue_blocks` frames.
- O worker é o único consumidor da FIFO e mantém um pool fixo de buffers de staging próprios, cada um com capacidade de `buffer_size` frames.
- Um lote de frames é copiado para uma alocação de staging dedicada antes de SubmitSourceBuffer.
- O ponteiro submetido é obtido do armazenamento heap do `Vec<StereoFrame>`, nunca do scratch/FIFO reutilizável.
- Mover o handle `Vec` entre coleções `VecDeque` não move sua alocação de dados.
- Os buffers submetidos permanecem na coleção do worker até que o contador de callbacks confirme OnBufferEnd na reprodução FIFO normal.
- OnBufferEnd realiza apenas um incremento atômico bounded. OnVoiceError apenas sinaliza um estado atômico. Nenhum callback aloca, bloqueia, registra logs, libera memória ou chama APIs XAudio2.
- O worker é o único responsável por retirar buffers concluídos e devolvê-los ao pool.
- Se SubmitSourceBuffer falhar, o worker desfaz a inserção do buffer correspondente; não espera callback para submissão rejeitada.
- O contador de conclusão usa aritmética wrapping e o número de buffers pendentes é limitado pela capacidade do pool.
- A contagem sequencial depende da ordem FIFO de callbacks na reprodução normal. Flush não ocorre no caminho normal.

## 4. Encerramento, Stop, Flush e DestroyVoice

- O shutdown interrompe novas submissões.
- Stop e FlushSourceBuffers mantêm o comportamento de encerramento já existente.
- Não há reclaim individual baseado nos callbacks provocados por Flush, pois a Microsoft não garante ordem FIFO desses callbacks.
- Todos os buffers ainda submetidos permanecem vivos até DestroyVoice retornar.
- A documentação Microsoft estabelece que DestroyVoice aguarda a thread de processamento ficar ociosa e garante que a voz não fará mais callbacks nem lerá dados de áudio após seu retorno.
- O callback COM e seu estado emprestado permanecem vivos até depois de DestroyVoice, inclusive nos caminhos de erro após criação da source voice.
- Somente depois da destruição da voz o worker libera a coleção de buffers e o adaptador do callback.

## 5. Critérios de aceitação

- [x] WASAPI usa a FIFO frame-based.
- [x] XAudio2 usa PcmFrameProducer/PcmFrameConsumer como transporte de entrada.
- [x] Conversão mono PCM16 → StereoFrame ocorre em lotes fixos de 256 frames, sem vetor temporário proporcional ao tamanho do bloco recebido.
- [x] WASAPI e XAudio2 contabilizam exatamente os frames descartados pela política drop-newest.
- [x] O caminho host não-Windows do crate standalone é compilado/testado separadamente para proteger os stubs condicionais.
- [x] XAudio2 prepara armazenamento estável e próprio antes de SubmitSourceBuffer.
- [x] Callback não manipula buffers nem executa trabalho pesado.
- [x] Falha de SubmitSourceBuffer remove o staging pendente sem aguardar callback.
- [x] Reclaim de playback normal ocorre apenas após OnBufferEnd.
- [x] Flush não faz reclaim individual; buffers são retidos até DestroyVoice.
- [x] Estado de callback permanece vivo durante a vida da source voice.
- [x] CI Windows no HEAD validado: fmt, Clippy -D warnings e testes focados PASS.
- [x] Regressão global do workspace: cargo fmt, Clippy e cargo test PASS.
- [ ] Validação física de áudio em dispositivo Windows real (NOT RUN).

## 6. Testes adicionados nesta implementação

- Estabilidade do endereço de staging após submissões subsequentes.
- Correspondência de frames e bytes estéreo PCM16.
- Conclusões em sequência e reclaim somente após contador de callback.
- Rollback de staging após falha de submissão.
- Wrap do contador u64.
- Sinalização atômica concorrente entre callback e worker.
- Conversão PCM em lotes limitados, incluindo fronteira entre lotes e blocos maiores que a capacidade da FIFO.
- FIFO existente: overflow drop-newest, underflow, drenagem e desconexão do produtor.

Os testes unitários modelam o bookkeeping e as invariantes de memória. Eles não substituem a compilação/execução Windows real nem a validação física do dispositivo.

## 7. Validação da PR #654 — CI #5676

HEAD validado: `5fe508f9acba9dd5b2b7b58835d93bef7481743c`.

- Windows audio-output: fmt PASS; Clippy `-D warnings` PASS; testes PASS.
- Linux audio-output: fmt PASS; Clippy `-D warnings` PASS; testes PASS.
- Workspace root: `cargo fmt --all -- --check` PASS.
- Workspace root: `cargo clippy --workspace --all-targets --locked -- -D warnings` PASS.
- Workspace root: `cargo test --workspace --locked` PASS.
- Documentação `git diff --check`: PASS.
- Windows SDL3, D3D12 e PowerShell: PASS.
- Quality gate do workflow: PASS.
- Validação física de áudio em dispositivo Windows real: NOT RUN.

CI #5676 terminou com conclusão `success` para o HEAD acima. A execução comprova compilação e testes nos hosts Windows e Linux; não comprova reprodução audível em hardware físico.

## 8. Limites explícitos

- Não altera `PcmSink`, `NativeAudioSession`, DSP/resampler, Scheduler, PSG, VDP, controles de sessão ou GUI.
- Não implementa OUT-E (mute/flush/epoch/prebuffer).
- Não declara áudio fisicamente validado.
- Não declara regressão global concluída sem execução dos jobs correspondentes.
- Não altera versões de `windows = 0.62.2` ou `rtrb = 0.3`.

## 9. Referências técnicas

- Microsoft, [XAUDIO2_BUFFER](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/ns-xaudio2-xaudio2_buffer).
- Microsoft, [IXAudio2VoiceCallback::OnBufferEnd](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voicecallback-onbufferend).
- Microsoft, [IXAudio2Voice::DestroyVoice](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voice-destroyvoice).
- Microsoft, [IXAudio2SourceVoice::FlushSourceBuffers](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2sourcevoice-flushsourcebuffers).
- Microsoft, [XAudio2 Callbacks](https://learn.microsoft.com/en-us/windows/desktop/xaudio2/xaudio2-callbacks).
- windows 0.62.2, [IXAudio2VoiceCallback](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/XAudio2/struct.IXAudio2VoiceCallback.html) e [IXAudio2VoiceCallback_Impl](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/XAudio2/trait.IXAudio2VoiceCallback_Impl.html).
- rtrb 0.3 API utilizada pelo projeto: endpoints SPSC, slots e operações push/pop; a dependência não foi atualizada nesta mudança.
