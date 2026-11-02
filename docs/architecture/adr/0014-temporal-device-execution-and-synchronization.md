# ADR-0014 — contratos de execução e sincronização temporal de dispositivos

- **Estado:** Proposto para revisão.
- **Data:** 2026-11-02.
- **Base de trabalho:** PR #350, branch `feat/scheduler-event-queue-snapshot`, SHA `5ae6df8b156f8e01115955a104a5546c2ad6f23c`.
- **Dependências:** ADR-0002, ADR-0010, ADR-0012 e ADR-0013.
- **Referência comparativa:** arquitetura cooperativa de threads do [Ares Scheduler](https://github.com/ares-emulator/ares/tree/master/ares/ares/scheduler).
- **Escopo:** contrato extensível para múltiplas CPUs, coprocessadores, controladores de vídeo, áudio, DMA, timers, I/O e dispositivos auxiliares em sistemas Sega.

## 1. Contexto

O scheduler é a autoridade temporal da máquina emulada. Ele não pode ser projetado como um mecanismo exclusivo do Z80, nem como uma fila que apenas ordena callbacks. O roteiro inclui máquinas com múltiplos processadores e dispositivos independentes, como M68k, Z80, SH-2, VDPs, DMA, PSG e chips FM.

O próximo dispositivo de áudio previsto para o Master System é o Yamaha YM2413 (OPLL), quando presente no perfil de hardware. O YM2413 é um **circuito integrado sintetizador FM**, não uma CPU ou processador de propósito geral. Ainda assim, possui estado interno, clock, registradores, evolução temporal e interação com o barramento; portanto, deve ser um cidadão de primeira classe do scheduler, sem ser tratado como uma simples função de geração de amostras.

O Ares oferece uma referência útil em três pontos: relógios locais por componente, identidade estável para desempate e sincronização em pontos seguros. Seu modelo de corrotinas cooperativas não será copiado. O Sega Emulator manterá uma timeline central, domínios racionais e despacho explícito de eventos, evitando que a ordem de execução dependa de escalonamento de threads do host.

A implementação atual ainda é uma fundação parcial: `MachineTime` é inteiro, `ClockEdgeTime` calcula instantes racionais de borda mas não participa da fila, e a integração temporal de CPU/barramento/VDP/áudio ainda está pendente. Esta ADR define contratos futuros; não declara que tais capacidades já estejam implementadas.

## 2. Decisão arquitetural

A máquina terá **um único scheduler lógico por instância**, responsável por coordenar todos os domínios temporais. Cada componente avança somente sob um limite temporal explícito e reproduzível. O scheduler não conhece semântica de instruções de uma CPU nem detalhes internos de um chip específico.

A arquitetura separa quatro conceitos:

1. **Clock domain:** define a relação racional entre o relógio mestre e as bordas de clock locais de um componente.
2. **Execution component:** implementa a evolução determinística do estado interno de um dispositivo até um limite autorizado.
3. **Scheduled event:** representa uma ocorrência futura observável ou uma fronteira de coordenação que precisa ser despachada na timeline.
4. **Machine/bus profile:** define conexões, decodificação, contenção, prioridades e regras físicas sustentadas por evidência do hardware.

Um componente pode possuir um ou mais domínios de clock se a documentação demonstrar clocks internos distintos. Compartilhar frequência nominal não implica compartilhar estado de fase: domínios distintos mantêm identidades e resíduos próprios, salvo quando a topologia elétrica demonstrar que são o mesmo clock físico.

## 3. Identidade e registro de componentes

- Cada instância de componente recebe um identificador estável, definido pelo perfil da máquina e não pela ordem incidental de alocação, endereço de memória, ordem de threads do host ou hash aleatório.
- IDs de clock domain são estáveis no escopo da máquina e não são reutilizados durante uma sessão ativa.
- A ordem de avanço e exportação deve ser canônica por ID estável.
- Nenhuma decisão de hardware pode depender da iteração de `HashMap`/`HashSet`, de ponteiros, de ASLR ou da ordem de criação de threads.
- Estruturas ordenadas, como `BTreeMap`/`BTreeSet`, ou sequências explicitamente ordenadas devem ser usadas quando a ordem for observável.
- A sequência de inserção da `EventQueue` continua sendo local à fila, conforme ADR-0013; não substitui a identidade estável do componente.

## 4. Contrato de execução dos componentes

O contrato conceitual de um componente temporal deverá permitir ao scheduler:

- identificar os domínios de clock que pertencem ao componente;
- consultar a próxima fronteira temporal conhecida, quando houver;
- avançar deterministicamente até um deadline que não ultrapasse uma fronteira que exija arbitragem;
- processar todos os ciclos/bordas locais devidos no intervalo, preservando a fase racional;
- expor eventos produzidos durante o avanço, com timestamp e identidade de origem;
- aceitar transações externas (por exemplo, leitura/escrita de registrador ou interrupção) no instante determinado pela máquina;
- salvar e restaurar seu estado interno sem depender de endereços ou stacks do host.

Os nomes e assinaturas Rust concretos serão definidos após E4–E7 e prototipagem headless. Esta ADR não congela prematuramente uma trait pública.

### 4.1 Avanço limitado e pontos seguros

Um *safe point* é uma fronteira em que o componente pode suspender sua execução sem perder informação causal ou estado intermediário necessário à retomada.

- CPU: fronteiras de micro-operação/acesso ao barramento suficientemente pequenas para preservar T-states, wait states, interrupções e efeitos de leitura/escrita.
- VDP: fronteiras internas necessárias para preservar fetches, contadores, interrupções, colisões e demais efeitos observáveis; a granularidade depende de evidência do ASIC/perfil.
- PSG/FM: fronteiras necessárias para preservar divisores, fases de osciladores, envelopes, timers, registradores, status e instante de atualização digital.
- DMA e controladores: fronteiras de transferência, arbitragem e acesso compartilhado ao barramento.
- I/O: instante de amostragem/latch e propagação de alterações, conforme o perfil de hardware.

Não se exige que cada oscilador interno gere um evento global por ciclo se o componente puder calcular exatamente o mesmo estado até o próximo limite observável. A otimização por avanço agregado só é válida quando demonstrar equivalência bit a bit com a evolução ciclo a ciclo no intervalo correspondente.

Nenhum componente pode avançar além do deadline autorizado, consumir um evento pertencente a outro componente ou aplicar retroativamente uma transação já ultrapassada.

### 4.2 Semântica de pausa, reset e clock gating

Pausa, reset, suspensão e clock gating são estados explícitos do componente/domínio. Não serão representados por razão de clock zero, manipulação de relógio do host ou congelamento informal de contadores.

Reset deve definir quais estados temporais são reinicializados e quais clocks continuam correndo, com base no hardware. Essa regra é específica do perfil e não pode ser inferida genericamente pelo scheduler.

## 5. Relógios e precisão temporal

- Todo tempo emulado usa inteiros e razões racionais; `f32`/`f64` não podem determinar bordas, ciclos, timestamps, fase, ordem ou estado digital.
- Cada domínio preserva numerador, denominador, origem de registro, fase inicial, fase atual e ciclos acumulados.
- Conversões e avanços usam aritmética checked; overflow/underflow é erro tipado, nunca wrap silencioso.
- Avanço em blocos deve ser *chunk-invariant*: dividir o mesmo intervalo em blocos diferentes deve produzir o mesmo estado e os mesmos eventos.
- O tempo de parede, VSync, latência de áudio, carga da CPU hospedeira e agendamento do sistema operacional não alteram a timeline.
- A fila atual despacha por `MachineTime` inteiro. `ClockEdgeTime` ainda não é timestamp de evento. Qualquer requisito de interleaving sub-tick deve ser tratado em decisão arquitetural separada antes de depender dele; `HardwarePhase` não pode ser usado como substituto de fração temporal.

## 6. Eventos, causalidade e arbitragem

A chave de ordenação permanece a definida na ADR-0012:

`(absolute_timestamp, hardware_phase, platform_priority, stable_sequence)`

- Timestamp é tempo absoluto na timeline da máquina.
- `HardwarePhase` é apenas um estrato ordinal de software até que exista evidência física que sustente outra interpretação.
- `PlatformPriority` é política determinística de despacho, não precedência elétrica.
- `stable_sequence` desempata eventos equivalentes dentro da mesma fila.

Eventos devem transportar dados determinísticos: tipo, origem/destino estáveis, timestamp e payload tipado. A fila não deve persistir closures, ponteiros de função, referências de memória ou objetos dependentes do host.

O despacho pode produzir novos eventos no mesmo timestamp. A política delta-cycle limita loops infinitos, mas não pode descartar eventos válidos nem inventar uma ordem elétrica. A regra de causalidade deve rejeitar regressões já despachadas e permitir eventos simultâneos legítimos quando não violarem as dimensões causais anteriores.

Arbitragem de barramento, wait states, contenção, prioridade de DMA e precedência entre CPU e VDP pertencem ao perfil/barramento e exigem documentação, medição ou oráculo. O scheduler fornece a infraestrutura para executar a decisão; não cria a decisão física.

## 7. Múltiplos processadores e dispositivos heterogêneos

O scheduler não pressupõe uma CPU primária única, uma ISA, uma frequência comum ou um número fixo de componentes.

Deve suportar, sem mudar seu núcleo:

- múltiplas CPUs com domínios independentes;
- coprocessadores e processadores auxiliares;
- dispositivos sem CPU que possuem clock e estado temporal;
- dispositivos com mais de um domínio de clock;
- clocks com razões inteiras ou racionais;
- dispositivos que geram interrupções, DMA ou eventos para outros componentes;
- dispositivos opcionais habilitados por perfil de hardware.

A dependência entre componentes é expressa por transações/eventos e pelo barramento, não por callbacks arbitrários que executam outro dispositivo fora da timeline.

A execução pode ser cooperativa e determinística dentro de uma única thread de emulação. Paralelismo de host não é requisito e não pode alterar a ordem lógica. Qualquer paralelização futura exige prova de independência causal e equivalência diferencial.

## 8. Caso de referência: YM2413 (OPLL) no Master System

O YM2413 será o primeiro dispositivo-alvo para validar que o scheduler não é específico de CPU.

O contrato esperado para sua integração deverá incluir:

1. Registro de identidade estável do dispositivo e de seu(s) domínio(s) de clock no perfil SMS que realmente possua FM.
2. Relação de clock expressa por razão racional em relação ao master clock SMS, com fase inicial explícita e evidência da fonte para frequência, divisão e relação de fase.
3. Escritas de endereço/dados provenientes do caminho de I/O/barramento Z80 entregues ao OPLL no timestamp correto; não aplicar escritas ao final da instrução se o hardware as observa antes.
4. Evolução de operadores, envelopes, fases, ruído/modulação e timers em aritmética inteira/fixed-point determinística, conforme o modelo validado do chip.
5. Produção de amostras/eventos digitais com timestamp ou posição temporal inequívoca; resampling e entrega ao host não controlam o clock do chip.
6. Preservação integral de registradores, estado de operadores, fases, envelopes, timers, status e buffers/posição de saída necessários à continuação exata do snapshot.
7. Testes de escrita/leitura de registradores, latência de escrita, timers/status, reset, continuidade de fase, geração de áudio e replay diferencial.
8. Perfil sem FM não deve registrar nem avançar o dispositivo; perfil com FM deve fazê-lo de forma reproduzível.

**Limite atual:** esta ADR não fixa a frequência nominal, revisão do chip, divisão de clock, fase elétrica, latência de escrita, comportamento de timers/IRQ ou detalhes de mixagem. Esses valores exigem fontes primárias e/ou oráculos independentes e deverão ser documentados na especificação de hardware do SMS/FM. O nome YM2413 identifica o alvo planejado, não uma implementação existente nem um resultado de validação.

## 9. Snapshot, restore e replay

O snapshot de máquina deverá preservar tanto o estado do scheduler quanto o estado de cada componente:

- identidade e versão do perfil;
- todos os clock domains e suas fases/históricos;
- fila, allocator de sequência, watermark e política delta;
- estado interno de cada componente;
- estado de barramento/arbitragem e transações pendentes;
- interrupções, linhas de controle e eventos ainda não consumidos;
- estado de reset/enable/clock gating;
- estado de saída digital necessário para continuar sem descontinuidade.

A restauração será validada e atômica. Não se serializam endereços de memória, ponteiros, ordem física de alocação, stacks de corrotinas ou threads do host. Após restore, o replay deverá reproduzir os mesmos eventos, clocks, transações de barramento, saídas digitais e erros que uma execução contínua.

A conclusão de E4–E7 da ADR-0013 é pré-requisito para declarar o snapshot completo do scheduler; a serialização de estado dos dispositivos será contratada também em suas ADRs específicas.

## 10. Determinismo e estruturas de dados

- Nunca depender de ordem de iteração de `HashMap`/`HashSet` para decisões observáveis.
- Nunca usar ponteiros, `usize` de endereço, ASLR ou endereço de alocação como identidade/ordenação.
- Nunca usar `Debug` output ou layout interno de heap como formato canônico.
- Ordenar eventos por chave completa e identidades por IDs estáveis.
- Ordenações instáveis só podem ser usadas quando a chave total garante desempate único e isso é demonstrado por teste; por padrão, preferir clareza e chave total.
- RNG que afete estado emulado deve ter algoritmo, seed e estado explícitos e serializados. Entropia do host não pode entrar silenciosamente no estado determinístico.
- Operações aritméticas relevantes ao estado digital devem ser inteiras ou fixed-point com regras de overflow e arredondamento documentadas.

## 11. Estratégia de implementação

A ordem de trabalho recomendada é:

1. Concluir e revisar E3 da ADR-0013 no SHA exato; não confundir CI verde com integração na `main`.
2. Concluir E4: snapshot/restore atômico do scheduler e invariantes cruzados.
3. Concluir E5: codec canônico e versão de formato.
4. Concluir E6: replay diferencial, incluindo sequências determinísticas de operações e chunk-invariance.
5. Concluir E7: gates de CI e evidências no SHA exato.
6. Projetar a interface mínima de avanço/safe-point com protótipo headless e testes, antes de conectar hardware.
7. Auditar a necessidade de timestamp sub-tick e fechar sua decisão antes de qualquer dispositivo depender dessa capacidade.
8. Integrar um componente simples existente como primeiro consumidor real, preservando os resultados SG-1000.
9. Integrar o YM2413 como primeiro dispositivo de áudio independente sob o scheduler, com fontes e oráculos.
10. Integrar CPU Z80, VDP, PSG, barramento e demais dispositivos progressivamente.
11. Expandir o mesmo contrato para M68k, Z80 do Genesis, SH-2 e outros processadores/dispositivos, sem especializar o núcleo para o SMS.

Nenhuma etapa autoriza JIT, frame skipping, callbacks Python por ciclo ou paralelismo especulativo.

## 12. Critérios de aceitação da arquitetura

- [ ] Um scheduler por instância de máquina; nenhuma autoridade temporal concorrente.
- [ ] Identidades de componentes/domínios estáveis, explícitas e independentes de alocação.
- [ ] Múltiplos processadores e dispositivos heterogêneos sem pressuposto de CPU primária.
- [ ] Domínios racionais independentes com fase preservada e avanço chunk-invariant.
- [ ] Safe points retomáveis sem perda de microestado nem avanço além do deadline.
- [ ] Barramento/arbitragem separados do mecanismo genérico de despacho.
- [ ] Eventos tipados, sem closures/ponteiros persistidos, com causalidade validada.
- [ ] Nenhuma dependência observável de HashMap, ponteiros, ASLR, wall-clock ou ordem de threads.
- [ ] Snapshot/restore e replay reproduzem eventos e estados futuros.
- [ ] YM2413 pode receber writes no instante correto e evoluir seu clock sem depender de stepping do Z80.
- [ ] Ausência de FM no perfil SMS não cria domínio ou eventos FM.
- [ ] Valores de timing e prioridade específicos de hardware possuem fontes/oráculos rastreáveis.
- [ ] CI Rust/Python sem warnings no SHA candidato exato.
- [ ] `TEMPORAL_PASS` reservado à integração com traces e oráculos independentes; esta ADR não o declara.

## 13. Consequências

A arquitetura deixa de tratar o scheduler como um executor de CPUs e o define como infraestrutura temporal comum a toda a máquina. Isso permite incorporar o YM2413 e, posteriormente, sistemas multi-CPU sem redesenhar a autoridade temporal.

O custo é maior disciplina nos contratos de fronteira, nos IDs estáveis, no estado serializável e na evidência de timing. Essa complexidade é intencional: a prioridade do projeto é fidelidade demonstrável, determinismo e continuidade exata, não conveniência de implementação.

## Referências

- [Ares Scheduler](https://github.com/ares-emulator/ares/tree/master/ares/ares/scheduler)
- [Ares scheduler.cpp](https://github.com/ares-emulator/ares/blob/master/ares/ares/scheduler/scheduler.cpp)
- [Ares thread.cpp](https://github.com/ares-emulator/ares/blob/master/ares/ares/scheduler/thread.cpp)
- [ADR-0002 — scheduler temporal](0002-machine-temporal-scheduler.md)
- [ADR-0010 — runtime Rust e propriedade temporal](0010-rust-runtime-temporal-ownership.md)
- [ADR-0012 — dimensões de ordenação temporal](0012-event-ordering-dimensions.md)
- [ADR-0013 — snapshot/restore determinístico](0013-deterministic-scheduler-snapshot-restore.md)
