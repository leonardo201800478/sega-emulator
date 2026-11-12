# Saída física de áudio — CPAL 0.18.2

**Estado:** CPAL 0.18.2 integrado pela PR #770; adaptação Hi-Fi H4 em implementação na branch `feat/audio-h4-float-pcm-cpal-negotiation-20261112`.
**Validação automatizada H4:** pendente no HEAD final da PR.
**Validação física Windows x64:** NOT RUN.
**Escopo:** substituir os adaptadores próprios WASAPI/XAudio2 por um único adaptador CPAL, mantendo a geração PCM e o runtime DSP independentes do dispositivo.

## Arquitetura

`SMS/SG-1000 PSG → NativeAudioSession → PcmSink → NativeCpalOutput → PcmFrameProducer → SPSC FIFO → callback CPAL → dispositivo`

- `native/audio_runtime` mantém mixer, resampler, volume, lifecycle e contrato `PcmSink::write_samples(Vec<f64>)`.
- `native/audio_output` mantém transporte SPSC com `StereoFrame = [f64; 2]` e conversão mono→estéreo.
- `NativeCpalOutput` é o único adaptador de saída física do projeto.
- CPAL enumera dispositivos, identifica dispositivos por ID estável e negocia uma tupla suportada de canais/taxa/formato. A seleção H4 prioriza stereo→mono, F32→I24→I16 e, dentro de cada formato, 48→88,2→96→44,1 kHz. O default CPAL é fallback somente quando nenhum desses formatos/rates está anunciado.
- `native/sega_desktop` e o shell anterior usam o mesmo adaptador, sem backends concorrentes.
- O PSG e o MachineScheduler não conhecem CPAL, dispositivos, sample rate de saída ou clocks do host.

No Windows, CPAL usa seu backend WASAPI interno. Isso não é dependência do adaptador WASAPI próprio: o projeto não importa o crate `wasapi`, não chama IAudioClient/IAudioClient3 e não mantém um caminho de saída XAudio2.

## Callback e transporte

- O produtor PCM pertence ao worker de emulação; o consumidor SPSC pertence ao callback CPAL.
- A FIFO é limitada, drop-newest e contabiliza frames aceitos/perdidos.
- O callback não executa emulação, não chama o Scheduler, não acessa GUI, não faz I/O e não aloca buffers.
- Os buffers temporários do callback são pré-alocados; underruns são preenchidos com silêncio.
- A fonte interna permanece em f64 normalizado até a fronteira CPAL; não existe quantização PCM16 intermediária.
- A conversão final usa `FromSample<f64>` no callback para o formato efetivamente negociado. I16 é fallback implementado; I24 e F32 são escolhidos somente quando anunciados pelo endpoint. I24 usa armazenamento de 4 bytes com 24 bits significativos; F32 é float, não deve ser rotulado como I24.
- Saída mono recebe downmix de L/R; em dispositivos com mais de dois canais, os canais adicionais recebem silêncio.
- O sample rate real negociado é fornecido à sessão DSP; a fonte permanece em 48 kHz e o resampler converte para 48/88,2/96 kHz quando disponíveis, com 44,1 kHz como fallback de taxa.

## Dispositivos e lifecycle

A API do adaptador oferece enumeração de dispositivos com ID estável, abertura do dispositivo padrão ou de um ID previamente enumerado, abertura/fechamento do stream, pause e retomada.

CPAL cria streams pausados; a sessão chama `play()` somente ao retomar a execução. Erros do callback são registrados e propagados em operações subsequentes. Reconexão automática/hotplug ainda não está implementada e permanece pendente.

## Limite de flush

CPAL oferece pause/play portável, mas não oferece uma operação genérica para esvaziar buffers que já foram entregues ao sistema operacional/dispositivo. `suspend_and_flush` suspende o stream e agenda o descarte da FIFO de aplicação antes de consumir dados no próximo epoch. Não promete cortar amostras que já estejam no buffer interno do driver/dispositivo; a latência residual deve ser medida em hardware real.

Esse limite não altera a timeline emulada e não autoriza compensar atraso modificando clocks do PSG ou do MachineScheduler.

## Métricas

Mantidas no contrato genérico:

- sample rate efetivo;
- log `R3_AUDIO CPAL output opened` informa formato CPAL real (I16/I24/F32 ou fallback) e flag `default_config_fallback`;
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

## Critérios ainda pendentes para concluir R3/H4

1. CI H4 no HEAD exato: fmt, Clippy, testes de resampler, seleção de configurações, conversões e build Windows x64.
2. Testar em endpoint físico que anuncia F32, I24 e I16; confirmar que a seleção respeita as capacidades reais e reporta o formato.
3. Reprodução SMS em 48/88,2/96 kHz quando disponíveis e fallback para PCM16/44,1 kHz quando necessário.
4. Pause, stop, reset, troca de ROM e retomada sem vazamento de epoch.
5. Desconexão/reconexão e recuperação explícita (ainda não implementadas automaticamente).
6. Campanha prolongada, underruns, latência e loopback físico.
7. Verificação de que áudio físico não altera Scheduler, determinismo ou equivalência contínua/em blocos.

Até a conclusão desses gates, R3 permanece **IN_PROGRESS**, não concluída.

**Lockfiles:** CPAL 0.18.2 e os lockfiles aplicáveis foram integrados pela PR #770. A branch H4 não altera dependências nem lockfiles; a CI `--locked` deve confirmar a consistência.
