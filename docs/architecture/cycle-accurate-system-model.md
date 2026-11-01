# Modelo temporal cycle-accurate — arquitetura-alvo do Sega Emulator

**Estado:** decisão arquitetural aprovada para orientar novas implementações; contratos detalhados ainda sujeitos a evidência de hardware.  
**Escopo inicial:** SG-1000 e Master System, com SMS como primeiro alvo de integração temporal do Z80/VDP.  
**Não significa:** implementação concluída, validação física concluída ou `TEMPORAL_PASS`.

## 1. Decisão

O Sega Emulator adotará execução digital orientada a eventos, com precisão temporal cycle-accurate como requisito de arquitetura. CPU, barramento, VDP, PSG, FM, memória, I/O, interrupções e DMA (quando existentes) compartilharão uma única timeline por instância de máquina, pertencente ao runtime Rust.

A unidade de implementação não será obrigatoriamente uma chamada por tick mestre ou por pixel. Cada componente poderá processar intervalos internamente, desde que:
- nenhum evento observável seja atravessado ou omitido;
- todos os efeitos sejam aplicados no timestamp correto;
- dependências entre dispositivos sejam sincronizadas antes de leituras, escritas e saídas;
- a granularidade agrupada seja demonstravelmente equivalente ao processamento detalhado para o comportamento coberto.

O scheduler deve conhecer e despachar as próximas bordas/eventos relevantes. Não será permitido avançar CPU, VDP e PSG em blocos independentes e reconciliar seus estados posteriormente.

## 2. Vocabulário e níveis de precisão

- **Instruction-accurate:** estado arquitetural após cada instrução está correto.
- **T-state accurate:** cada T-state e suas fases relevantes são representados corretamente.
- **M-cycle accurate:** ciclos de máquina e transações associadas estão ordenados e temporizados.
- **Bus-cycle accurate:** endereço, dados, sinais e fases de acesso ao barramento correspondem ao hardware.
- **Raster/dot accurate:** eventos internos do vídeo e a produção de pixels são posicionados na fase horizontal/vertical correta.
- **Cycle-accurate (claim do projeto):** conjunto integrado de CPU, barramento, vídeo, áudio digital, interrupções e arbitragem reproduz as relações temporais observáveis do perfil de hardware, com traces e oráculos independentes.

Um componente pode atingir um nível sem que a máquina inteira o atinja. A documentação e telemetria devem sempre identificar o escopo e o nível demonstrados. Contagem agregada de ciclos, frame pacing estável ou timestamps artificiais não provam cycle accuracy.

## 3. Invariantes não negociáveis

1. **Autoridade temporal única:** um scheduler por máquina; nenhum scheduler independente por chip.
2. **Tempo virtual determinístico:** o estado emulado não depende de wall-clock, FPS, VSync, áudio do host, threads do SO ou velocidade da GUI.
3. **Aritmética exata:** inteiros/racionais checked; ponto flutuante não é fonte de verdade para clocks, fases, bordas ou ordenação.
4. **Causalidade:** um evento não pode modificar o passado nem inserir retroativamente uma prioridade anterior no mesmo instante já despachado.
5. **Transações temporais:** toda operação observável de CPU/barramento possui instante de início, fases/duração e conclusão.
6. **Espera explícita:** WAIT, contenção, arbitragem e stalls estendem/alteram a operação segundo regras do hardware; não são adicionados como correção posterior.
7. **Sem execução retrospectiva:** um dispositivo não pode descobrir depois de avançar que uma operação deveria ter ocorrido no meio do intervalo.
8. **Agrupamento seguro:** processar N pixels/ciclos juntos é permitido somente se não houver evento pendente ou interação capaz de observar os estados intermediários.
9. **Determinismo de desempate:** simultaneidade é resolvida por fases/prioridades específicas do perfil e sequência estável, documentadas e testadas.
10. **Snapshot completo:** salvar/restaurar inclui fases dos clocks, eventos pendentes, micro-op em curso, transação de barramento, latches, pipelines e estado dos dispositivos.
11. **Sem falsa precisão:** todo comportamento não confirmado permanece rotulado como nominal, aproximado, hipótese ou desconhecido.
12. **Sem regressão silenciosa:** golden masters preservam comportamento anterior; correções semânticas exigem justificativa e oráculo independente.

## 4. Modelo de tempo

### 4.1 MachineTime

`MachineTime` representa tempo virtual absoluto e monotônico da máquina. A escala deve ser escolhida por perfil de hardware e precisa representar exatamente as relações de clock, fases e eventos requeridos.

Não fixar a unidade em "um tick do clock mestre" antes de provar que ela representa todas as bordas e fases necessárias. Uma opção é uma base inteira refinada; outra é uma representação racional normalizada. A decisão de escala será um ADR técnico após inventário dos clocks e fases SMS/SG.

Não usar `f32`/`f64` para converter ciclos em timestamps.

### 4.2 Clock domains e bordas

Um `ClockDomain` representa uma fonte/divisor/fase logicamente relevante. Componentes podem compartilhar domínio somente quando fonte, divisor e fase forem comprovadamente comuns.

O contrato deve permitir:
- consultar a próxima borda local em tempo mestre;
- consumir uma borda individual sem perder fase;
- avançar até timestamp limite sem atravessar eventos externos;
- preservar fase fracionária ao dividir o avanço em blocos;
- restaurar exatamente o estado após save state.

A API atual de avanço agregado de `ClockDomain` continua útil para conversões e validação chunk-invariant, mas não substitui a API de próxima borda necessária à integração cycle-accurate.

### 4.3 Scheduler orientado a eventos

O scheduler seleciona o menor timestamp entre:
- próxima borda de clock necessária;
- próximo evento interno anunciado por dispositivo;
- conclusão/fase de transação de barramento;
- interrupção, DMA, mudança de latch ou outro evento externo.

Uma implementação híbrida é preferida: bordas são representáveis individualmente, mas dispositivos podem anunciar o próximo instante em que seu estado se torna observável. Não criar um evento global para cada tick/pixel quando não houver dependência.

A fila deve ordenar por:
`(timestamp, hardware_phase, platform_priority, stable_sequence)`

A semântica de `hardware_phase` e `platform_priority` não é universal. Deve ser definida por perfil e respaldada por documentação, medição ou testes de hardware. Eventos delta no mesmo timestamp precisam de limite contra ciclos causais infinitos e regras de inserção explícitas.

### 4.4 Eventos simultâneos

Não presumir CPU antes do VDP nem VDP antes da CPU. Definir por situação:
- quando uma escrita da CPU é amostrada;
- quando o VDP faz fetch de VRAM;
- quando uma borda de IRQ é gerada/observada;
- quando uma mudança de registro/CRAM afeta o pixel de saída;
- quando WAIT/contenda bloqueia ou desloca uma transação.

Quando fontes não permitirem determinar a ordem, registrar a ambiguidade e isolar a hipótese em configuração/teste; não escondê-la em um número arbitrário de prioridade.

## 5. CPU Z80 e micro-operações

A CPU compartilhada deve manter separação entre:
- estado arquitetural (registradores, flags, PC, SP, I, R, IFF, IM, HALT etc.);
- decoder/semântica de opcode;
- sequenciador temporal de micro-operações;
- interface de transação com barramento;
- observação/trace opcional.

Cada instrução expande para sequência determinística de micro-operações: fetch M1, leitura/escrita, I/O, refresh, operações internas, acknowledge, stack e espera. O sequenciador precisa suspender/resumir em fronteiras temporais sem aplicar resultados prematuramente.

Regras:
- M-cycle e T-state são conceitos explícitos, não apenas um total final;
- endereços/dados/sinais permanecem opcionais quando o barramento não os define;
- refresh/RFSH/M1/WAIT devem ser representados quando o perfil e a evidência os exigirem;
- interrupções podem ser amostradas nos instantes documentados, não apenas ao final de uma instrução;
- prefixos, instruções repetidas, HALT e ciclos de interrupção devem gerar traces completos;
- o stepping por instrução será ferramenta de debug sobre o mesmo sequenciador, nunca outro executor de produção.

O contrato de trace Z80 existente é uma base parcial. Cobertura de oito instruções ED não deve ser generalizada para todo o opcode space.

## 6. Barramento e transações

A interface de barramento não pode ser apenas `read(addr) -> value`/ `write(addr, value)` sem tempo.

Uma transação deve modelar, conforme necessário:
- tipo: opcode fetch, memória read/write, I/O read/write, interrupt acknowledge, refresh;
- endereço e dados opcionais;
- sinais/fases e instante de amostragem;
- início, duração e conclusão;
- WAIT/stall/contenda;
- dispositivo selecionado e efeitos colaterais;
- sequência/identidade para trace e replay.

CPU solicita uma transação; máquina/scheduler coordena dispositivos e arbitragem; resposta é disponibilizada no instante correto. O barramento não deve possuir relógio autônomo nem avançar hardware fora do scheduler.

## 7. VDP e raster

O VDP SMS terá estado temporal explícito: gerador H/V, fase de linha, blanking, fetch pipeline, sprite evaluation, pixel pipeline, registradores/latches, VRAM/CRAM, status e IRQ.

O VDP pode processar grupos de pixels internamente quando provar que não há write, fetch, latch, IRQ ou observador entre eles. Uma escrita de CPU em meio à linha deve sincronizar o VDP até o timestamp da transação antes de aplicar o efeito.

A apresentação do framebuffer é uma saída; não é o modelo de hardware. O renderer do host nunca determina quando o VDP avança.

## 8. PSG/FM e áudio digital

PSG/FM evoluem em tempo emulado. Divisores, contadores, ruído, latches, envelopes e mudanças digitais de saída devem ser atualizados nos instantes de hardware relevantes. O resampler/filtro de saída do host é separado do estado digital do chip.

A produção de PCM pode ser em blocos, desde que cada transição digital e integração de amostra preserve a cronologia. O callback WASAPI/CPAL consome buffers; não avança o scheduler.

## 9. Threads e runtime

Uma thread de emulação é proprietária da máquina e do scheduler. Não criar uma thread por chip. Workers de host podem capturar input, consumir PCM e apresentar vídeo, mas não mutam o estado digital.

CLI e PyO3 são fachadas sobre a mesma Session/Runtime API. Python/Qt não possui `step`, `run_frame`, `advance_cycles`, clocks, IRQ, buffers PCM ou framebuffer mutável. Comandos de controle são coarse-grained e aplicados em safe points definidos.

## 10. Save states, replay e trace

Save state deve serializar:
- MachineTime e política/versão de tempo;
- clock domains e fases;
- fila de eventos com payloads e sequência;
- fase de despacho/eventos delta;
- micro-op atual e estado interno da CPU;
- transação de barramento em voo;
- estado temporal de VDP/PSG/FM e latches;
- RAM/VRAM/CRAM/mapper/I/O;
- sequência de inputs normalizados pendentes.

Traces são opcionais e não podem mudar a execução quando desativados. Devem permitir diff da primeira divergência com timestamp, dispositivo, fase, sinais/endereço/dados e causa.

## 11. Estratégia de implementação

- **T0 — evidência e contratos:** inventário de fontes, clocks, fases, diagramas e lacunas; congelar regressões.
- **T1 — temporal primitives:** MachineTime, clock edge/next-edge, fase, fila determinística, causalidade e snapshots.
- **T2 — bus transactions:** transações temporais, waits, trace e testes unitários sem alterar execução legada.
- **T3 — Z80 micro-op sequencer:** migrar progressivamente famílias de opcode e provar traces.
- **T4 — SMS VDP temporal core:** raster/fetch/sprite/pixel pipeline com estado explícito, mantendo API de saída compatível.
- **T5 — integração CPU/VDP/bus:** concorrência VRAM, writes raster, IRQ e ordenação simultânea.
- **T6 — PSG/FM e áudio digital:** integrar transições e geração de amostras ao mesmo tempo.
- **T7 — integração de máquina:** replay, save state, CLI/headless, determinismo SG-1000/SMS.
- **T8 — validação:** comparação com oráculos independentes e hardware quando disponível; performance baseline e regressão.

A ordem pode ter trilhas paralelas de pesquisa/testes, mas a integração temporal depende dos contratos e evidências anteriores. Não integrar CPU/VDP ao scheduler enquanto este só retornar ciclos agregados.

## 12. Critérios de aceitação e estados

- **IMPLEMENTED:** código existe, contrato e limites documentados.
- **QUICK_PASS:** testes unitários focados passaram no SHA identificado.
- **SHORT_PASS:** conjunto de testes funcionais/temporais curtos passou.
- **LONG_PASS:** execução longa e stress repetível passou.
- **INTEGRATION_PENDING:** integração de sistema ou host não validada.
- **TEMPORAL_PASS:** traces integrados, por timestamp/fase, comparados com oráculos adequados; divergências explicadas; determinismo e regressões aprovados.

TEMPORAL_PASS exige SHA exato, manifest de evidência, versões/configuração, oráculos com ancestralidade e logs CI. Ausência de falha não é evidência de precisão.

## 13. Performance

O caminho crítico deve usar estruturas compactas, memória pré-alocada/reutilizada, generics/monomorfização para CPU e bus, sem alocação por T-state/evento normal e sem despacho dinâmico por micro-op. Instrumentação é opt-in. Otimizar somente após baseline Release reprodutível.

Nenhuma GPU/CUDA, JIT, frame skip, execução especulativa ou paralelismo de estado de hardware é necessário ou permitido pela arquitetura atual.

## 14. Questões que exigem evidência antes de codificar

- Base de tempo inteira exata que represente clocks e fases de SG-1000/SMS e revisões.
- Relações e fases reais dos clocks Z80/VDP/PSG, inclusive divisores e reset phase.
- Ordem de eventos simultâneos CPU/VDP/VRAM e wait/contention.
- Fases de leitura/escrita Z80 e pinos observáveis por família de instrução.
- H/V counter e timing regional/revisional do SMS VDP.
- Pipeline de fetch, sprite evaluation, CRAM/DAC, scroll latches e efeito de writes raster.
- Diferenças comportamentais comprovadas entre VDP 315-5124 e 315-5246.
- Momento de IRQ e acknowledge, inclusive bordas subinstrução.
- Estado não inicializado/indefinido e comportamento de reset de cada ASIC.

Nenhum valor ou prioridade deve ser preenchido por conveniência. Registrar fonte, seção/página, confiança, contraditórios e teste proposto.

## Referências internas

- [Scheduler mestre atual](native-master-scheduler.md)
- [Temporal Core atual](native-temporal-core.md)
- [Contrato temporal Z80/barramento](z80-temporal-bus-model.md)
- [Projeto temporal do VDP SMS](sms-vdp-cycle-accurate-design.md)
- [Registro de evidências e perguntas abertas](cycle-accuracy-evidence-register.md)
- [Roadmap SG-1000/SMS](master-system-roadmap.md)
