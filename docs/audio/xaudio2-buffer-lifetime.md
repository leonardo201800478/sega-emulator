# OUT-D — ownership e conclusão de buffers XAudio2

**Status:** contrato aplicado no backend XAudio2; validação Windows da migração PCM FIFO pendente.
**Base do contrato:** PR #652, integrada na main.
**Implementação de callback:** PR #653, integrada na main.
**Migração PCM FIFO:** PR em preparação sobre a main `604346b9abd8d7b9505ced8ae9007927f9e32535`.

## 1. Contrato Microsoft

A Microsoft especifica que:

1. A memória da estrutura `XAUDIO2_BUFFER` pode ser liberada quando `SubmitSourceBuffer` retorna.
2. A memória apontada por `pAudioData` não pode ser liberada até `IXAudio2VoiceCallback::OnBufferEnd` ou até a voz ser parada/destruída.
3. `OnBufferEnd` ocorre depois que o último byte do buffer foi consumido e antes do primeiro byte do buffer seguinte ser consumido.
4. Callbacks XAudio2 executam na thread de processamento de áudio e precisam retornar rapidamente. Não devem fazer I/O, chamadas caras/bloqueantes, sincronização com outras threads ou trabalho significativo.
5. `FlushSourceBuffers` gera callbacks para buffers removidos, mas esses callbacks podem ocorrer fora da ordem de submissão.
6. `DestroyVoice` aguarda a thread de processamento ficar ociosa. Depois do retorno, a voz não executará mais callbacks nem lerá dados de áudio.

Fontes:
- Microsoft, [XAUDIO2_BUFFER](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/ns-xaudio2-xaudio2_buffer).
- Microsoft, [IXAudio2VoiceCallback::OnBufferEnd](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voicecallback-onbufferend).
- Microsoft, [XAudio2 Callbacks](https://learn.microsoft.com/en-us/windows/desktop/xaudio2/xaudio2-callbacks).
- Microsoft, [IXAudio2SourceVoice::FlushSourceBuffers](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2sourcevoice-flushsourcebuffers).
- Microsoft, [IXAudio2Voice::DestroyVoice](https://learn.microsoft.com/en-us/windows/win32/api/xaudio2/nf-xaudio2-ixaudio2voice-destroyvoice).

## 2. Implementação integrada na main

A PR #653 substituiu o polling de `BuffersQueued` como prova de lifetime por confirmação explícita `OnBufferEnd`:

- O callback incrementa um contador atômico.
- `OnVoiceError` sinaliza erro atomicamente.
- O worker é o único proprietário da coleção de buffers submetidos.
- O callback e os buffers permanecem vivos até `DestroyVoice`.
- O caminho de shutdown não tenta reclaim individual com base na ordem dos callbacks de Flush.

A implementação de callback foi construída com a API `IXAudio2VoiceCallback::new` do crate `windows 0.62.2`. O adaptador COM empresta o estado do callback; ambos permanecem vivos até a destruição da voz.

## 3. Modelo de staging após a migração PCM FIFO

A migração OUT-B mantém o contrato de lifetime e altera apenas a origem dos dados:

1. O worker consome frames de `PcmFrameConsumer`.
2. Cada lote é copiado para uma alocação própria de `Vec<StereoFrame>` preparada em um pool de staging do worker.
3. O ponteiro de `pAudioData` aponta para a alocação estável do staging; nunca aponta para a FIFO ou scratch reutilizável.
4. A coleção de buffers submetidos mantém os `Vec<StereoFrame>` vivos até o worker observar as conclusões.
5. O callback somente incrementa o contador; o worker remove buffers concluídos e os devolve ao pool.
6. Se `SubmitSourceBuffer` falhar, o worker remove o staging recém-inserido. A submissão rejeitada não é aguardada como se houvesse callback pendente.
7. O pool tem capacidade fixa; as coleções `VecDeque` são reservadas antes do loop. Nenhuma alocação é feita no callback.

A contagem simples de conclusões é válida apenas no playback FIFO normal. O worker não faz Flush durante reprodução normal.

## 4. Shutdown e falhas de inicialização

- O worker interrompe novas submissões.
- Executa `Stop` e `FlushSourceBuffers` conforme o caminho de shutdown existente.
- Não tenta liberar buffers individualmente com callbacks de Flush, cuja ordem não é garantida.
- Chama `DestroyVoice` antes de liberar buffers ou callback.
- Depois do retorno de `DestroyVoice`, libera a coleção de staging e o adaptador COM.
- Se a criação da source voice tiver sucesso, mas `StartEngine` ou `Start` falhar, o caminho de erro destrói a source voice antes de deixar o callback sair de escopo.

## 5. Limites e validação

- OUT-B só será concluída após CI Windows aprovado no HEAD final e regressão aplicável.
- Testes unitários do bookkeeping não substituem compilação Windows real.
- CI não comprova saída audível em hardware físico.
- Flush/epoch/mute/prebuffer continuam fora do escopo (OUT-E).
- Não alterar Scheduler, PSG, DSP, VDP, runtime ou GUI nesta etapa.
