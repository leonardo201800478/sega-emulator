# OUT-B — contrato de integração dos backends nativos com PCM frame FIFO

**Status:** especificação de implementação; nenhum backend foi migrado nesta etapa.  
**Base auditada:** `531764ee836b8c412b550685d923a912961d7015`.  
**Escopo:** definir o limite de transporte entre `sega-audio-runtime`, a FIFO PCM estéreo e os consumidores WASAPI/XAudio2.

## 1. Estado observado na base

- `pcm_fifo.rs` fornece uma SPSC limitada de `StereoFrame = [i16; 2]`, com overflow drop-newest, preenchimento de underflow com silêncio e métricas em frames.
- `NativeWasapiOutput` ainda transporta `Vec<u8>` por bloco. O worker mantém bytes pendentes em `VecDeque<u8>` e atende o tamanho variável solicitado pelo endpoint WASAPI.
- `NativeXAudio2Output` ainda transporta `Vec<u8>`. O worker move blocos para `VecDeque<Vec<u8>>` e os mantém vivos enquanto estiverem submetidos à voz.
- `NativeAudioSession` chama `PcmSink::write_pcm(Vec<u8>)`; o sink recebe PCM16 mono e cada backend faz a duplicação/pan para estéreo na fronteira de saída.
- Nenhum backend consome `PcmFrameConsumer` ainda.

## 2. Invariantes obrigatórios

1. A FIFO comum é SPSC: exatamente um produtor e um consumidor, sem compartilhamento de endpoint entre threads.
2. A unidade de transporte é sempre um frame estéreo PCM16 intercalado [L, R]. Tamanhos de bytes não podem ser confundidos com quantidade de frames.
3. O produtor converte o PCM16 mono recebido do runtime em frames estéreo antes da publicação. Pan/balance continua na fronteira do backend e não altera o PSG nem o mixer do hardware.
4. O consumidor não aloca memória por callback/evento. Buffers de scratch e staging são dimensionados e preparados antes do loop de áudio.
5. Overflow mantém a política drop-newest e incrementa contadores por frame; underflow preenche apenas a região ausente com silêncio e contabiliza frames/eventos.
6. Nenhum caminho de áudio pode bloquear esperando espaço ou dados na FIFO.
7. O encerramento de endpoint é tratado como transição terminal; não se tenta continuar publicando após consumidor abandonado.
8. Não alterar clocks, pacing, Scheduler, PSG, DSP/resampler, política Pause/Stop/Resume/StepFrame ou GUI nesta etapa.
9. O transporte de host não define o relógio do emulador e não pode introduzir callbacks por instrução.

## 3. WASAPI — estratégia de adaptação

- O worker WASAPI permanece o único consumidor da FIFO.
- A quantidade solicitada pelo endpoint é expressa em frames de áudio; a conversão para bytes deve usar `block_align` validado para estéreo PCM16 (4 bytes/frame).
- Ler os frames disponíveis para um scratch buffer previamente alocado. Preservar a semântica de leitura parcial e o silêncio apenas para o sufixo ausente.
- O buffer de staging usado pelo método de escrita do render client deve ter capacidade suficiente para o maior bloco suportado pelo endpoint e ser reutilizado entre eventos.
- Se a API existente exigir `VecDeque<u8>`, essa estrutura deve ser reservada uma vez e reutilizada; não converter por meio de novas alocações em cada evento.
- Tratar desconexão do produtor somente depois de drenar os frames já publicados. Não reiniciar a fila implicitamente durante a recuperação do dispositivo.

## 4. XAudio2 — estratégia de adaptação

- O worker XAudio2 permanece o único consumidor da FIFO.
- Converter lotes de frames em buffers estéreo PCM16 alinhados (4 bytes/frame).
- Cada buffer submetido precisa ter armazenamento estável até a conclusão do playback. A documentação do XAudio2 determina que os dados apontados por `XAUDIO2_BUFFER::pAudioData` não podem ser liberados até `IXAudio2VoiceCallback::OnBufferEnd`, ou até a voz ser parada/destruída.
- Portanto, nunca submeter ponteiro para scratch/FIFO que será reutilizado na próxima leitura.
- Na primeira migração, manter ownership explícito dos buffers submetidos em uma coleção de staging do worker e só liberar após confirmação de conclusão por mecanismo comprovado. A estratégia de callback e o gerenciamento de lifetime detalhados pertencem ao OUT-D.
- Não introduzir callback que faça alocação, bloqueio, logging pesado ou operações de host. O callback, quando implementado, apenas sinaliza conclusão para o worker por mecanismo bounded e realtime-safe.
- A fila de entrada e a coleção de buffers submetidos têm capacidades independentes; definir limites e comportamento de saturação explicitamente.

## 5. Fronteira runtime / sink

A assinatura atual `PcmSink::write_pcm(Vec<u8>)` pode continuar no primeiro incremento OUT-B. A conversão mono → estéreo ocorre no produtor/backend antes de `push_frames`. Não mudar simultaneamente a API do runtime e os dois backends sem necessidade demonstrada.

A alocação já existente no lado produtor é aceitável nesta fase, desde que:
- seja contabilizada e não ocorra no consumidor;
- não se faça uma alocação por frame;
- o lote seja convertido de forma determinística;
- overflow parcial retorne/registre o número exato de frames aceitos e descartados.

Uma futura API de escrita por fatia poderá remover a transferência de `Vec<u8>`, mas não é pré-requisito para migrar a fila e deve ser avaliada separadamente.

## 6. Critérios de aceitação OUT-B

- [ ] WASAPI e XAudio2 usam a mesma FIFO frame-based como transporte entre producer e worker.
- [ ] Testes unitários provam conversão mono PCM16 → estéreo, incluindo `i16::MIN`, `i16::MAX`, zero e balance -100/0/+100.
- [ ] Testes provam alinhamento frame/byte e rejeição de lote PCM16 mono com número ímpar de bytes.
- [ ] Testes provam overflow parcial drop-newest e contagem exata.
- [ ] Testes provam underflow com silêncio apenas no sufixo ausente.
- [ ] Testes provam produtor/consumidor abandonado e drenagem dos frames já publicados.
- [ ] O consumidor não aloca no loop de leitura; staging WASAPI é reutilizado.
- [ ] Buffers XAudio2 permanecem válidos até conclusão ou destruição da voz; nenhum ponteiro aponta para memória reutilizada prematuramente.
- [ ] Não há alteração em Scheduler, timing, chips, DSP, worker SMS ou política de controle de sessão.
- [ ] CI Windows executa fmt, Clippy com `-D warnings` e testes no HEAD exato da PR.
- [ ] A regressão completa do workspace é executada quando os filtros de impacto a exigirem; qualquer job não aplicável permanece NOT RUN/SKIPPED, nunca PASS.

## 7. Decomposição de implementação

1. Extrair helpers puros de conversão e validação de frames, com testes direcionados.
2. Migrar o producer e o worker WASAPI, preservando a reserva de staging e a recuperação atual do endpoint.
3. Migrar XAudio2 separadamente, primeiro fechando ownership e conclusão dos buffers; não reutilizar o modelo de polling como prova de lifetime sem validar a relação entre `BuffersQueued` e os buffers próprios.
4. Validar cada backend em Windows CI antes de iniciar OUT-C (WASAPI Event-Driven) e OUT-D (callback XAudio2).
5. Manter OUT-E (mute/flush/epoch e controles de sessão) fora deste incremento.

## 8. Referências primárias

- Contrato FIFO implementado: [pcm-frame-queue-contract.md](pcm-frame-queue-contract.md).
- API `rtrb`: [Producer](https://docs.rs/rtrb/latest/rtrb/struct.Producer.html), [Consumer](https://docs.rs/rtrb/latest/rtrb/struct.Consumer.html).
- Microsoft, [XAUDIO2_BUFFER](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/ns-xaudio2-xaudio2_buffer).
- Microsoft, [IXAudio2SourceVoice::SubmitSourceBuffer](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2sourcevoice-submitsourcebuffer).

## 9. Limites desta especificação

Este documento não declara OUT-B implementado, não altera comportamento executável e não constitui validação física de áudio. Cada critério acima deve ser demonstrado por código, testes e CI antes de marcar OUT-B como concluído.
