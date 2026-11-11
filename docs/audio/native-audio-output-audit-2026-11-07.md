# Auditoria do caminho de saída de áudio nativo — 2026-11-07

**Baseline:** `main` — `f47f71830344b85f72329376505d400bfbfaae69`.
**Escopo:** `native/audio_output`, `native/audio_runtime` e `rust/runtime/sms-runtime`.
**Status:** auditoria estática; não comprova desempenho ou estabilidade em dispositivo Windows.
**Alterações de runtime:** nenhuma nesta auditoria.

## 1. Arquitetura observada

O caminho atual é:

`SmsTemporalMachine → drain_audio_samples → AudioSession → NativeAudioSession → PcmResampler → PcmSink → WASAPI/XAudio2`.

O worker SMS executa `step_frame`, publica vídeo, drena as amostras e entrega o lote de áudio à sessão. O áudio é submetido uma vez por frame, fora do caminho de instruções individuais. A thread do backend consome a fila; ela não deve avançar nem regular o MachineScheduler.

A sessão usa mixer mono, resampler PCM e sink abstrato. O sink recebe blocos PCM16 em bytes. A saída nativa expande mono para estéreo na fronteira do backend.

## 2. WASAPI Shared / Event-Driven

O backend atual já usa Shared Mode orientado a eventos. Tenta `IAudioClient3` com `get_shared_mode_engine_period` e `initialize_shared_audio_stream`; se indisponível, usa a inicialização Shared/Event convencional. O worker espera o evento, consulta o espaço disponível e escreve exatamente o número de frames solicitado.

Transporte observado:
- SPSC limitado de `Vec<u8>`, com capacidade expressa em blocos;
- cada chamada `write_pcm` publica um bloco estéreo completo;
- se a fila estiver cheia, o bloco novo é descartado e `dropped_blocks` aumenta;
- o consumidor mantém bytes parciais em `VecDeque<u8>`, permitindo que um bloco atravesse mais de um evento;
- se o consumidor não conseguir completar o buffer solicitado, preserva os bytes já acumulados, completa o restante com silêncio e incrementa `underruns` uma vez naquele evento;
- a métrica atual conta eventos com falta, não a quantidade exata de frames/samples ausentes.

O WASAPI não é uma implementação diferente da pretendida: já é Event-Driven. O trabalho pendente está na política de transporte, telemetria, latência, recuperação e transições de estado.

## 3. XAudio2

O XAudio2 também recebe blocos PCM estéreo através de uma fila SPSC limitada. Diferentemente do WASAPI, divide os lotes em blocos cujo tamanho máximo deriva de `buffer_size * 4` bytes. Quando a fila enche, descarta o bloco novo e contabiliza `dropped_blocks`.

A implementação atual usa uma thread própria e consulta o estado de buffers enfileirados por polling. A interface de callback de voz ainda não está conectada. A alternativa deve ser preservada para comparação; não há evidência suficiente para descartá-la nem para declará-la superior.

## 4. Defeito de transição de estado: Pause / Stop / StepFrame

No worker SMS, `Pause` e `Stop` apenas definem `running = false` e atualizam telemetria. Não há comando correspondente para:
- silenciar imediatamente a saída física;
- descartar ou invalidar PCM já enfileirado;
- reiniciar a FIFO em uma fronteira conhecida;
- exigir prebuffer antes da retomada.

Consequentemente, após pausar, buffers já submetidos podem continuar sendo reproduzidos até esvaziarem. Isso é compatível com relatos de áudio residual durante a pausa, mas a auditoria estática não prova que explique todo chiado observado.

`StepFrame` chama o mesmo `execute_frame` e entrega amostras à sessão de áudio. Portanto, o passo-a-passo de depuração também pode produzir áudio físico. A semântica desejada deve ser definida explicitamente: executar um frame pausado não deve necessariamente reproduzir áudio no dispositivo; a máquina ainda deve avançar seus chips e estado de áudio deterministicamente.

## 5. Riscos e prioridades

| ID | Achado | Impacto | Próxima ação |
|---|---|---|---|
| OUT-01 | Fila limitada por blocos `Vec<u8>`, não por frames PCM com leitura parcial explícita | Overflow descarta unidades inteiras; capacidade/idade são pouco observáveis | Especificar FIFO bounded de frames, leitura parcial e contadores em frames |
| OUT-02 | Underrun WASAPI contado por evento | Não mede a dimensão da lacuna nem permite comparar backends com precisão | Contar frames/samples de silêncio inseridos e duração dos episódios |
| OUT-03 | Pause/Stop não silenciam nem invalidam PCM pendente | Som residual após pausa e transições pouco previsíveis | Contrato de mute/flush/epoch e retomada com prebuffer |
| OUT-04 | StepFrame encaminha áudio ao sink físico | Debug pode tocar áudio enquanto a execução contínua está pausada | Separar evolução determinística da máquina da política de apresentação host |
| OUT-05 | XAudio2 usa polling e não callback de conclusão | Trabalho periódico e possível atraso de reação | Avaliar callback mínimo e propriedade segura dos buffers |
| OUT-06 | Telemetria expõe apenas underruns e blocos descartados | Não permite diagnosticar profundidade, idade, frames perdidos ou latência | Expor occupancy, high-water/low-water, underrun frames, dropped frames e latência estimada/medida |
| OUT-07 | Produção ocorre por frame e a fila é limitada por blocos | Burstiness e capacidade precisam ser quantificados | Medir frames PCM por lote, tamanho máximo, cadência e margem de fila |

## 6. Contrato arquitetural obrigatório

1. MachineScheduler é a única autoridade temporal dos chips. WASAPI/XAudio2 nunca avançam, pausam ou corrigem o Scheduler.
2. PSG, FM e PCM/CD-DA produzem fontes digitais com relógios e semânticas próprios. A mixagem alinha as fontes em uma linha temporal determinística antes de converter para PCM de host.
3. A thread de saída apenas consome PCM já produzido e preenche o buffer físico solicitado pelo backend.
4. O transporte host é bounded, não bloqueia o Scheduler e mede precisamente overflow/underrun.
5. Pause/Stop precisam definir silêncio físico imediato e política explícita para PCM pendente, sem alterar o estado congelado dos chips.
6. StepFrame avança a máquina de modo determinístico, mas a reprodução física durante depuração deve ser uma política separada e testável.
7. Save states incluem estado dos chips e DSP determinístico na fronteira definida; não incluem estado físico do endpoint, nem buffers já entregues ao driver.
8. WASAPI Event-Driven Shared permanece backend padrão candidato; XAudio2 permanece backend alternativo. A escolha final depende de medições equivalentes no mesmo hardware.

## 7. Plano incremental recomendado

- **OUT-A — contrato e testes headless:** definir semântica da FIFO por frames, leitura parcial, overflow, underrun e transições Pause/Resume/StepFrame.
- **OUT-B — transporte comum:** implementar e validar a FIFO bounded independente de backend, com testes de chunking e invariância contínua/em blocos.
- **OUT-C — WASAPI:** adaptar o consumidor Event-Driven ao contrato comum, preencher somente frames ausentes com silêncio e medir a falta em frames.
- **OUT-D — XAudio2:** adaptar ao mesmo contrato; avaliar callback de conclusão sem trabalho pesado em callback.
- **OUT-E — integração worker:** mute/flush/epoch, prebuffer na retomada e política de áudio no StepFrame.
- **OUT-F — telemetria e tuning:** medir occupancy, latência e estabilidade; seleção automática determinística baseada em limites do endpoint e observações, nunca em IA opaca.
- **OUT-G — validação física:** testes prolongados em Windows com WASAPI e XAudio2, reportando cada backend separadamente.

Não alterar a temporização de hardware para compensar o relógio do endpoint. Se for necessária correção de deriva, ela pertence ao resampler assíncrono da fronteira host e deve preservar a linha temporal determinística da máquina.

## 8. Estado de validação

| Atividade | Estado |
|---|---|
| Inspeção estática do WASAPI no SHA de baseline | PASS |
| Inspeção estática do XAudio2 no SHA de baseline | PASS |
| Inspeção do contrato `PcmSink` / `NativeAudioSession` | PASS |
| Inspeção de Pause / Stop / StepFrame no worker | PASS |
| Testes novos de FIFO e transições | NOT RUN |
| Cargo fmt / Clippy / workspace tests desta auditoria | NOT RUN |
| Comparação de latência e estabilidade em Windows | NOT RUN |
| Teste físico com dispositivos WASAPI/XAudio2 | BLOCKED / NOT RUN |

Esta auditoria não declara nenhum backend aprovado para latência mínima ou reprodução sem engasgos.