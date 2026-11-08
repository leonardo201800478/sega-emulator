# OUT-D — contrato de ownership e conclusão de buffers XAudio2

**Status:** especificação técnica; nenhuma alteração executável nesta etapa.
**Base auditada:** `d8e1b657354be1096973c321ee6c44cb3b40dc29`.
**Backend:** `native/audio_output/src/lib.rs`, `NativeXAudio2Output` e `run_xaudio2_worker`.
**Objetivo:** tornar explícita a prova de lifetime de `XAUDIO2_BUFFER::pAudioData` antes da migração do transporte XAudio2 para a FIFO frame-based.

## 1. Estado atual observado

- O produtor publica blocos `Vec<u8>` em uma FIFO `rtrb`.
- O worker retira blocos e os mantém em `VecDeque<Vec<u8>>`.
- O endereço de `Vec<u8>` é passado como `pAudioData` em `SubmitSourceBuffer`.
- A estrutura `XAUDIO2_BUFFER` local é descartada após a chamada, o que é permitido pela API.
- O código atualmente remove buffers do `VecDeque` comparando seu comprimento com `XAUDIO2_VOICE_STATE::BuffersQueued`, consultado por polling de `GetState`.
- A criação da source voice passa `None::<&IXAudio2VoiceCallback>`; portanto, não há callback `OnBufferEnd` implementado.

## 2. Evidência documental primária

A Microsoft especifica que:
1. A memória da estrutura `XAUDIO2_BUFFER` pode ser liberada quando `SubmitSourceBuffer` retorna.
2. A memória apontada por `pAudioData` não pode ser liberada até o callback `IXAudio2VoiceCallback::OnBufferEnd` ou até a voz ser parada/destruída.
3. `OnBufferEnd` ocorre depois que o último byte do buffer foi consumido e antes do primeiro byte do buffer seguinte ser consumido.
4. Callbacks XAudio2 executam na thread de processamento de áudio e precisam retornar rapidamente. Não devem fazer I/O, chamadas caras/bloqueantes, sincronização com outras threads ou trabalho significativo.
5. `FlushSourceBuffers` gera `OnBufferEnd` para buffers removidos, mas esses callbacks não são necessariamente entregues na ordem em que os buffers foram submetidos. O buffer atualmente em reprodução pode continuar até terminar.

Fontes:
- Microsoft, [XAUDIO2_BUFFER](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/ns-xaudio2-xaudio2_buffer).
- Microsoft, [IXAudio2VoiceCallback::OnBufferEnd](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voicecallback-onbufferend).
- Microsoft, [XAudio2 Callbacks](https://learn.microsoft.com/en-us/windows/desktop/xaudio2/xaudio2-callbacks).
- Microsoft, [IXAudio2SourceVoice::GetState](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2sourcevoice-getstate).
- Microsoft, [IXAudio2SourceVoice::FlushSourceBuffers](https://learn.microsoft.com/en-us/windows/desktop/api/xaudio2/nf-xaudio2-ixaudio2sourcevoice-flushsourcebuffers).
- Windows crate 0.62.2, [IXAudio2VoiceCallback_Impl](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/XAudio2/trait.IXAudio2VoiceCallback_Impl.html).

## 3. Conclusão da auditoria do mecanismo atual

**Não aceitar `BuffersQueued` isoladamente como prova de que a memória do buffer pode ser liberada.**

A documentação define o contador como o número de buffers atualmente enfileirados, incluindo o que está em processamento. Ela não documenta que a observação de uma redução do contador pelo worker estabelece a sincronização necessária com o callback de conclusão nem que a memória já pode ser liberada naquele instante. O contrato de lifetime nomeia explicitamente `OnBufferEnd` ou parada/destruição da voz.

Assim, embora o `VecDeque` mantenha os dados enquanto o contador parece indicar buffers pendentes, o polling não é uma prova documental suficiente para liberar o bloco. A migração não deve perpetuar essa suposição.

## 4. Estratégia proposta para reprodução normal (sem flush)

A primeira implementação deve usar o callback como autoridade de conclusão e manter toda a gestão de memória no worker:

1. Cada bloco de áudio é colocado em armazenamento próprio e estável no `VecDeque` **antes** de chamar `SubmitSourceBuffer`. O ponteiro submetido aponta para o heap do bloco, não para memória da FIFO ou scratch reutilizável.
2. A source voice recebe uma implementação de `IXAudio2VoiceCallback` cuja referência permanece viva até depois de `DestroyVoice`.
3. `OnBufferEnd` executa somente uma operação atômica bounded (incrementar contador monotônico de conclusões). Não libera `Vec`, não remove elementos de coleção, não aloca, não bloqueia e não chama APIs XAudio2.
4. O worker compara o contador de conclusões com o número de conclusões já reaproveitadas e remove do início do `VecDeque` exatamente essa quantidade de buffers concluídos.
5. Esta estratégia pressupõe reprodução FIFO sequencial, sem `FlushSourceBuffers` durante o funcionamento normal. O contrato de `OnBufferEnd` garante a notificação do buffer antes do consumo do próximo; o worker libera os buffers em ordem de submissão.
6. Se `SubmitSourceBuffer` falhar, o worker desfaz a inserção correspondente, sem esperar callback para buffer que não foi aceito.
7. `OnVoiceError` apenas sinaliza erro atômico para o worker. A política de recuperação/destruição é executada fora da thread de áudio.

O callback não pode manter ponteiro para `Vec` nem executar destruição do armazenamento. A coleção continua pertencendo exclusivamente ao worker.

## 5. Encerramento

- O worker interrompe submissões novas.
- Para a voz e executa o protocolo de encerramento definido pela API.
- Pode liberar todos os blocos ainda mantidos somente depois que a voz tiver sido destruída com sucesso (ou outra condição de parada documentada que prove que ela não acessará mais os dados).
- O objeto de callback deve permanecer vivo até que a voz tenha sido destruída; somente então pode ser descartado.
- Erros no caminho de shutdown não podem levar a liberar buffers enquanto a voz ainda puder referenciá-los. A implementação deve tratar o resultado das operações de parada/destruição explicitamente, sem ignorar erros que invalidem a prova de lifetime.

## 6. Limite importante: Flush

A contagem atômica simples depende da ordem FIFO dos callbacks. A Microsoft documenta que callbacks provocados por `FlushSourceBuffers` podem ocorrer fora da ordem de submissão. Portanto:

- não introduzir flush no caminho normal desta primeira migração;
- se flush se tornar necessário, trocar a contagem sequencial por identificação individual via `XAUDIO2_BUFFER::pContext` e um canal de conclusão bounded/realtime-safe, ou adotar outro mecanismo que trate conclusões fora de ordem;
- não liberar armazenamento a partir de suposições sobre ordem de callback durante flush;
- políticas de flush/epoch permanecem OUT-E e não pertencem à primeira migração XAudio2.

## 7. Critérios de aceitação da futura implementação

- [ ] Source voice recebe callback válido e callback vive até depois da destruição da voz.
- [ ] O armazenamento de cada `pAudioData` permanece estável e pertencente ao worker até `OnBufferEnd` ou destruição da voz.
- [ ] Callback executa somente operações atômicas bounded; sem lock, alocação, log, I/O ou chamada XAudio2.
- [ ] Worker é o único proprietário que remove/libera buffers da coleção.
- [ ] Testes unitários do tracker cobrem zero, uma e várias conclusões, bem como wrap/overflow conforme o tipo escolhido.
- [ ] Falha de `SubmitSourceBuffer` não deixa buffer fantasma nem aguarda callback inexistente.
- [ ] Shutdown não libera buffers antes da destruição confirmada da voz.
- [ ] Nenhum flush é introduzido nesta etapa; caso necessário, a estratégia de conclusão é reavaliada.
- [ ] XAudio2 migra para a mesma FIFO `StereoFrame` sem usar ponteiro para memória reutilizável da FIFO.
- [ ] Windows x64 CI executa fmt, Clippy com `-D warnings` e testes no HEAD exato.
- [ ] Validação física em Windows é registrada separadamente; CI não é evidência de reprodução audível.

## 8. Escopo excluído

- Não altera WASAPI, `PcmSink`, `NativeAudioSession`, DSP/resampler, Scheduler, worker SMS, Pause/Stop/Resume/StepFrame ou GUI.
- Não implementa OUT-E (mute/flush/epoch/prebuffer) nem declara OUT-B concluída.
- Não altera versão de `windows` (0.62.2) ou `rtrb` (0.3.6).

Este documento é o gate de projeto para a implementação XAudio2. A estratégia deve ser demonstrada por código e testes Windows antes de migrar o transporte.
