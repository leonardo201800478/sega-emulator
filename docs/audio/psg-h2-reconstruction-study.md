# Estudo de reconstrução PSG — H2

**Escopo:** SN76489 compartilhado por SG-1000 e Master System.
**Implementação:** protótipo exclusivamente de teste; nenhuma mudança na síntese de produção.
**Referência histórica:** f47f71830344b85f72329376505d400bfbfaae69.
**Baseline de integração:** b375d4bb85149678b82ab39a497177d1a1cd2319.

## 1. Hipótese

A interpolação linear entre duas saídas FIR atenua energia crescente nas bandas altas e suaviza transições que o baseline histórico preservava. A hipótese H2 é que a reconstrução das transições digitais por eventos com timestamp subamostral e resposta de passo band-limited reduz esse erro sem alterar clock, contadores, registradores ou LFSR do chip.

O protótipo recebe eventos (clock, delta_amplitude). O timestamp é convertido para a posição fracionária na linha de saída:

\[
q = \frac{t_{PSG}\,f_s}{f_{clock}}
\]

A saída é reconstruída pela soma das respostas de passo deslocadas:

\[
y[n] = y_0 + \sum_k \Delta A_k\,S(n+1-q_k-G)
\]

O termo n+1 representa a convenção da saída PSG: a primeira amostra é emitida no primeiro cruzamento do acumulador de amostragem, não no instante inicial da máquina. O protótipo mantém a fase fracionária explicitamente e não altera o MachineScheduler.

## 2. Kernel experimental

| Parâmetro | Valor |
|---|---:|
| Taxa de saída | 48.000 Hz |
| Janela | Kaiser |
| Beta | 7,85726 |
| Largura total | 65 taps equivalentes |
| Meia largura | 32 amostras |
| Oversampling da tabela integrada | 1.024 fases por amostra |
| Cutoff | 0,46 ciclos/amostra |
| Compensação fracionária experimental | -0,75 amostra |
| Atraso de grupo do protótipo | 31,25 amostras |

A resposta de passo é obtida integrando numericamente o kernel sinc limitado por janela. O protótipo é independente do código Gearsystem/Blip_Buffer; somente os princípios de timestamp, delta de amplitude e reconstrução band-limited são usados como referência conceitual.

A compensação fracionária é um parâmetro experimental de alinhamento com o sample grid histórico, não uma propriedade do SN76489. Deve permanecer explícita nos relatórios e não pode ser confundida com clock de hardware.

## 3. Gates matemáticos e temporais

- Varredura efetiva por eventos de todos os períodos válidos 2–1023, com WAV checkpoints e relatório JSON de RMS, pico, energia fundamental no alias e ganho do kernel.
- Períodos 0 e 1 preservados como casos especiais de saída constante.
- Ganho DC unitário e resposta finita.
- Resposta de passo com overshoot, undershoot e settling medidos.
- Resposta contínua avaliada no limite de banda passante e em Nyquist.
- Eventos com timestamp inteiro em clocks PSG e posição subamostral racional.
- Captura segmentada e contínua comparadas bit a bit com limites de bloco 1, 7, 997 e 16.384 clocks.
- Eventos simultâneos são coalescidos por timestamp com soma canônica, independente da ordem de escritas no mesmo clock.
- Fronteiras exatas de amostragem e eventos a ±1 clock são testados (715.909 PSG clocks = 9.600 amostras a 48 kHz).
- Protótipo isolado do caminho de produção.

## 4. Transientes

A captura utiliza os mesmos eventos de registradores da referência A/B: alteração de volume, mudança do divisor de ruído, mudança de período, alteração de volume de outro canal, reset do LFSR e mudança final de volume do ruído.

O relatório compara janelas pré-evento, início, região assentada, pico e diferença entre amostras adjacentes. A resposta de passo do kernel também é medida independentemente, para que ringing do kernel não seja confundido com transientes específicos do PSG.

## 5. Tons e aliasing

O teste percorre integralmente os períodos 2–1023. WAVs de checkpoints são exportados para períodos 2, 3, 4, 5, 6, 7, 9, 14, 28, 56, 112, 255, 512 e 1023.

Os períodos 0 e 1 são testados separadamente e não são tratados como frequências oscilantes. A frequência teórica para período válido p é:

\[
f = \frac{f_{clock}}{32p}
\]

A frequência acima de Nyquist é dobrada para a banda observável antes da análise da resposta de reconstrução.

## 6. Ruído e LFSR

| Perfil | Largura | White taps | Período branco | Período periódico |
|---|---:|---|---:|---:|
| SMS | 16 bits | 0 e 3 | 57.337 | 16 |
| SG-1000 | 15 bits | 0 e 1 | 32.767 | 15 |

Os testes verificam estados, contagem de bits, retorno ao seed e o ciclo periódico. Capturas WAV separadas cobrem ruído periódico e branco para cada perfil. O trace integral também cobre a seleção de ruído derivada do terceiro tom e reset durante a execução.

A periodicidade SMS de 57.337 é característica documentada no SMS Power; não deve ser substituída por um LFSR maximal genérico de 65.535 estados.

## 7. Gearsystem como referência conceitual

Na revisão de referência indicada, Gearsystem utiliza timestamps de transição e deltas de amplitude com Blip_Buffer. O cabeçalho define qualidade 12 como boa qualidade, qualidade 8 como média e qualidade 16 como alta. O mecanismo de offset_resampled distribui a contribuição do evento em múltiplos taps conforme a fase subamostral.

O estudo atual implementa um modelo independente, com kernel integrado em ponto flutuante e parâmetros próprios. Nenhum código Gearsystem é incorporado. A referência é conceitual, não um oráculo absoluto.

## 8. Decisão arquitetural

O protótipo event-based é candidato técnico para uma etapa posterior de produção, não uma substituição aprovada. A síntese atual permanece intacta até que os artefatos CI sejam inspecionados, os resultados por perfil e período sejam aceitos, a resposta de ruído e transientes seja aprovada e exista avaliação auditiva A/B controlada.

H2 fecha a investigação e a seleção experimental. A promoção do candidato à síntese de produção não é parte desta alteração.

## 9. Custo computacional e adequação a streaming

A CI registra, separadamente para SMS e SG-1000, o tempo de captura de eventos, reconstrução em lote e renderização pelo core atual. O relatório estima a memória mantida pelo vetor de eventos, kernel e buffers temporários completos.

O protótipo atual é deliberadamente offline: armazena a lista completa de eventos e aloca buffers para toda a duração. Não é adequado para callback de áudio nem deve ser promovido diretamente para produção. A resposta simétrica de 65 taps requer lookahead causal aproximado de 32 frames (0,667 ms a 48 kHz). Uma implementação de produção precisaria de fila de eventos limitada e acumulador circular, mantendo o callback CPAL sem locks e sem alocações.

Os tempos de CI são medições de referência, não gates rígidos de performance; variam com runner e carga. A comparação de fator realtime não substitui medição física nem prova de latência ponta a ponta.


## 10. Resultados reproduzíveis da CI — 2026-11-12

**Execução:** [CI #43289661184](https://github.com/leonardo201800478/sega-emulator/actions/runs/43289661184), concluída com sucesso.  
**SMS Debugger Runtime:** [#43289661057](https://github.com/leonardo201800478/sega-emulator/actions/runs/43289661057), concluído com sucesso.  
**HEAD validada:** `4dc0a906da1f5795c3078e59fb627ed2fda71f00`.  
**Artefato:** `psg-audio-ab-reference`, ID `13977310503`, SHA-256 `ffaa58a1ffa3bf467a585cafdc3eb636f30c61b7712a7b08778ff9d5b0fafd4a`.

### 10.1 Comparação integral dos traces

| Perfil | Baseline × atual NRMSE | Baseline × evento NRMSE | Atual × evento NRMSE | Correlação baseline × evento |
|---|---:|---:|---:|---:|
| SMS | 3,903223% | 3,066105% | 5,704215% | 0,999528123 |
| SG-1000 | 3,874234% | 3,042106% | 5,660919% | 0,999535897 |

O NRMSE menor do evento em relação ao baseline, comparado ao atual, indica maior proximidade amostral no trace integral específico. Isso não constitui prova de fidelidade elétrica, preferência auditiva ou superioridade universal. A comparação é dependente do trace e do alinhamento temporal adotado.

### 10.2 Varredura e resposta do kernel

- Foram cobertos todos os 1.022 períodos válidos, de 2 a 1023; períodos 0 e 1 foram testados separadamente como saídas constantes.
- Maior ganho de stopband nos períodos 2–4: 0,000015671835.
- Ganho de transição no período 5: 0,333425223.
- Menor ganho de passband nos períodos 6–1023: 0,999969259.
- Kernel: 65 taps, Kaiser beta 7,85726, 1.024 fases, cutoff 0,46 ciclos/amostra.
- Atraso de grupo: 31,25 amostras; lookahead causal estimado: 32 frames (0,666667 ms).
- Overshoot: +8,905564%; undershoot: -8,905564%; settling a 0,1%: 51,785 amostras.

O ringing de aproximadamente 8,9% é mensurável e deve ser tratado como compromisso do kernel, não como comportamento automaticamente desejável. Os resultados de tons isolados também mostram NRMSE elevado nos períodos próximos/acima da banda útil, em parte por normalização sobre sinais de amplitude muito baixa; esses pontos não devem ser resumidos pelo NRMSE sem RMS, ganho do kernel e frequência de alias.

### 10.3 Ruído

| Perfil | Modo | NRMSE atual × evento | Centróide do evento | Maior autocorrelação de curto atraso |
|---|---|---:|---:|---:|
| SMS | Periódico | 4,595224% | 1.085,783 Hz | 0,884296 |
| SMS | Branco | 9,273552% | 2.592,077 Hz | 0,891960 |
| SG-1000 | Periódico | 4,754251% | 1.143,134 Hz | 0,883710 |
| SG-1000 | Branco | 9,065867% | 2.538,507 Hz | 0,897005 |

A diferença no ruído branco é material e permanece como limitação aberta para qualquer proposta de adoção em produção. Os testes determinísticos verificam a sequência interna do LFSR, seed, taps, periodicidade e reinício; eles não provam equivalência acústica entre os renderizadores.

### 10.4 Custo e restrição de execução

| Perfil | Captura | Render evento | Core atual | Memória estimada |
|---|---:|---:|---:|---:|
| SMS | 12,990 ms | 36,228 ms | 508,033 ms | 6.612.224 bytes |
| SG-1000 | 12,248 ms | 30,322 ms | 508,099 ms | 6.608.352 bytes |

O fator realtime do pipeline em lote foi 0,011011 (SMS) e 0,009524 (SG-1000), para traces de 4,469833 s. A medição é offline e não representa latência de streaming. O protótipo mantém todos os eventos e buffers de duração integral; `streaming_ready=false`. Não pode ser ligado ao callback CPAL.

### 10.5 Gate e decisão

**H2 — investigação e caracterização experimental: concluído.** A CI, os WAVs e os relatórios reproduzíveis estão preservados. O módulo continua exclusivamente de teste. A síntese de produção, o Scheduler e o callback CPAL não foram modificados.

**H2 — promoção do reconstruidor para produção: NÃO APROVADA.** A divergência mensurável no ruído branco, o ringing do kernel, os resultados de tons de baixa amplitude e a ausência de avaliação auditiva A/B controlada impedem a promoção. Qualquer adoção futura exige desenho streaming bounded-memory, análise adicional do ruído/transientes, validação física e aceite auditivo separado.
