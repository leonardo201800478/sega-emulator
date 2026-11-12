# Plano Modern Hi-Fi — SMS e SG-1000

**Status:** H2 em validação; reconstrução event-based implementada como protótipo exclusivamente de teste. Nenhuma alteração de síntese foi promovida à produção.
**Branch de trabalho:** `feat/modern-hifi-audio-foundation-20261112`
**Baseline de main após atualização:** `b375d4bb85149678b82ab39a497177d1a1cd2319` (2026-11-12).
**Baseline sonora anterior de comparação:** `f47f71830344b85f72329376505d400bfbfaae69`.
**Escopo inicial:** SN76489, pipeline PCM, saída nativa e preparação para CPAL 0.18.2.
**Não autoriza merge:** toda implementação será submetida a PR e validada antes de qualquer integração.

## 1. Diretriz do produto

O emulador deve representar o SG-1000/Master System como se o hardware original tivesse recebido um upgrade de saída de áudio Hi-Fi estéreo.

A intenção não é simular a televisão CRT, a transmissão RF, o cabo RF ou as limitações de áudio mono do equipamento histórico. Devemos preservar a identidade sonora e o comportamento digital dos dispositivos Sega, mas reconstruir e transportar o áudio com qualidade moderna.

### Regras normativas

1. O comportamento digital do SN76489 e os perfis SMS/SG-1000 permanecem distintos e não podem ser alterados apenas para produzir uma preferência sonora.
2. O MachineScheduler continua sendo a única autoridade temporal. A taxa de amostragem, o dispositivo físico e a fila de áudio nunca avançam ou corrigem clocks de hardware.
3. O alvo preferencial de produção é **48.000 Hz, estéreo, 24 bits**. A configuração efetiva será negociada com o dispositivo; nunca forçar formato não suportado.
4. A precisão interna e o processamento DSP devem permanecer em ponto flutuante de alta precisão ou representação equivalente. A redução para 24 bits ocorre somente na fronteira final de saída.
5. Para SMS e SG-1000, a fonte PSG permanece mono e é apresentada como dual-mono L/R. Não inventar separação estéreo, reverberação, chorus, widening ou canal LFE.
6. Sistemas 2.1 recebem o sinal estéreo normal; crossover e bass management pertencem ao sistema operacional, receiver ou amplificador.
7. Filtros de reconstrução/anti-aliasing não devem ser confundidos com modelos analógicos da placa. Nenhum filtro tonal arbitrário ou reforço de graves será ativado por padrão.
8. A qualidade auditiva, medições objetivas e determinismo são critérios de aceite. Mais fidelidade teórica não justifica regressão sonora.

## 2. Arquitetura-alvo

```text
MachineScheduler
    │ ClockAdvance — tempo de hardware inalterado
    ▼
SN76489 / perfis SMS e SG-1000
    │ fonte digital mono, amostras determinísticas
    ▼
Audio source / mixer (f64 ou precisão equivalente)
    │ linha temporal de áudio de referência: 48 kHz
    ▼
DSP e resampler de saída
    │ sem PCM16 intermediário
    ▼
PCM final / conversão de formato
    │ estéreo, I24 preferencial; F32 como alternativa negociada
    ▼
CPAL 0.18.2
    │ stream e formato realmente suportados
    ▼
Sistema de áudio / receiver / caixas / subwoofer
```

O perfil de saída é independente do clock do PSG. Uma troca de dispositivo pode reconstruir o resampler e a sessão de host, mas não pode modificar o estado digital do chip nem a linha temporal do Scheduler.

## 3. Auditoria inicial da main

### 3.1 Estado observado

| Área | Estado observado em 1f98f4c | Implicação |
|---|---|---|
| SN76489 | Core compartilhado com perfis SMS/SG-1000, FIR e amostragem fracionária | Preservar semântica; medir antes de trocar algoritmo |
| Fonte temporal SMS | `DEFAULT_AUDIO_SAMPLE_RATE = 44_100` em `native/sms_core/src/temporal_bus.rs` | Fonte ainda não está padronizada em 48 kHz |
| Worker SMS | `SMS_TEMPORAL_SAMPLE_RATE = 44_100` | Pacing de áudio usa 44,1 kHz |
| Mixer | Mono, precisão `f64`, ganho sem clipping | Bom ponto de partida; preservar headroom |
| PcmResampler | Entrada fixa em 44,1 kHz; FIR polifásico de 63 taps e 1024 fases | Generalizar taxa de entrada e preservar continuidade entre blocos |
| Quantização | `pcm16()` serializa PCM16 antes do sink | Gargalo real para uma saída 24-bit |
| Sessão | `PcmSink::write_pcm(Vec<u8>)` recebe bytes PCM16 | Contrato precisa deixar de pressupor PCM16 |
| Saída legada | `native/audio_output` expande mono PCM16 para frames estéreo | Dual-mono existe, mas ainda em PCM16 |
| CPAL | PR #770 integrada à main | Backend CPAL 0.18.2 integrado; adaptação Hi-Fi do contrato PCM permanece pendente |

### 3.2 Interpolação do PSG: comparação com o baseline

A main atual usa interpolação fracionária entre duas saídas FIR. O baseline `f47f718` amostrava a saída filtrada no tick em que a fase de saída cruzava a fronteira.

A auditoria não encontrou prova de que a condição temporária zero nos ticks sem emissão cause defeito: nesses ticks o valor não é consumido; no tick de emissão, a saída filtrada anterior é recalculada antes de inserir a amostra atual no FIR.

**Conclusão:** a mudança de algoritmo é real; regressão sonora ainda não está demonstrada. Não alterar nem reverter esse caminho sem um teste A/B PCM reproduzível e métricas comparáveis.

## 4. Plano de execução e gates

### H0 — Inventário e baseline arquitetural

**Status: CONCLUÍDO (auditoria estática).**

- [x] Confirmar SHA da main e PR #770.
- [x] Identificar taxas de amostragem atuais.
- [x] Identificar o ponto de quantização PCM16.
- [x] Confirmar mono PSG e duplicação dual-mono na saída.
- [x] Comparar o caminho de amostragem atual com o baseline anterior.
- [ ] Executar testes locais/workflow no branch de trabalho.

### H1 — Referência sonora reproduzível

**Status: CONCLUÍDO — captura histórica integral e determinismo comprovados pela CI.**

1. [x] Criar trace determinístico de registradores e clocks no teste do core, sem ROM comercial: tons 0–2, volumes, ruído branco e mudanças de registradores durante a execução.
2. [x] Gerar PCM determinístico de teste para SMS e SG-1000 a 48 kHz com três tons, ruído branco e alterações de volume/controle durante a execução. A saída é comparada entre avanço contínuo e avanço segmentado em blocos de 997 clocks. [x] Exportar os mesmos traces para WAV/PCM bruto para comparação externa.
3. [x] Capturar e comparar o mesmo trace no baseline `f47f718` e na implementação atual; o artefato A/B integral foi publicado.
4. Registrar WAV/PCM bruto, taxa, perfil, clock, sequência de writes e SHA do código.
5. Comparar RMS, pico, DC offset, frequência, resposta espectral, aliasing, transientes, ruído, clipping e erro normalizado.
6. Testar execução contínua versus blocos arbitrários e verificar igualdade bit a bit do estado e da sequência produzida.

**Exportação de referência para comparação histórica:**

O exemplo `native/sn76489/examples/psg_trace.rs` gera WAV IEEE-float 32-bit mono a 48 kHz a partir de uma sequência fixa de escritas em registradores e clocks. O hash FNV-1a impresso é calculado sobre as amostras `f64` internas antes da conversão WAV; é um identificador de reprodução, não uma assinatura criptográfica.

Execute a mesma fonte do exemplo na revisão histórica e na branch atual:

```powershell
git worktree add ../sega-audio-baseline f47f71830344b85f72329376505d400bfbfaae69
New-Item -ItemType Directory -Force ../sega-audio-baseline/native/sn76489/examples
Copy-Item native/sn76489/examples/psg_trace.rs ../sega-audio-baseline/native/sn76489/examples/psg_trace.rs

cargo run --release --manifest-path ../sega-audio-baseline/Cargo.toml -p sega-sn76489 --example psg_trace -- sms baseline-sms.wav
cargo run --release -p sega-sn76489 --example psg_trace -- sms current-sms.wav
cargo run --release --manifest-path ../sega-audio-baseline/Cargo.toml -p sega-sn76489 --example psg_trace -- sg1000 baseline-sg1000.wav
cargo run --release -p sega-sn76489 --example psg_trace -- sg1000 current-sg1000.wav

# Controle de invariância: segmenta o avanço do Scheduler em blocos de 997 clocks.
cargo run --release -p sega-sn76489 --example psg_trace -- sms current-sms-chunked.wav 997
```

O WAV contém a fonte mono crua do PSG, sem dual-mono, volume de host, bass management ou quantização para 24 bits. Assim, a comparação isola o núcleo e seu filtro de reconstrução. A conversão final para WAV float32 serve apenas para inspeção/escuta; os valores e o hash impressos são calculados sobre as amostras `f64`.

A validação `psg-focused` da CI executa automaticamente a captura histórica e atual, compara SMS/SG-1000 e a invariância contínua/segmentada, e publica o artefato `psg-audio-ab-reference` com WAVs e relatórios JSON/Markdown.

**Captura integral A/B:** workflow [#43260895841](https://github.com/leonardo201800478/sega-emulator/actions/runs/43260895841) concluído com sucesso; artefato [psg-audio-ab-reference](https://github.com/leonardo201800478/sega-emulator/actions/runs/43260895841/artifacts/13962279974). O exportador agora drena o buffer a cada avanço de até 16.384 clocks e exige a contagem integral esperada de amostras.

| Perfil | Amostras | Duração | Baseline RMS | Atual RMS | Variação RMS | NRMSE | Correlação |
|---|---:|---:|---:|---:|---:|---:|---:|
| SMS | 214.552 | 4,4698 s | 0.199967213421 | 0.199879217416 | -0,0440% | 3,9032% | 0.999235 |
| SG-1000 | 214.552 | 4,4698 s | 0.199547399558 | 0.199470901369 | -0,0383% | 3,8742% | 0.999247 |

Pico: SMS 0.512066 → 0.510672 (-0,2723%); SG-1000 0.512677 → 0.508157 (-0,8818%). Erro absoluto máximo: 0.108459 (SMS) e 0.108442 (SG-1000). O PCM atual contínuo e segmentado em blocos de 997 clocks é **idêntico amostra a amostra** nos dois perfis: erro RMS e erro máximo zero.

**Delta de energia espectral (atual em relação ao baseline, FFT radix-2 com janela Hann):**

| Banda | SMS | SG-1000 |
|---|---:|---:|
| 20–200 Hz | -0,000 dB | -0,000 dB |
| 200 Hz–1 kHz | -0,001 dB | +0,001 dB |
| 1–4 kHz | -0,001 dB | -0,002 dB |
| 4–8 kHz | -0,019 dB | -0,015 dB |
| 8–12 kHz | -0,042 dB | -0,040 dB |
| 12–16 kHz | -0,146 dB | -0,140 dB |
| 16–20 kHz | -0,150 dB | -0,151 dB |
| 20–24 kHz | -0,498 dB | -0,497 dB |

O centróide diminuiu 5,874 Hz no SMS e 5,804 Hz no SG-1000. A mudança concentra-se nas bandas altas; abaixo de 4 kHz a diferença é praticamente nula. RMS global e níveis médios permanecem muito próximos. Isso aponta para alteração na reconstrução/forma de onda, não para perda geral de volume.

**Varredura de tons isolados:** oito períodos, canal 0 ativo e demais canais silenciados. A energia fundamental atual em relação ao baseline foi:
- período 5 (22,372 kHz): -0,1434 dB;
- período 6 (18,643 kHz): -0,0994 dB;
- período 7 (15,980 kHz): -0,0730 dB;
- período 9 (12,429 kHz): -0,0442 dB;
- período 14 (7,990 kHz): -0,0182 dB;
- período 28 (3,995 kHz): -0,0046 dB;
- período 56 (1,998 kHz): -0,0011 dB;
- período 112 (999 Hz): -0,0003 dB.

A atenuação adicional cresce com a frequência e é pequena até 8 kHz. Isso confirma a diferença de resposta do interpolador linear, mas ainda não estabelece inferioridade perceptiva.

**Experimento Lagrange4 (somente testes, não produção):** o protótipo causal de quatro pontos recupera energia em relação à interpolação linear, sobretudo nas frequências altas:

| Período | RMS linear | RMS Lagrange4 | Energia fundamental Lagrange4 vs. linear |
|---:|---:|---:|---:|
| 5 | 0.074326145 | 0.077072407 | +0,3162 dB |
| 6 | 0.222515651 | 0.228053146 | +0,2140 dB |
| 7 | 0.223195658 | 0.227192489 | +0,1545 dB |
| 9 | 0.223936962 | 0.226306045 | +0,0915 dB |
| 14 | 0.224603249 | 0.225562303 | +0,0370 dB |
| 28 | 0.241155099 | 0.241843331 | +0,0091 dB |
| 56 | 0.245019625 | 0.245317669 | +0,0023 dB |
| 112 | 0.247590748 | 0.247742408 | +0,0006 dB |

O protótipo foi finito, permaneceu abaixo do limite de pico do teste e foi invariável ao particionamento em blocos de 997 clocks no período 5. Ele ultrapassa ligeiramente o nível fundamental do baseline nas frequências altas; portanto, **não deve ser promovido à produção ainda**. Falta avaliar THD/harmônicos, ringing, transientes de escrita e comportamento de ruído.

**THD e harmônicos:** a análise de tons mostra que a interpolação linear reduz ligeiramente a energia harmônica em relação ao baseline; Lagrange4 recupera essa energia, mas ultrapassa o baseline nos períodos baixos:
| Período | THD baseline | THD linear | THD Lagrange4 |
|---:|---:|---:|---:|
| 14 | 0,0079% | 0,0052% | 0,0054% |
| 28 | 38,8751% | 38,6266% | 39,1592% |
| 56 | 43,1834% | 43,0583% | 43,3206% |
| 112 | 45,9140% | 45,8449% | 45,9893% |

Isso indica que Lagrange4 não é uma correção neutra: restaura harmônicos, mas também altera o balanço harmônico e excede o baseline em parte da faixa. Nenhum interpolador foi promovido à produção.

**Transientes do trace completo:** as janelas de 2 ms antes/depois e 5–10 ms após as escritas mostram RMS assentado praticamente igual. Entretanto, no SMS, o delta entre amostras adjacentes caiu de 0.08157 para 0.07013 no reset do ruído (-14,0%) e de 0.05224 para 0.04964 na mudança do período do tom (-5,0%). É evidência de bordas mais suaves na versão linear, não prova isolada de ringing nem de inferioridade auditiva.

**Integração temporal isolada:** o teste `scheduled_psg_matches_direct_core_for_identical_register_timestamps` dirige `ScheduledPsg` através de `MachineScheduler` com os mesmos timestamps de escrita do trace e compara com avanço direto do SN76489. PCM completo e estado final são idênticos. CI [#43262267964](https://github.com/leonardo201800478/sega-emulator/actions/runs/43262267964) passou.

A comparação de implementação entre `f47f718` e a revisão atual confirmou que `tick_internal_clock`, `push_filter_sample`, `filtered_output`, `mix_output`, `write_inner`, `update_noise`, `clock_tone` e `design_lowpass` permanecem idênticos. A diferença funcional identificada no core está em `advance_clocks_inner`, no instante de reconstrução/interpolação das amostras. Isso, junto com o teste ScheduledPsg/direct, concentra a investigação no método de reconstrução, não na semântica digital do chip nem no wrapper do Scheduler.

**Correção metodológica:** os relatórios antigos [#43257745528](https://github.com/leonardo201800478/sega-emulator/actions/runs/43257745528) e [#43258136796](https://github.com/leonardo201800478/sega-emulator/actions/runs/43258136796) capturavam apenas a janela final de um segundo e ficam substituídos pela captura integral acima.

**Gate H1 — CONCLUÍDO:** trace determinístico, exportação PCM integral, comparação histórica SMS/SG-1000, métricas de forma de onda/espectro e invariância contínua/segmentada foram executados. A CI da revisão [#43260895841](https://github.com/leonardo201800478/sega-emulator/actions/runs/43260895841) passou em PSG, Clippy, formatação, regressão Rust, documentação e validação de workflows.

### H2 — Fechamento da síntese e reconstrução

**Status: EM VALIDAÇÃO — protótipo event-based, sweep de períodos, transientes e perfis de ruído adicionados em módulo exclusivamente de teste. A conclusão depende dos novos artefatos e da CI.**

1. [x] Auditar divisor, fase de amostragem, FIR e diferenças de resposta.
2. [x] Validar a interpolação fracionária com o core real e comparar baseline, linear e protótipo Lagrange4.
3. [ ] Medir THD/harmônicos, ringing, transientes de escrita de tom/volume/ruído e limites de período 0/1.
4. [ ] Comparar, com os mesmos traces, o FIR atual com um protótipo independente de síntese por eventos/Blip Buffer.
5. [ ] Usar Gearsystem e ARES como referências comparativas, não como oráculos absolutos.
6. [ ] Revisar licenças antes de qualquer reutilização de código; preferir implementação independente da arquitetura estudada.

**Gate H2:** resultado objetivo melhor ou equivalente ao baseline, sem mudança não fundamentada da semântica do PSG.

### H3 — Linha de áudio de referência em 48 kHz

1. Definir 48 kHz como taxa interna de produção de áudio, independente da taxa física do endpoint.
2. Generalizar o `PcmResampler` para taxa de entrada explícita, sem constante global implícita de 44,1 kHz.
3. Preservar determinismo, contagem de frames, flush e invariância por tamanho de bloco.
4. Evitar resampling redundante quando a taxa de saída negociada também for 48 kHz.
5. Manter `f64` até a fronteira final de conversão.

**Gate H3:** 48 kHz end-to-end em testes headless; equivalência entre blocos; sem alteração de clocks do Scheduler.

### H4 — Contrato PCM moderno e saída 24-bit

1. Substituir o contrato implícito PCM16 de `PcmSink` por frames/amostras com formato explícito, sem alocações no callback físico.
2. Manter PCM em precisão elevada até a saída; aplicar volume e saturação uma única vez na fronteira correta.
3. Negociar formato com o dispositivo: I24 quando suportado; F32 como alternativa de alta precisão; fallback documentado quando necessário.
4. Confirmar a semântica de 24 bits de CPAL: `I24/U24` usam armazenamento de 4 bytes, com 24 bits significativos.
5. Não assumir que todo endpoint suporta I24 ou 48 kHz.
6. Atualizar contadores/telemetria para refletir taxa, canais, formato e bits efetivamente negociados.

**Gate H4:** testes de conversão, saturação, silêncio, dual-mono, fallback e callback sem locks/alocação; CI Windows x64.

### H5 — Integração CPAL e experiência de saída

1. Coordenar com a PR #770 sem misturar a migração do backend com mudanças do núcleo SN76489.
2. Manter callback CPAL limitado à leitura da FIFO e conversão/cópia previamente preparada.
3. Validar pause/stop/reset/load/epoch, underrun, overflow, encerramento e reconexão.
4. Mostrar formato realmente negociado; não exibir “24-bit” se o endpoint estiver em F32 ou 16 bits sem explicar a representação.
5. Manter saída estéreo dual-mono para PSG SMS/SG-1000.

**Gate H5:** CI aplicável verde e testes físicos documentados. Nenhum merge automático.

### H6 — Validação física e aceite auditivo

- Testes prolongados em Windows x64 com saída 48 kHz estéreo.
- Testar endpoints I24 e F32 quando disponíveis, e fallback para 44,1 kHz.
- Medir underruns, drops, latência de fila e estabilidade; não confundir ocupação FIFO com latência ponta a ponta.
- Ouvir os mesmos traces/ROMs de referência no emulador, Gearsystem e ARES, com volume e cadeia de áudio equalizados.
- Confirmar ausência de clipping, ruído digital, estalos, canais desequilibrados e perda de transientes.
- Validar em caixas estéreo e sistema 2.1 com bass management externo.

**Gate H6:** relatório com dados, hardware, configurações, WAVs e resultados; sem alegar teste físico que não tenha sido executado.

## 5. Critérios de aceite globais

- [ ] Identidade sonora SMS e SG-1000 preservada.
- [ ] 48 kHz estéreo como perfil preferencial.
- [ ] 24 bits efetivos quando I24 estiver disponível; fallback transparente.
- [ ] Nenhuma redução para PCM16 antes da fronteira final.
- [ ] Nenhum LFE artificial ou processamento tonal obrigatório.
- [ ] Scheduler, CPU, VDP e barramentos independentes da saída física.
- [ ] Execução contínua e segmentada determinísticas.
- [ ] Sem clipping, underruns ou drops não contabilizados.
- [ ] Testes headless, CI e testes físicos classificados separadamente.

## 6. Riscos e decisões em aberto

| Risco/decisão | Tratamento |
|---|---|
| Mudança do source rate 44,1 → 48 kHz altera WAVs e save-state de áudio | Fazer como mudança explícita de versão/configuração e testar snapshots |
| PR #770 modifica `audio_output` e `session.rs` | Não editar os mesmos arquivos em branches paralelas sem coordenar o contrato |
| I24 não suportado em todos os drivers | Negociação por capacidades e fallback explícito |
| F32 não é PCM inteiro de 24 bits | Reportar o formato real; não rotular F32 como I24 |
| Blip Buffer pode mudar a resposta transitória | Comparação A/B antes de adoção |
| 24 bits não corrigem uma síntese ruim | Corrigir primeiro fonte, filtros e resampling |

## 7. Referências técnicas

- [CPAL 0.18.2 — documentação](https://docs.rs/cpal/0.18.2/cpal/)
- [CPAL — formatos I24/U24 e armazenamento](https://docs.rs/cpal/latest/src/cpal/sample_format.rs.html)
- [Gearsystem — Sms_Apu](https://github.com/drhelius/Gearsystem/blob/1be9a12b82cae84ddd6f9b7006d546abe2d58bd4/src/audio/Sms_Apu.cpp)
- [Gearsystem — Blip_Buffer](https://github.com/drhelius/Gearsystem/blob/1be9a12b82cae84ddd6f9b7006d546abe2d58bd4/src/audio/Blip_Buffer.h)
- [SMS Power](https://www.smspower.org/)

---
**Estado desta revisão:** planejamento e auditoria estática documentados. Não há alteração de algoritmo de áudio nesta etapa. Testes locais e validação física permanecem pendentes.


Consulte também o estudo PSG H2 — reconstrução event-based, com a modelagem independente, os testes matemáticos, a varredura de períodos e a caracterização dos perfis de ruído.
