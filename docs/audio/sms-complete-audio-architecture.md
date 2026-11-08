# Arquitetura do subsistema de áudio — Master System

**Status:** proposta arquitetural inicial; não representa implementação concluída nem certificação de fidelidade.
**Plataforma-alvo:** Sega Master System / Mark III, com reutilização dos componentes compartilhados também pelo SG-1000 e, quando aplicável, Game Gear.
**Crates atuais auditados:** `native/sn76489`, `native/audio_runtime`, `native/audio_output`, `native/machine_scheduler`, `native/temporal_core` e `native/sms_core`.
**Baseline auditada:** `main` em `ece2d634851313d9066e5d87d4b3235a9b8fcd9b` (2026-11-08).

## 1. Objetivo e limites desta proposta

Estabelecer o desenho técnico para completar o caminho de áudio do SMS sem criar uma implementação isolada por console, sem duplicar o PSG existente e sem colocar síntese, temporização ou DSP no frontend.

O escopo de hardware compreende:

1. PSG da família SN76489, já existente em `native/sn76489`, com perfis de hardware explícitos;
2. FM OPLL YM2413, ainda sem núcleo de síntese identificado na baseline auditada;
3. adaptação temporal dos dispositivos ao `MachineScheduler`;
4. mixagem de fontes digitais, filtragem/resampling e conversão PCM;
5. transporte de áudio para o backend nativo WASAPI/XAudio2;
6. serialização/restauração de estado determinístico dos dispositivos e dos acumuladores associados.

A proposta arquitetural original que introduziu este documento não adicionava o núcleo YM2413 nem modificava o algoritmo SN76489. A auditoria posterior da seção 11 registra uma correção independente de amostragem na PR #661; isso não conclui o YM2413 nem declara a saída física de áudio validada.

## 2. Auditoria da arquitetura existente

### 2.1 Componentes já presentes

| Componente | Situação observada na baseline | Papel preservado |
|---|---|---|
| `native/sn76489` / `sega-sn76489` | Implementa `Sn76489Core`, perfis `Sg1000` e `Sms`, divisor interno, geradores de tom/ruído, atenuação, buffer de amostras e FIR | Núcleo digital compartilhado do PSG |
| `native/audio_runtime` / `sega-audio-runtime` | Implementa `ScheduledPsg`, `AudioMixer`, `OnePoleLowPass`, `PcmResampler`, `NativeAudioSession` e `PcmSink` | Integração temporal, DSP e fronteira independente de host |
| `native/audio_output` / `sega-audio-output` | Workspace standalone com `rtrb` e dependências Windows `wasapi` / `windows` | Saída física e transporte entre produtor/consumidor |
| `native/machine_scheduler` | Mantém domínios racionais de clock e entrega `ClockAdvance` em cada dispatch | Fonte única de avanço temporal |
| `native/sms_core` | Depende do PSG, runtime de áudio e Scheduler | Integração do perfil SMS |

### 2.2 Lacuna principal

A CI já registra explicitamente que a arquitetura YM2413/OPLL existe apenas como especificação e que não há, na baseline, um pacote/core FM executável. Portanto, a primeira entrega de implementação será um núcleo YM2413 compartilhado dentro da camada de áudio existente, não um novo subsistema SMS acoplado à máquina.

### 2.3 Princípio de não duplicação

O PSG atual não deve ser reescrito para acomodar o FM. PSG e OPLL são fontes independentes que convergem na interface comum de áudio. O perfil de máquina decide quais fontes estão presentes, seus clocks, roteamento e ganho. SG-1000 continua instanciando o PSG com seu perfil próprio; SMS instancia o perfil SMS e pode habilitar FM apenas em configurações de hardware que realmente possuam a expansão OPLL.

## 3. Correções arquiteturais importantes à especificação inicial

A especificação recebida é um ponto de partida, mas alguns detalhes não podem ser transformados em constantes de implementação sem auditoria documental e diferencial:

1. **Não impor `catch_up(target_cycle)` como mecanismo paralelo ao Scheduler.** O PSG existente já consome `ClockAdvance` do `MachineScheduler` por `ScheduledPsg::apply_clock_advance`. O YM2413 deve seguir o mesmo contrato: receber o número de bordas de clock produzidas pelo domínio registrado. Uma API de catch-up pode existir como detalhe interno, desde que não crie uma segunda linha temporal.
2. **Não assumir “16 ciclos de CPU por step” como regra universal.** O núcleo SN76489 já possui divisor interno de 16 clocks do chip. Isso não equivale automaticamente a 16 T-states do Z80. A razão entre master clock da máquina e clock de entrada do dispositivo deve vir do perfil regional/revisão de hardware e ser validada.
3. **Não fixar “72 ciclos de CPU por step” para OPLL sem evidência.** A taxa de atualização interna, o clock de entrada, as fases do pipeline e o ponto observável de escrita/leitura devem ser determinados a partir de documentação e comparadores de referência. O divisor será expresso em clocks do dispositivo ou em fases internas explicitamente modeladas.
4. **Não assumir um LFSR/tap único para todos os chips.** O PSG já diferencia perfis `Sg1000` e `Sms`. Qualquer mudança exige matriz de revisão de chip, documentação e testes diferenciais.
5. **Não ativar LPF genérico automaticamente como “modelo analógico do console”.** O runtime já possui FIR para conversão e um `OnePoleLowPass` opcional. Resposta analógica, carga, ganho e filtros do console devem ser calibrados separadamente; filtros digitais de anti-aliasing não são equivalentes a um modelo analógico.
6. **Não adotar `LazyLock` e tabelas de ponto flutuante como decisão antecipada de precisão.** Tabelas OPLL devem ter geração reproduzível, quantização e arredondamento documentados. Preferir tabelas inteiras determinísticas geradas/verificadas por testes; `LazyLock` é uma opção de armazenamento, não uma especificação de comportamento.
7. **Não introduzir uma thread/timer de precisão no núcleo de emulação.** A thread de host consome PCM e reporta telemetria; não governa clocks de chips. `timeBeginPeriod` não deve ser requisito para determinismo do hardware emulado.

## 4. Arquitetura-alvo

```text
SMS machine profile
  ├── MachineScheduler (master timeline)
  │     ├── PSG clock domain ──> ScheduledPsg ──> Sn76489Core
  │     └── OPLL clock domain ─> ScheduledYm2413 ─> Ym2413Core
  │
  ├── Audio frame/batch boundary
  │     ├── drain PSG source samples
  │     ├── drain OPLL source samples
  │     └── align source timelines / deterministic mix
  │
  └── sega-audio-runtime
        ├── source routing + gains
        ├── optional hardware-calibrated analog stages
        ├── rate conversion / PCM quantization
        └── PcmSink
              └── sega-audio-output
                    └── bounded SPSC ring buffer
                          └── WASAPI / XAudio2 consumer
```

### 4.1 Responsabilidades

- **`sega-sn76489`:** mantém a semântica digital SN76489 já implementada e expõe evolução por clocks do próprio chip, escritas e estado exportável.
- **Núcleo OPLL compartilhado:** implementa registradores, endereçamento, operadores, canais, envelopes, LFO/modulação, rhythm/percussion e estado exportável; não conhece SMS, Windows ou GUI.
- **`sega-audio-runtime`:** faz a ligação de cada núcleo ao domínio de clock do Scheduler, coleta blocos, alinha e mistura fontes, executa DSP configurado e entrega PCM ao sink.
- **`sega-audio-output`:** transporta blocos por buffer limitado e implementa o consumidor do host. Não modifica estado de chips nem decide eventos do Scheduler.
- **`sega-sms-core`:** configura os dispositivos e encaminha transações de I/O para o dispositivo correto, preservando timestamp/ordem causal.
- **Frontend:** somente configura dispositivo de saída, taxa de amostragem, volume, latência e telemetria. Não executa síntese nem agenda clocks.

### 4.2 Contrato temporal

Cada dispositivo de áudio deverá:

- ser associado a um `ClockDomainId` estável e exclusivo dentro da máquina;
- receber os `ClockAdvance` produzidos pelo Scheduler, em ordem determinística;
- avançar exatamente o número de bordas informado, preservando divisores/fases fracionárias;
- aplicar escrita de registrador no timestamp do evento, depois de avançar o dispositivo até aquele instante;
- rejeitar avanço de domínio incorreto e timestamp regressivo;
- produzir a mesma sequência de estado/amostras para execução contínua e execução em blocos equivalentes.

O domínio de clock não deve ser inferido a partir da taxa de amostragem do host. Alterar 44,1 kHz para 48 kHz não pode alterar a quantidade de clocks de hardware nem o resultado lógico do chip.

### 4.3 Contrato de áudio

As fontes digitais precisam compartilhar uma convenção documentada de amplitude, tipo de amostra e instante de amostragem. O mixer não deve somar blocos desalinhados silenciosamente. Se PSG e OPLL tiverem diferentes frequências de produção, o runtime deverá usar uma linha temporal comum ou conversão determinística por fonte antes da mixagem.

O buffer de host deve ser limitado. Overflow/underrun gera telemetria explícita; não pode bloquear o Scheduler nem descartar silenciosamente amostras sem contador/diagnóstico. A política de overflow, recuperação e latência será definida no desenho da integração com `rtrb`.

## 5. Modelo funcional do YM2413

O núcleo OPLL deverá ser dividido internamente em componentes testáveis, sem pressupor que cada componente seja um crate separado:

- registrador de endereço e latch de dados;
- mapa de registradores e semântica de escrita;
- nove canais melódicos;
- dois operadores por canal (modulator/carrier);
- phase generator e incremento de fase;
- envelope generator com estados e taxas de ataque/decay/sustain/release;
- tabelas de atenuação/forma de onda e conversão log-seno/exponencial;
- feedback e modulação entre operadores;
- key-on/key-off, instrumentação e parâmetros de volume;
- LFO/vibrato/tremolo, quando suportados pelo chip;
- Rhythm Mode com os geradores de percussão e ruído específicos;
- saída mono/stereo conforme o roteamento real do hardware anfitrião, sem inventar panning inexistente no chip.

Os detalhes exatos de registradores, escalas, saturação, precisão de fase, ordem de atualização e comportamento de escrita deverão ser rastreados para fontes primárias e comparados com ao menos uma implementação de referência reconhecida. Divergências entre revisões/clones deverão ser representadas como perfil ou limitação documentada, não como heurística oculta.

## 6. Persistência e determinismo

O estado restaurável deve incluir, no mínimo:

- registradores e latch/endereço atual;
- contadores/divisores e fases de clock;
- fase de cada operador e acumuladores de feedback;
- estado completo dos envelopes e respectivos contadores;
- estados de LFO, ruído e rhythm/percussion;
- buffers de amostras pendentes ou uma fronteira segura que garanta equivalência após restore;
- estado dos filtros/resamplers/mixer pertencentes à máquina, quando sua continuidade fizer parte do contrato do save state;
- versão do formato e validação de invariantes.

A serialização deve ser DTO versionado e validado antes da restauração, sem aceitar estados parciais ou inconsistentes. A política de incluir o buffer do endpoint host é diferente: filas físicas/PCM já entregue ao dispositivo não são estado de hardware e não devem ser serializadas como se fossem parte do chip. O save state deve definir uma fronteira determinística de áudio e retomar sem salto lógico; a latência física do host pode ser reinicializada conforme política explícita.

## 7. Plano de implementação incremental

A sequência abaixo é proposta de execução. Cada fase terá PR pequena ou commits isolados, testes próprios e gates aplicáveis.

### A0 — Contratos e evidência (esta PR)

- Congelar os limites de responsabilidade e o contrato temporal descritos neste documento.
- Inventariar APIs e estado serializável do PSG, DSP, resampler, sessão e backend.
- Criar matriz de fontes técnicas para PSG SMS e YM2413/OPLL.
- Definir perfil de hardware SMS com FM opcional, sem habilitá-lo universalmente.

**Aceite:** revisão arquitetural aprovada; nenhum comportamento de hardware alterado.

### A1 — Auditoria e fechamento do PSG compartilhado

- Auditar a implementação atual contra documentação SN76489/SMS e SG-1000.
- Verificar divisores, períodos zero, ruído periódico/branco, seeds/taps por perfil, latch/data writes, atenuação, overflow de buffer e alinhamento de samples.
- Validar execução contínua versus blocos e snapshot/restore.
- Não reimplementar o PSG se a auditoria demonstrar conformidade suficiente; corrigir lacunas em PRs específicas.

**Aceite:** matriz de conformidade preenchida; testes determinísticos e diferenciais para cada comportamento coberto.

### A2 — YM2413: contrato de registradores e estado

- Criar o núcleo OPLL compartilhado e seu DTO de estado versionado.
- Implementar escrita de endereço/dados e mapa de registradores com testes isolados.
- Definir clocks e fronteiras de atualização com evidência técnica.

**Aceite:** testes de registradores, reset, key-on/off e round-trip de estado; sem declarar síntese concluída.

### A3 — YM2413: síntese melódica

- Implementar operadores, fase, feedback, envelopes e tabelas de onda/atenuação.
- Validar notas, envelopes, frequências, saturação e vetores de saída com oráculo independente.

**Aceite:** vetores diferenciais reproduzíveis e estabilidade contínua/chunked.

### A4 — Rhythm Mode e casos de borda OPLL

- Implementar percussion/noise e transições de Rhythm Mode.
- Cobrir simultaneidade de escrita, key transitions, resets e combinações de canais.

**Aceite:** matriz completa de casos suportados com referência independente; divergências explicitadas.

### A5 — Integração Scheduler + mixagem

- Adicionar adapter `ScheduledYm2413` simétrico ao adapter PSG.
- Integrar PSG/FM ao perfil de máquina e ao caminho comum de mixagem.
- Resolver alinhamento de amostras por timestamps e backpressure.

**Aceite:** equivalência contínua/chunked, ordem causal em escritas I/O, testes de buffer cheio/vazio e regressão SMS/SG-1000.

### A6 — Host output, telemetria e calibração

- Auditar o ring buffer SPSC e a consumer thread WASAPI/XAudio2 já existentes.
- Definir métricas de profundidade/idade da fila, underrun, overflow e latência.
- Calibrar ganho e resposta analógica somente com evidência de circuito/medições ou referência confiável.

**Aceite:** testes headless PASS e validação física Windows registrada separadamente; nenhum timer de host controla o Scheduler.

### A7 — Fechamento e auditoria sistêmica de áudio

- Validar save/load/replay, reset, pausa/retomada, troca de ROM e desligamento.
- Rodar regressão SMS e SG-1000; validar perfis com e sem FM.
- Publicar scorecard de áudio com PASS/FAIL/BLOCKED/NOT RUN por suíte.

**Aceite:** CI verde no HEAD exato, testes diferenciais e temporais completos, sem pendências ocultas.

## 8. Matriz mínima de validação

| Área | Cobertura exigida |
|---|---|
| PSG | tom, período zero, latch/data, volume, ruído periódico/branco, reset, perfil SMS/SG-1000 |
| OPLL registradores | endereço, dados, registradores reservados, reset, key-on/off |
| OPLL síntese | fase, frequência, envelopes, feedback, modulação, saturação |
| Rhythm | habilitação/desabilitação, cada instrumento, ruído e transições |
| Scheduler | domínio correto, fase fracionária, ordem causal, chunk invariance |
| Mixagem | alinhamento temporal, ganho, headroom, clipping/quantização |
| Buffer | capacidade limitada, overflow/underrun mensurado, consumidor não bloqueante |
| Save State | round-trip, estado inválido, versão incompatível, continuidade pós-restore |
| Regressão | SMS sem FM, SMS com FM quando suportado, SG-1000 sem regressão |
| Host | WASAPI/XAudio2 em Windows; headless sink determinístico em CI |

Cada suíte será classificada individualmente como **PASS**, **FAIL**, **BLOCKED** ou **NOT RUN**. Um teste ignorado ou não executado nunca será promovido a PASS.

## 9. Referências e rastreabilidade

Fontes já presentes no projeto ou fornecidas para esta modelagem:

- Documento de especificação de áudio anexado pelo mantenedor: PSG SN76489, YM2413/OPLL, mixer, DSP e saída Windows.
- Roadmap mestre do projeto: crates compartilhados `sega-audio`, `sega-scheduler` e `sega-hosts`; SMS como plataforma base.
- SMS Power: https://www.smspower.org/ — documentação de desenvolvimento SMS/Mark III e expansão FM.
- Repositório atual: https://github.com/leonardo201800478/sega-emulator

Antes de cada implementação de hardware, os links/documentos específicos, revisão do chip e afirmações temporais serão registrados em uma matriz de evidências com trechos verificáveis. Esta proposta não atribui precisão a valores apenas porque aparecem em um esqueleto de código.

## 10. Decisões desta PR

- Manter o PSG atual e evoluí-lo por auditoria, sem duplicação.
- Implementar YM2413 como núcleo compartilhado, não como módulo exclusivo do SMS.
- Fazer do Scheduler a única fonte de tempo.
- Separar amostragem interna de hardware da taxa de saída do host.
- Tratar FM como capacidade opcional de perfil de hardware.
- Manter saída física, DSP e GUI fora dos núcleos de síntese.
- Não incluir ROMs, BIOS ou dumps protegidos.
- Não realizar merge automaticamente; integração depende de revisão e CI verde no HEAD final.


## 11. Auditoria da amostragem PCM do SN76489 — 2026-11-08

### 11.1 Evidência do defeito

A auditoria do caminho SMS identificou que `Sn76489Core::advance_clocks_inner` preservava a contagem média de amostras com acumulador inteiro, mas emitia o valor FIR do tick interno atual no primeiro tick que ultrapassava a fronteira temporal da próxima amostra.

Para o perfil NTSC de 3.579.545 Hz, o divisor interno de 16 resulta em aproximadamente 223.721,56 ticks FIR por segundo. Com saída configurada em 44.100 Hz, o instante ideal de cada amostra normalmente cai entre dois ticks internos; a distância entre saídas varia entre cinco e seis ticks. O algoritmo anterior arredondava cada instante para o tick seguinte, sem interpolação fracionária. Os backends reproduziam então esses valores como se tivessem sido amostrados em instantes uniformes.

A taxa média não era o problema: a fase inteira preservava a contagem. O defeito estava na posição temporal dos valores PCM.

### 11.2 Experimento diferencial

O teste `fractional_output_sampling_reduces_clock_quantization_error` reproduz o filtro Kaiser NTSC/44,1 kHz atual (321 taps), aplica um tom sintético de 10 kHz e compara:

1. valor do FIR no tick interno mais próximo (algoritmo anterior);
2. interpolação entre as saídas FIR dos ticks adjacentes no instante fracionário calculado pelo acumulador;
3. referência do mesmo FIR avaliado no instante ideal.

Com origem temporal alinhada ao primeiro tick interno, a simulação numérica de referência obteve erro RMS normalizado aproximado de 16,2% no método anterior e 0,72% com interpolação linear. O teste automatizado exige erro superior a 5% no método antigo e inferior a 1% no interpolado, além de conferir a contagem exata de amostras.

Esses valores são erro em relação à referência temporal do filtro, não THD medido em loopback físico. Não devem ser interpretados como certificação acústica.

### 11.3 Correção no core

A PR #661 captura o valor FIR anterior antes de avançar o PSG, calcula o valor FIR após o tick atual e interpola usando a fração do cruzamento do acumulador:

`fraction = (clock_hz - previous_phase + sample_index * clock_hz) / (sample_rate * 16)`

A fase e a contagem permanecem inteiras; somente a avaliação do valor filtrado usa interpolação em ponto flutuante. Não são alterados o clock do chip, o divisor interno, o Scheduler, o ritmo de execução da máquina, a taxa PCM configurada ou o contrato do backend.

A alteração não introduz estado adicional e, portanto, não exige mudança de versão do snapshot do PSG.

### 11.4 Taxa média e percepção de lentidão

Para o perfil NTSC modelado:

- 262 linhas × 228 T-states = 59.736 T-states por frame;
- 3.579.545 T-states/s ÷ 59.736 ≈ 59,922743 frames/s;
- 44.100 ÷ 59,922743 ≈ 735,9476 amostras por frame.

O acumulador de fase carrega a fração entre avanços; a duração acumulada a partir das amostras acompanha o tempo lógico dentro de menos de uma amostra. A fórmula média não demonstra deriva sistemática de pitch. A percepção de lentidão ainda precisa ser separada de timbre/artefatos de fase e medida por loopback com timestamps e frequência efetiva de consumo.

### 11.5 Comparação externa e limites

O MAME configura a variante Sega PSG com divisor interno 8 e cria o stream em `clock()/2`. O loop de áudio emite esse stream de alta taxa e avança os contadores internos quando o divisor expira; portanto, o stream tem oito amostras por tick interno efetivo (`clock()/16`) antes da conversão para a taxa do host. Não é uma taxa física de saída do chip, mas uma estratégia de oversampling que evita selecionar diretamente um tick interno a cada amostra de 44,1 kHz. Referência: [MAME SN76496](https://sources.debian.org/src/mame/0.289%2Bdfsg1-1/src/devices/sound/sn76496.cpp), em especial `segapsg_device`, `device_start` e `sound_stream_update`.

O rsemu adota outra estratégia: declara `SAMPLE_DIVISOR = 80`, amostrando a cada 80 clocks de entrada (cinco ticks internos de 16 clocks) e preservando a taxa regional como racional: NTSC 3.937.500/88 Hz (≈44.744,32 Hz) e PAL 709.379/16 Hz (≈44.336,19 Hz). Isso separa a taxa natural escolhida para a fonte da taxa de saída do host e evita quantização variável dos instantes da fonte. É uma decisão de implementação, não uma frequência física documentada do pino de áudio. Referência: [rsemu SMS audio source](https://docs.rs/rsemu/latest/src/rsemu/host/audio/sms.rs.html), especialmente `SAMPLE_DIVISOR`, `NTSC_SAMPLE_RATE` e `resolve_rate`.

A solução atual da PR #661 mantém a taxa de host 44,1 kHz e interpola entre ticks internos. O caminho de rsemu é uma alternativa arquitetural futura que exigiria generalizar a taxa de entrada racional do `PcmResampler`; não é necessário para corrigir a falha demonstrada e não foi introduzido nesta PR.

Outra implementação pública, rsemu, declara uma taxa de amostragem nativa racional derivada do clock do PSG dividido por 80 (44.744,32 Hz NTSC e 44.336,19 Hz PAL), separando a taxa da fonte da taxa do host. É uma decisão de arquitetura de emulador, não uma frequência de amostragem física documentada do pino do chip. Referência: [rsemu SMS audio source](https://docs.rs/rsemu/latest/src/rsemu/host/audio/sms.rs.html).

A interpolação fracionária corrige a quantização de instante identificada; não modela o filtro analógico do console, não certifica variantes de placa e não substitui captura física. Underruns WASAPI permanecem uma questão independente de transporte e cadência de consumo.

### 11.6 Validação registrada na PR #661

No HEAD `58fbddcc9ca52d5ecda46ec6743235f8144782a8`:

- `cargo fmt --all -- --check`: PASS;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS;
- `cargo test --workspace --locked`: PASS; o teste diferencial foi executado e passou;
- validação standalone Linux de áudio: PASS;
- validação Windows WASAPI/XAudio2 (formatação, Clippy e 26 testes): PASS.

As validações de host são headless/automatizadas. Não houve captura de loopback físico nem teste auditivo pós-correção. A PR permanece sujeita a revisão e não foi integrada à main.
