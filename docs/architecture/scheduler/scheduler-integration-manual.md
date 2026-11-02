# Manual de arquitetura e integração do Master Clock Scheduler

- **Escopo:** arquitetura temporal comum às máquinas Sega do projeto.
- **Natureza:** documento normativo para implementação futura e revisão de código.
- **Autoridade de implementação:** crates Rust em `native/temporal_core` e `native/machine_scheduler`.
- **Não é:** prova de que todos os dispositivos já estejam integrados.

## 1. Objetivo

O Scheduler mantém uma timeline virtual única por instância de máquina. Ele permite que componentes com frequências diferentes avancem de forma determinística, preservando razão e fase, e que eventos sejam observados em uma ordem total reproduzível.

A unidade mestre é definida pelo perfil da máquina. O Scheduler não deve presumir que um tick mestre equivale a um ciclo Z80, um pixel, uma borda de VDP ou um ciclo de áudio.

## 2. Princípios invioláveis

1. Uma instância de máquina possui uma única autoridade temporal.
2. Todo tempo digital é inteiro/racional; ponto flutuante não decide ciclos, fases ou ordem.
3. Cada domínio conserva sua própria razão e fase residual.
4. Avanços devem ser monotônicos e aritmética deve ser checked.
5. Eventos possuem identidade/payload determinísticos e chave de ordenação estável.
6. Um componente não avança além do deadline concedido nem atravessa uma fronteira de interação ainda não processada.
7. O Scheduler não executa lógica de dispositivos nem chama callbacks arbitrários.
8. A máquina consome despachos e coordena os dispositivos em ordem explícita.
9. Nenhum componente cria um segundo master clock, scheduler privado ou contador temporal concorrente.
10. Processamento agregado só é permitido se for demonstravelmente equivalente à evolução detalhada até o próximo ponto observável.
11. Python, Qt, áudio host, VSync, relógio do SO e threads de apresentação nunca comandam o tempo emulado.
12. Sem JIT, frame skip, paralelismo especulativo ou hacks temporais.

## 3. Divisão de responsabilidades

| Camada | Responsabilidade | Não deve fazer |
|---|---|---|
| `temporal_core` | Representação do tempo, razões, fases, bordas calculadas, fila e causalidade | Conhecer SMS, Z80, registradores ou semântica de chips |
| `machine_scheduler` | Timeline por máquina, registro de domínios, despacho, limites e snapshot | Executar CPU/VDP/PSG, arbitrar barramento ou sintetizar áudio |
| Perfil da máquina | Definir unidade mestre, IDs, razões, fases iniciais e semântica de eventos | Reimplementar aritmética temporal |
| Bus/máquina | Resolver transações, contenção, WAIT, efeitos colaterais e ordem física conforme evidência | Manter relógio independente |
| Dispositivos | Manter estado funcional e temporal interno; avançar até limite autorizado | Avançar globalmente ou despachar eventos de outros dispositivos |
| Host/engine | Pacing, apresentação, filas de controle, métricas | Alterar timestamps ou estado temporal digital |
| Python | Controle de alto nível e telemetria | Participar do loop temporal ou callbacks por instrução |

## 4. Modelo de execução

A máquina deve operar cooperativamente:

1. Consultar/obter o próximo evento ou deadline relevante.
2. Determinar o limite seguro até o qual cada componente pode avançar sem cruzar uma interação externa.
3. Avançar os domínios até o timestamp autorizado.
4. Consumir o `MachineDispatch` retornado.
5. Interpretar o payload no nível da máquina/perfil.
6. Executar a operação do dispositivo ou transação correspondente no instante definido pelo perfil.
7. Permitir que o processamento produza novos eventos, respeitando a causalidade da fila.
8. Repetir até o alvo solicitado (`run_until`, frame ou evento de controle).

O passo 5–6 não é executado pelo Scheduler atual. A integração deve ser adicionada no nível da máquina sem transformar o Scheduler em um framework de callbacks.

## 5. Registro de domínios

Cada domínio recebe um `ClockDomainId` estável e uma `ClockRatio` positiva. IDs devem ser constantes do perfil da máquina, não derivados da ordem de construção, ponteiro, hash aleatório ou thread.

O registro inicial ocorre em um timestamp conhecido. A fase inicial deve ser explícita quando a relação de fase entre os clocks for relevante e sustentada por evidência. Não usar fase zero por conveniência quando o hardware exigir outra.

Domínios distintos com a mesma frequência nominal não são automaticamente o mesmo domínio: a identidade/fase depende da topologia real.

## 6. Eventos

A chave atualmente implementada é:

`(MachineTime, HardwarePhase, PlatformPriority, insertion_sequence)`

Timestamp é absoluto. `HardwarePhase` e `PlatformPriority` são dimensões ordinais de ordenação de software. Não representam automaticamente sub-tick, fase elétrica ou prioridade física de barramento.

A sequência de inserção é estável dentro da fila. Eventos com payload não determinístico, closures, ponteiros ou referências de host não podem ser persistidos.

A fila rejeita eventos retroativos e regressões causais. Eventos simultâneos só podem ser ordenados por dimensões documentadas do perfil; não inventar prioridades para produzir o resultado visual esperado.

## 7. Granularidade e eficiência

O Scheduler não precisa emitir um evento para cada tick mestre se nenhum componente precisar observar aquele tick. Pode-se avançar em spans até o próximo evento/fronteira, desde que:
- o estado final seja idêntico ao avanço detalhado;
- todos os eventos internos produzidos sejam preservados;
- nenhuma interação de outro componente seja atravessada;
- a equivalência seja testada diferencialmente.

A API atual despacha eventos em `MachineTime` inteiro. `ClockEdgeTime` calcula uma posição racional de borda, mas não habilita dispatch sub-tick. Não converter frações em `HardwarePhase` artificialmente.

## 8. Delta events

A política delta limita quantos eventos podem ser despachados no mesmo timestamp para impedir ciclos infinitos. O default atual é 1024; é limite de segurança de software, não propriedade do hardware.

Exceder o limite deve falhar de forma explícita e preservar o evento pendente. Alterar o limite deve ser política consciente e coberta por teste.

## 9. Falhas e atomicidade

Overflow, timestamp regressivo, razão inválida, domínio duplicado/desconhecido, evento causalmente inválido e estado de restore inválido devem resultar em erro tipado. Nunca corrigir silenciosamente valores, fazer wrap, descartar evento ou avançar parcialmente.

Operações compostas devem validar/preflight antes de mutar o estado sempre que a falha possa ocorrer no meio da operação. Testes devem verificar que falhas deixam o estado observável inalterado.

## 10. Estado e persistência

Snapshot de máquina não é apenas snapshot do Scheduler. Deve incluir estado de todos os dispositivos, transações pendentes, estado de execução da CPU, buffers/filas digitais e eventos futuros relevantes. A restauração deve ser atômica e preservar exatamente a continuação.

O Scheduler fornece DTOs e codec de seu próprio estado; payloads de evento pertencem ao codec fornecido pela aplicação e precisam de versão própria.

## 11. Integração por fases

1. **Inventário:** identificar o que existe e o que ainda avança tempo por conta própria.
2. **Contrato:** mapear clock, fase, safe points, eventos e interações.
3. **Prova local:** testar domínio isolado, avanço segmentado, overflow e snapshot.
4. **Adaptador:** implementar avanço limitado e transações sem duplicar estado.
5. **Máquina:** conectar scheduler e componentes em ordem determinística.
6. **Diferencial:** comparar caminho detalhado e caminho agregado, além do executor legado quando aplicável.
7. **Integração:** validar save/restore, replay, interrupções, bus WAIT e regressões de sistema.
8. **Evidência:** registrar SHA, workflow, fontes, limitações e status por suíte.

## 12. Níveis de integração

- **Scheduler-aware:** o componente usa os tipos/contratos temporais, mas ainda pode não ser conduzido pelo Scheduler.
- **Scheduler-driven:** a máquina efetivamente concede deadlines e o Scheduler controla os avanços.
- **Temporally integrated:** CPU, bus e dispositivos interagem na cronologia correta, com safe points, causalidade e efeitos observáveis validados.

Não usar os termos como sinônimos. Um componente pode ser Scheduler-aware sem estar integrado temporalmente.

## 13. Evidência e alterações

Toda regra física deve apontar para fonte primária, medição ou oráculo com ancestralidade declarada. Código de outro emulador é evidência comparativa, não prova isolada. Hipóteses devem ser rotuladas `INFERENCE` ou `UNKNOWN`.

Alterações no Scheduler exigem branch/PR, testes focados, gates do workspace e revisão de documentos. Não integrar diretamente na `main`. Não declarar `TEMPORAL_PASS` sem os critérios sistêmicos definidos em [validação](validation-requirements.md).
