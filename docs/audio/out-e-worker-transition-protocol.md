# OUT-E — protocolo de transição de estado do áudio nativo

**Status:** implementação inicial em revisão na PR #656; validação CI do HEAD final pendente.
**Base:** `main` após OUT-B, merge commit `0a7a0579d01277fcccc21f6ad45190891604919d`.
**Escopo:** integrar Pause/Stop/Reset/LoadRom/StepFrame com mute, flush, epoch e prebuffer sem alterar a linha temporal da máquina.

## 1. Auditoria do estado atual

Arquivos auditados:
- `rust/runtime/sms-runtime/src/worker.rs`
- `rust/runtime/sms-runtime/src/audio.rs`
- `native/audio_runtime/src/session.rs`
- `native/audio_output/src/lib.rs`
- `native/audio_output/src/pcm_fifo.rs`
- `native/sega_app/src/app.rs`

Achados observados no HEAD da main:
1. `Pause` e `Stop` apenas definem `running = false`; não comandam mute nem flush físico.
2. `StepFrame` chama `execute_frame`, que drena as amostras da máquina e encaminha o lote à sessão de áudio, podendo alimentar a saída física durante depuração.
3. `NativeAudioSession` oferece `process_samples`, `set_volume`, `metrics` e `close`; não possui operação explícita de descarte/flush, mute ou reset de epoch.
4. `PcmSink` oferece apenas `write_pcm`, `metrics` e `close`.
5. WASAPI e XAudio2 possuem threads consumidoras independentes; a FIFO é SPSC e o worker SMS é o produtor. O worker consumidor deve continuar sendo o único proprietário do endpoint de leitura.
6. WASAPI já possui no wrapper `wasapi 0.26.0` os métodos `stop_stream` e `reset_stream`; Reset exige stream parado e descarta dados pendentes do stream.
7. XAudio2 possui callback `OnBufferEnd`; `FlushSourceBuffers` pode emitir callbacks fora da ordem de submissão e não remove o buffer que já está tocando enquanto a voz está iniciada.

Esta auditoria é estática. Ela não demonstra o instante exato em que o endpoint físico deixa de emitir som.

## 2. Objetivos semânticos

- **Pause:** congelar a máquina sem avançar Scheduler; invalidar áudio host pendente e silenciar a saída. Preservar estado de chips, mixer e resampler para continuidade após Resume.
- **Resume/Start:** iniciar nova epoch de apresentação de áudio; aguardar prebuffer mínimo antes de liberar som. O prebuffer não altera o clock da máquina.
- **Stop:** manter a semântica atual da máquina (parada), descartar PCM host pendente e manter saída silenciada.
- **Reset/LoadRom:** invalidar a epoch anterior antes de substituir a máquina; não permitir PCM antigo atravessar a fronteira da nova máquina. Política de reset do estado FIR/resampler deve ser explícita e testada.
- **StepFrame:** avançar deterministamente CPU/VDP/PSG e estado DSP enquanto pausado, mas não publicar áudio no endpoint físico. As amostras do frame são consumidas/descartadas na fronteira host sem retroalimentar a máquina.
- **Close/Shutdown:** silenciar, interromper produção, drenar/invalidar filas e destruir endpoints seguindo ownership seguro.

## 3. Protocolo de concorrência obrigatório

1. A thread do worker SMS continua sendo o único produtor da FIFO; a thread backend continua sendo o único consumidor.
2. Nenhum comando de Pause/Stop/Reset pode acessar diretamente o endpoint consumidor nem manipular o cursor interno da FIFO pelo lado produtor.
3. Uma operação de flush é um comando coarse-grained enviado ao worker do backend, com confirmação síncrona de que a epoch anterior foi invalidada.
4. A fronteira entre epochs é estabelecida por uma barreira síncrona serializada no worker SMS: enquanto `suspend_and_flush` aguarda a confirmação, o único produtor não publica novos frames. Um contador numérico não é necessário neste modelo de produtor único e transições não reentrantes; se o controle se tornar assíncrono/multifonte, deverá ser introduzido um identificador monotônico com rejeição explícita de comandos obsoletos.
5. O worker SMS não pode publicar novos frames entre emitir flush e receber a confirmação do backend.
6. O backend deve descartar os frames ainda na FIFO e limpar o áudio já submetido ao endpoint segundo o contrato específico da API.
7. Nenhuma API, lock, espera ou alocação nova pode ser introduzida no caminho de instruções ou no callback XAudio2.
8. Erro ou timeout na barreira de flush deve ser propagado ao worker/UI; não pode ser tratado como sucesso nem permitir reuso de uma epoch incerta.

## 4. Requisitos por backend

### WASAPI Shared/Event

- Executar Stop e Reset apenas na thread proprietária do `IAudioClient`.
- `IAudioClient::Reset` só é válido depois de Stop e descarta dados pendentes, zerando a posição do clock do stream.
- Drenar a FIFO na thread consumidora antes de confirmar a epoch nova.
- Retomar com prebuffer antes de Start, respeitando a sequência de preenchimento do buffer de renderização.
- Acordar o worker consumidor para comandos de controle sem depender exclusivamente de um evento de áudio que pode não ocorrer enquanto o stream estiver parado.

Referências: [IAudioClient::Stop](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-stop), [IAudioClient::Reset](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-reset), [Rendering a Stream](https://learn.microsoft.com/en-us/windows/win32/coreaudio/rendering-a-stream).

### XAudio2

- O controle de voz ocorre exclusivamente na thread do worker XAudio2.
- Para flush, impedir novas submissões, aplicar `SetVolume(0)` e chamar `FlushSourceBuffers` sem Stop. O buffer atualmente em reprodução permanece ativo, mas fica silenciado até concluir e entregar `OnBufferEnd`; parar a voz suprimiria callbacks e impediria a barreira de lifetime.
- `FlushSourceBuffers` pode gerar callbacks fora de ordem; não associar callbacks a reclaim FIFO individual após flush.
- Buffers submetidos permanecem vivos até que cada `OnBufferEnd` esperado seja observado ou até `DestroyVoice`; callback e estado COM permanecem vivos durante a barreira.
- A voz permanece iniciada e mutada após a barreira; depois do prebuffer, restaurar `SetVolume(1)` antes de submeter os frames da nova epoch. Na criação inicial, a voz só é iniciada após o primeiro prebuffer.
- O estado de mute/volume precisa ser restaurado antes de iniciar o playback após o prebuffer.

Referências: [FlushSourceBuffers](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2sourcevoice-flushsourcebuffers), [OnBufferEnd](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voicecallback-onbufferend), [windows 0.62.2 IXAudio2SourceVoice](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/XAudio2/struct.IXAudio2SourceVoice.html).

## 5. Prebuffer e política de StepFrame

- O limite de prebuffer deve ser expresso em frames, limitado pela capacidade configurada da FIFO e derivado de períodos reais do endpoint; não fixar milissegundos arbitrários sem medir a configuração.
- O consumidor deve permanecer em silêncio até alcançar o limite mínimo de frames.
- O primeiro lote liberado após Resume deve pertencer integralmente à epoch atual.
- StepFrame mantém execução determinística do hardware e do DSP, mas o PCM produzido durante o passo é descartado antes do sink.
- Testar Start inicial, Pause/Resume repetidos, Stop/Start, Reset durante pausa, troca de ROM e StepFrame repetido.

## 6. Implementação atual — PR #656

A implementação introduzida na PR #656 cobre os seguintes pontos:

- `PcmSink` exige `suspend_and_flush` e `resume`; não há implementação padrão que retorne sucesso sem realizar controle.
- `NativeAudioSession` expõe flush, resume e reset do estado FIR/resampler.
- O worker SMS serializa Start/Pause/Stop/Reset/LoadRom/ConfigureAudio e aguarda a confirmação do backend antes de concluir as transições relevantes.
- WASAPI executa Stop → Reset → descarte pelo consumidor. A espera do evento tem timeout de 10 ms para observar comandos de controle; o restart espera o prebuffer em frames, limitado à capacidade efetiva da FIFO.
- XAudio2 executa Stop → FlushSourceBuffers e aguarda o número agregado de callbacks esperado e `BuffersQueued == 0`. Não libera staging individualmente por callback durante Flush.
- A barreira de flush tem timeout de 5 s. Falha ou timeout é propagado como erro e não é convertido em sucesso.
- StepFrame processa mixer/resampler, mas não envia PCM ao sink. A história FIR dos frames depurados é descartada antes do primeiro bloco audível após Resume.
- Pause/Resume sem StepFrame preserva a história FIR; Reset/LoadRom a reinicializam.

### Evidência automatizada adicionada

- FIFO: descarte pelo consumidor e reutilização da fila com novos frames.
- DSP: StepFrame sem publicação; ausência de vazamento de FIR após Resume; Pause/Resume preserva FIR; Reset reinicializa FIR.
- Worker: StepFrame avança a máquina sem publicar PCM.
- CI Windows e Linux precisa passar no mesmo HEAD da PR #656 antes de marcar a etapa como validada.
- Reprodução física, hotplug e escuta real permanecem OUT-G / NOT RUN.

## 7. Critérios de aceitação para implementação OUT-E

- [x] `AudioSession`/ `PcmSink` possuem contrato explícito de suspend/resume, flush e descarte de PCM.
- [x] Pause só confirma após backend silenciado e epoch anterior invalidada.
- [x] Stop não deixa PCM residual reproduzindo após confirmação.
- [x] Resume só libera áudio após prebuffer mínimo da epoch atual.
- [x] Reset/LoadRom não reproduzem frames da máquina anterior.
- [x] StepFrame avança estado DSP deterministicamente sem emitir som no endpoint.
- [x] WASAPI usa Stop → Reset → drain/rearm → prebuffer → Start, com chamadas na thread proprietária.
- [x] XAudio2 preserva staging até conclusão segura de todos os callbacks ou DestroyVoice; sem reclaim individual por callback fora de ordem.
- [x] Operações de controle são coarse-grained; nenhuma lógica entra na GUI além dos comandos já existentes.
- [ ] Cobertura headless completa de transições concorrentes, timeout/falha de flush e equivalência contínua/em blocos (há testes direcionados iniciais, ainda não a suíte integral).
- [ ] CI Windows e Linux no HEAD exato; fmt, Clippy `-D warnings`, testes standalone e workspace raiz PASS.
- [ ] Validação física posterior classificada separadamente (OUT-G).

## 8. Riscos e decisões ainda abertas

1. O wrapper WASAPI atual aguarda eventos de áudio; quando o stream está parado, o protocolo de controle precisa de mecanismo independente de wake-up. Reduzir timeout por polling só pode ser adotado com justificativa e medição.
2. XAudio2 deve silenciar a source voice antes do Flush e usar uma barreira agregada de conclusão para a fila inteira; não parar a voz, pois Stop suprime callbacks. O contador normal de callbacks em ordem FIFO não é suficiente para reclaim incremental durante flush.
3. A semântica de Reset do resampler deve distinguir Pause (preservar estado DSP) de Reset/LoadRom (nova fronteira de conteúdo).
4. A duração do prebuffer deve ser derivada de buffer/period do endpoint e testada contra latência/underruns, não escolhida por conveniência.
5. Mute físico imediato e descarte de buffers são propriedades distintas; ambas precisam ser comprovadas.
6. O protocolo deve evitar comandos concorrentes ou reentrantes e tratar timeout como falha explícita.

## 9. Referências de versão

- Manifesto atual: `wasapi = 0.26.0`; Docs.rs já lista 0.27.0 (2026-11-06). Não atualizar nesta especificação sem auditar changelog, MSRV e API usada.
- Manifesto atual: `windows = 0.62.2`; APIs de XAudio2 consultadas na documentação correspondente.
- `rtrb = 0.3`, lockfile 0.3.6; manter sem atualização até auditoria semântica independente.
