# Aceitação do backend de áudio nativo SMS — 2026-11-06

**Estado:** instrumentação e testes físicos preparados; execução em endpoint Windows real ainda **NOT RUN**.
**Base de implementação:** main `b82f748b83749eac2173ff86e535cc2a9e2ca862`, após PR #598.
**Branch desta implementação:** `test/sms-native-audio-backend-acceptance-20261106`.

## Objetivo

Separar a evidência de PCM entregue à sessão de DSP da evidência de aceitação pelo endpoint de áudio nativo. Os testes iniciam uma instância real de `SmsTemporalWorker`, cujo loop executa `execute_frame`, usando a ROM diagnóstica sintética já usada pela baseline A/V; não contêm ROM comercial.

## Mudanças desta fatia

- `PcmSink` e `NativeAudioSession` expõem contagem de submissões de buffers PCM ao endpoint e o último erro assíncrono do backend.
- WASAPI incrementa a contagem somente após uma escrita de render buffer bem-sucedida que recebeu PCM da fila.
- XAudio2 incrementa a contagem somente após `SubmitSourceBuffer` retornar sucesso.
- Falhas assíncronas do worker WASAPI/XAudio2 ficam disponíveis à sessão e à telemetria do worker SMS. Erros WASAPI permanecem registrados durante a sessão, inclusive se o worker tentar reconectar. XAudio2 registra `IXAudio2EngineCallback::OnCriticalError` e `IXAudio2VoiceCallback::OnVoiceError`; ambos os callbacks apenas gravam o HRESULT em atômicos, e o worker realiza o tratamento fora da thread de áudio.
- Dois testes Windows `#[ignore]` exercitam o caminho de frame SMS → NativeAudioSession → backend real, um para WASAPI e outro para XAudio2.
- Nenhum clock, Scheduler, PSG, formato de estado, CPU ou caminho Python de PCM foi alterado.

## Execução física no Windows

Executar os testes separadamente, com um endpoint de saída padrão habilitado:

```powershell
cargo test --manifest-path native/z80_core/Cargo.toml worker_frames_reach_real_wasapi_endpoint -- --ignored --nocapture
cargo test --manifest-path native/z80_core/Cargo.toml worker_frames_reach_real_xaudio2_endpoint -- --ignored --nocapture
```

Cada teste executa 120 frames da máquina SMS, enviando áudio pelo mesmo executor de frame usado pelo worker. A saída registra:

- `endpoint_submissions`: buffers com PCM aceitos pela API de saída;
- `underruns`: eventos de falta de dados observados pelo consumidor;
- `dropped_blocks`: blocos descartados na fila limitada;
- `output_error`: último erro assíncrono informado pelo worker do backend.

Critérios automatizados do teste:
- pelo menos uma submissão de PCM ao endpoint;
- zero blocos descartados;
- nenhum erro assíncrono persistente na sessão.

Underruns são registrados, mas não há limite arbitrário embutido no teste: devem ser avaliados em conjunto com a duração, a carga e a audição. O teste não deve ser executado simultaneamente para os dois backends.

## Limites da comprovação

Uma escrita WASAPI bem-sucedida ou um `SubmitSourceBuffer` bem-sucedido comprova aceitação pela API/engine, não comprova que o DAC ou alto-falante produziu som audível. O teste também não é uma validação de gameplay.

- **A/V-04a:** worker → NativeAudioSession → sink determinístico em memória — PASS pela PR #597.
- **A/V-04b:** worker → NativeAudioSession → WASAPI/XAudio2 real — preparado; permanece **NOT RUN** até executar os testes ignorados em Windows físico.
- **A/V-05:** som efetivamente audível e vídeo apresentado em GPU física — **NOT RUN**; exige observação manual no dispositivo.
- **A/V-06:** gameplay com ROM legal/homebrew — **NOT RUN**.

Após cada teste, ouvir a saída durante toda a execução e registrar dispositivo, driver, formato, backend, underruns, blocos descartados, erros e resultado auditivo. Ausência de erro no teste não substitui a confirmação auditiva.

## Revisão de dependências — 2026-11-06

Versões declaradas/lockadas no projeto:
- `wasapi = 0.26`; documentação corrente consultada: 0.26.0.
- `windows = 0.62.2`; documentação corrente consultada: 0.62.2.
- `rtrb = 0.3`; lockfiles do crate de saída e do Z80 fixam 0.3.6. A documentação corrente também lista 0.4.1; a atualização não foi incluída porque esta tarefa não requer mudança da estrutura SPSC e uma troca de versão exigiria validação diferencial própria.

A documentação de `wasapi` confirma o modo compartilhado/event-driven e informa que `InitializeSharedAudioStream` não oferece autoconversão de formato. O runtime tenta IAudioClient3 e mantém fallback para `initialize_client` com `EventsShared { autoconvert: true }`. Essa compatibilidade deve continuar coberta pelos testes físicos nos endpoints disponíveis.

Referências:
- Microsoft — [IAudioClient3](https://learn.microsoft.com/windows/win32/api/audioclient/nn-audioclient-iaudioclient3)
- Microsoft — [WASAPI](https://learn.microsoft.com/windows/win32/coreaudio/wasapi)
- Rust `wasapi` 0.26 — [docs.rs](https://docs.rs/wasapi/0.26.0/wasapi/)
- Rust `windows` 0.62.2 — [docs.rs](https://docs.rs/windows/0.62.2/windows/)
- Rust `rtrb` — [docs.rs](https://docs.rs/rtrb/)

## Gate

Até a execução física, manter A/V-04b, A/V-05 e A/V-06 como **NOT RUN**. CI de compilação Windows e testes headless não são substitutos de endpoint real ou de audição.
