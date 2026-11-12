# Saída física de áudio — CPAL 0.18.2

**Estado:** implementação candidata na branch `feat/cpal-audio-backend-20261112`; PR em preparação.
**Validação automatizada:** pendente no HEAD final da PR.
**Validação física Windows x64:** NOT RUN.
**Escopo:** substituir os adaptadores próprios WASAPI/XAudio2 por um único adaptador CPAL, mantendo a geração PCM e o runtime DSP independentes do dispositivo.

## Arquitetura

`SMS/SG-1000 PSG → NativeAudioSession → PcmSink → NativeCpalOutput → PcmFrameProducer → SPSC FIFO → callback CPAL → dispositivo`

- `native/audio_runtime` mantém mixer, resampler, volume, lifecycle e contrato `PcmSink`.
- `native/audio_output` mantém transporte SPSC e conversão PCM16 mono para frames estéreo.
- `NativeCpalOutput` é o único adaptador de saída física do projeto.
- CPAL enumera dispositivos, identifica dispositivos por ID estável, negocia o formato padrão e cria o stream.
- `native/sega_desktop` e o shell anterior usam o mesmo adaptador, sem backends concorrentes.
- O PSG e o MachineScheduler não conhecem CPAL, dispositivos, sample rate de saída ou clocks do host.

No Windows, CPAL usa seu backend WASAPI interno. Isso não é dependência do adaptador WASAPI próprio: o projeto não importa o crate `wasapi`, não chama IAudioClient/IAudioClient3 e não mantém um caminho de saída XAudio2.

## Callback e transporte

- O produtor PCM pertence ao worker de emulação; o consumidor SPSC pertence ao callback CPAL.
- A FIFO é limitada, drop-newest e contabiliza frames aceitos/perdidos.
- O callback não executa emulação, não chama o Scheduler, não acessa GUI, não faz I/O e não aloca buffers.
- Os buffers temporários do callback são pré-alocados; underruns são preenchidos com silêncio.
- O formato negociado pode ser inteiro ou float, com conversão a partir de PCM16.
- Saída mono recebe downmix de L/R; em dispositivos com mais de dois canais, os canais adicionais recebem silêncio.
- O sample rate real negociado é fornecido à sessão DSP; não se presume 44,1 kHz.

## Dispositivos e lifecycle

A API do adaptador oferece enumeração de dispositivos com ID estável, abertura do dispositivo padrão ou de um ID previamente enumerado, abertura/fechamento do stream, pause e retomada.

CPAL cria streams pausados; a sessão chama `play()` somente ao retomar a execução. Erros do callback são registrados e propagados em operações subsequentes. Reconexão automática/hotplug ainda não está implementada e permanece pendente.

## Limite de flush

CPAL oferece pause/play portável, mas não oferece uma operação genérica para esvaziar buffers que já foram entregues ao sistema operacional/dispositivo. `suspend_and_flush` suspende o stream e agenda o descarte da FIFO de aplicação antes de consumir dados no próximo epoch. Não promete cortar amostras que já estejam no buffer interno do driver/dispositivo; a latência residual deve ser medida em hardware real.

Esse limite não altera a timeline emulada e não autoriza compensar atraso modificando clocks do PSG ou do MachineScheduler.

## Métricas

Mantidas no contrato genérico:

- sample rate efetivo;
- capacidade, ocupação e high-water da FIFO;
- frames aceitos, consumidos e descartados;
- underrun em frames e eventos;
- blocos com overflow;
- frames descartados na invalidação da FIFO.

Foram removidas métricas exclusivas de XAudio2, como source-voice starvation e falhas de SubmitSourceBuffer.

## Dependências removidas do adaptador

- crate `wasapi`;
- bindings diretos `windows` para WASAPI;
- bindings diretos `windows` para XAudio2;
- implementação própria de workers, source voices, callbacks e staging de buffers XAudio2.

D3D12 é tecnologia de vídeo e não integra o subsistema de áudio. Suas referências no shell legado pertencem exclusivamente à apresentação e serão retiradas no corte de vídeo R5.

## Critérios ainda pendentes para concluir R3

1. CI Windows x64 Release: fmt, Clippy, testes e build do adaptador, desktop Slint e shell legado.
2. Enumeração/seleção real de dispositivos e formatos.
3. Reprodução PCM real do SMS em 44,1/48 kHz e formatos efetivamente negociados.
4. Pause, stop, reset, troca de ROM e retomada sem vazamento de epoch.
5. Desconexão/reconexão e recuperação explícita.
6. Campanha prolongada, underruns, latência e loopback físico.
7. Verificação de que áudio físico não altera Scheduler, determinismo ou equivalência contínua/em blocos.

Até a conclusão desses gates, R3 permanece **IN_PROGRESS**, não concluída.
