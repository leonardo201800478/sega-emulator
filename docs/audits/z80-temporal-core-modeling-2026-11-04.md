# Modelo de execução temporal do Z80 — E7.3 a E7.7

**Data:** 2026-11-04  
**Baseline documental:** `main` em `644b2c22879d63447f63b730ed9415db11e81fab`  
**Continuação de E7.2E:** PR [#506](https://github.com/leonardo201800478/sega-emulator/pull/506), HEAD observado durante esta análise: `8d6751513f8763f5ebfca50ecbaeb9fe469f38fb`  
**Estado deste documento:** proposta de modelagem baseada em código e fontes; não é evidência de implementação nem de aprovação temporal.

## 1. Objetivo

Definir como as instruções e condições ainda pendentes do Z80 devem ser representadas como continuações retomáveis no **Temporal Core já existente**. Não criar um segundo Scheduler, um segundo barramento genérico ou uma CPU específica para SMS.

O Z80 permanece compartilhado. A CPU descreve a próxima operação e preserva seu estado; o perfil da máquina transforma essa operação em eventos temporais concretos; o Scheduler avança a linha temporal e devolve os eventos à máquina.

Três frentes de trabalho orientam a pesquisa e a consolidação:

1. **Referências de implementação:** comparar MAME, ares e Gearsystem para decomposição de instruções, prefixos, ciclos e efeitos colaterais.
2. **Documentação primária:** usar o manual ZiLOG UM008011-0816 como autoridade para a semântica documentada do Z80; usar documentação SMS Power/Sega para comportamento do VDP e integração da plataforma.
3. **Modelagem:** transformar evidências convergentes em estados, transições, micro-operações, contratos e testes Rust. Divergências permanecem registradas até serem resolvidas por documentação, teste independente ou evidência de hardware.

## 2. Fontes consultadas e limites

### 2.1 Implementações de referência

- [MAME — z80.lst, revisão 1ee501d](https://github.com/mamedev/mame/blob/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/devices/cpu/z80/z80.lst) e [diretório do core](https://github.com/mamedev/mame/tree/1ee501dbef32ba9a4597d9f5c27bed4a0164e499/src/devices/cpu/z80): descrição declarativa das instruções e operações separadas de fetch, memória, I/O, refresh e intervalos internos. Útil para decomposição e custos de ciclo; não é a especificação final do silício.
- [ares — Z80, revisão 234c999](https://github.com/ares-emulator/ares/tree/234c99911f471cb533aa564909aeb12920fce89f/ares/component/processor/z80): instruções compostas por operações de leitura/escrita intercaladas com esperas internas. O caminho indexed-CB é uma referência útil para a ordem displacement, subopcode e operação de memória.
- [Gearsystem, revisão f1202e8](https://github.com/drhelius/Gearsystem/tree/f1202e8cbfd1ef8a0addc698faa573f219b14a55/src): referência próxima do ecossistema SMS para semântica e dispatch. Sua execução por instrução/tabelas de timing não substitui uma continuação que possa ser suspensa em cada fronteira temporal.

Esses projetos são oráculos comparativos, não fontes para copiar código sem auditoria de licença, variante e semântica. A concordância entre dois emuladores não substitui o manual ou um teste independente.

### 2.2 Documentação primária

- [ZiLOG Z80 CPU User Manual — UM008011-0816](https://www.zilog.com/docs/z80/um0080.pdf), também fornecido nos materiais desta sessão.
- [SMS Power — documentação oficial SMS](https://www.smspower.org/Development/SMSOfficialDocs).
- [SMS Power — registradores VDP](https://www.smspower.org/Development/VDPRegisters) e [porta de controle/status](https://www.smspower.org/Development/ControlPort).

Do manual ZiLOG, os contratos relevantes para esta modelagem são:

- M1 é o fetch de opcode; normalmente ocupa quatro T-states, com extensão por WAIT. Fetches e prefixos não devem ser confundidos com leituras comuns de memória.
- Leituras/escritas comuns de memória normalmente ocupam três T-states. O dado de um fetch M1 é amostrado em T3; T3/T4 também participam do refresh.
- WAIT é amostrado na borda descendente durante T2 e cada estado TW subsequente. I/O possui um wait state automático no modelo documentado pelo manual.
- BUSREQ é amostrado na borda ascendente do último T-state de qualquer M-cycle. Quando aceito, os barramentos e sinais de controle passam a alta impedância na borda seguinte; durante BUSACK, INT e NMI não são aceitos.
- INT é amostrado no fim da instrução; o acknowledge usa M1 com IORQ e dois wait states automáticos.
- Durante HALT, o Z80 repete ciclos M1, ignora o byte lido e mantém refresh. INT/NMI são amostrados na borda ascendente de cada T4; NMI tem prioridade.
- RETI e RETN compartilham o retorno da pilha, mas não o efeito externo: RETI sinaliza ao periférico o fim do serviço para daisy-chain; RETN restaura IFF1 a partir de IFF2.

Referências no manual: seção Timing, pp. impressas 7–16; HALT Exit, p. 15; Interrupt Enable/Disable, pp. 17–18; RETI/RETN, pp. 288–290.

## 3. Contratos arquiteturais existentes

A modelagem deve reutilizar as seguintes responsabilidades observadas no código Rust:

| Componente | Responsabilidade atual | Limite que deve ser preservado |
|---|---|---|
| `Z80MicroSequencer` | Estado arquitetural e continuação da instrução; emite `BusRequest`, `InternalCycle` ou resultado terminal. | Não acessa memória/barramento diretamente e não avança relógio. |
| `sega-temporal-core` | `MachineTime`, contratos `BusTransaction`/`BusOperationResult`, eventos e primitivas temporais. | Não define clocks, prioridades ou comportamento elétrico de um console. |
| `MachineScheduler` | Linha temporal única, domínios de clock, ordenação e dispatch de eventos. | Não executa callbacks de hardware; a máquina consome os eventos despachados. |
| `SmsTemporalMachine` | Orquestra CPU, barramento, M1 e ciclos internos no perfil SMS. | Não deve duplicar a continuação da CPU nem introduzir um Scheduler paralelo. |
| `SmsTemporalM1` | Avança M1 em eventos individuais de T-state; amostra opcode em T3 e WAIT em T2/TW. | A máquina continua sendo proprietária do dispatch e dos efeitos da leitura. |
| `SmsTemporalBus` | Requests transacionais e commits nos limites de conclusão. | Não antecipar efeitos de memória/I/O nem consumir completion antes do acknowledgement. |

`BusTransaction` é metadado lógico de uma operação, não uma forma de onda elétrica completa. Os índices M-cycle/T-state não representam, por si, subfases de meia borda. O perfil de máquina é responsável por converter T-states em ticks mestres e por definir a ordenação de eventos.

A fatia temporal SMS ainda não representa VDP/PSG como fontes temporais integradas; o vetor padrão do barramento temporal é um recurso de fixture, não uma IRQ real do VDP. O DTO `SmsTemporalMachineState` permite export/restore em memória, mas não prova a existência de codec persistente binário da máquina SMS completa.

## 4. Modelo obrigatório por instrução

Cada opcode ou sequência de prefixos deverá ter uma ficha verificável com:

1. **Classificação:** BASE, CB, ED, DD, FD, DDCB/FDCB, alias documentado, prefixo ignorado, não documentado ou reservado.
2. **Decodificação:** bytes consumidos, PC, número de fetches M1 e comportamento de prefixos repetidos.
3. **Micro-operações ordenadas:** fetch M1, leitura/escrita de memória, I/O, acknowledge, refresh e ciclos internos.
4. **Temporização:** duração nominal de cada M-cycle, posição de cada T-state, WAIT automático e WAIT externo aplicável.
5. **Pontos de amostragem/commit:** quando o dado é amostrado, quando a escrita produz efeito e quando o estado arquitetural é atualizado.
6. **Efeitos semânticos:** registradores, flags S/Z/H/PV/N/C e bits X/Y quando definidos, WZ/MEMPTR, Q, R, PC/SP e bancos alternativos.
7. **Arbitragem:** fronteiras em que INT/NMI/BUSREQ podem ser amostrados e prioridade aplicável.
8. **Retomada:** estados válidos para snapshot, request/continuation IDs, eventos pendentes e invariantes cruzadas.
9. **Evidência:** teste dirigido, teste diferencial, fonte documental e trace reproduzível.

Uma instrução só estará modelada temporalmente quando a sequência de operações puder ser suspensa e retomada sem repetir commits, perder requests, alterar a ordem causal ou divergir da execução contínua.

## 5. E7.3 — amostragem T4 durante HALT

### Comportamento-alvo

1. A instrução HALT é buscada normalmente; depois de decodificada, o estado HALT fica ativo.
2. Cada ciclo de retenção é um M1 de quatro T-states, extensível por WAIT conforme o ciclo; o endereço de fetch permanece no PC, o byte lido é ignorado e R incrementa preservando o bit 7.
3. No T4 efetivo, na borda ascendente, o CPU amostra NMI e INT. NMI prevalece. INT só é elegível com IFF1 ativo e atraso EI consumido.
4. Se nenhuma interrupção for elegível, o próximo ciclo M1 de HALT é iniciado.
5. Se uma interrupção for elegível, a saída de HALT ocorre na próxima borda ascendente e o ciclo seguinte é o acknowledge/resposta correspondente.

### Modelagem proposta

- Introduzir no estado serializável do microsequenciador uma latch explícita de saída de HALT, distinguindo NMI de INT e preservando o vetor amostrado quando aplicável. Não reutilizar apenas `nmi_pending`/`int_pending`: essas entradas podem mudar depois da borda T4 e não representam, sozinhas, o valor efetivamente amostrado.
- Expor uma transição de amostragem de HALT no Z80 compartilhado; o perfil da máquina chama essa transição somente na conclusão do T4 real do M1.
- A amostragem deve ocorrer na conclusão temporal do T4, depois que o fetch/decodificação da instrução HALT foi concluído. Isso é necessário para que `EI; HALT; INT` observe o consumo do atraso de EI na aposentadoria de HALT antes de decidir a elegibilidade.
- Na próxima chamada de continuação, consumir a latch e iniciar a resposta NMI ou o acknowledge INT. Se não houver latch, continuar a retenção M1. Entradas que mudem depois de T4 só podem ser aceitas em um T4 posterior.
- Serializar e validar a latch em snapshot; atualizar versão do codec binário do microsequenciador se a representação persistida mudar. Não confundir esse codec com um codec da máquina SMS completa.

### Testes de aceitação

- INT elegível antes de T4; INT mascarada; IFF1 desligado; atraso EI ativo e consumido por HALT.
- Sequência explícita `EI; HALT; INT`.
- NMI isolada e NMI simultânea com INT, verificando prioridade.
- Mudança da linha INT imediatamente após T4: não pode antecipar a aceitação.
- WAIT=0/1/múltiplos no M1, confirmando amostragem no T4 efetivo, não em um índice nominal fixo.
- Snapshot antes do T4, após o T4 com latch pendente e após a saída; comparar execução contínua e retomada, incluindo trace, PC, R, IFF e tempo absoluto.

**Dependência:** a prioridade de BUSREQ sobre NMI/INT será fechada em E7.4. E7.3 não pode declarar arbitragem global concluída antes desse incremento.

## 6. E7.4 — BUSREQ/BUSACK e arbitragem

BUSREQ não é WAIT e não deve ser modelado como aumento genérico da duração de uma transação.

Modelo proposto:

- Representar BUSREQ como entrada de nível e BUSACK como estado de propriedade/indisponibilidade do barramento.
- Amostrar BUSREQ no limite documentado do último T-state de cada M-cycle.
- Se aceito, preservar integralmente a continuação da CPU e os requests ainda não iniciados; colocar as linhas de barramento aplicáveis em alta impedância e permitir que o mestre externo progrida no mesmo Scheduler.
- Enquanto BUSACK estiver ativo, a CPU não emite novos ciclos nem aceita INT/NMI. O tempo da máquina e os eventos do mestre externo continuam sob a linha temporal única.
- Na liberação, retomar exatamente a continuação suspensa, sem refazer uma escrita ou consumir uma leitura duas vezes.
- Snapshot deve preservar BUSREQ, BUSACK, proprietário do barramento, request suspenso e eventos DMA pendentes.

Testes: request em cada classe de fronteira M-cycle, BUSREQ simultâneo com INT/NMI, duração variável de BUSACK, liberação em safe point, snapshot/restore e equivalência contínua/retomada.

## 7. E7.5 — RETI e daisy-chain

- Preservar o retorno da pilha e separar semanticamente RETI de RETN.
- RETN restaura IFF1 a partir de IFF2 conforme o manual.
- RETI deve produzir um evento explícito de fim de serviço no limite de execução definido pela referência, consumível pelo controlador de interrupções/periféricos da máquina. A CPU não deve depender de callback Python nem conhecer o dispositivo concreto.
- O controlador daisy-chain mantém prioridade em cadeia e estado de serviço; RETI libera o dispositivo em serviço conforme a ordem de prioridade.
- Verificar separadamente o opcode documentado e aliases encontrados nos cores de referência. Não promover aliases não documentados a comportamento universal sem classificar variante e evidência.

Testes: RETI versus RETN, IFF1/IFF2, pilha e PC, evento emitido uma única vez, snapshot antes/depois do evento e sequência com dois dispositivos de prioridades diferentes.

## 8. E7.6 — IRQ real do VDP SMS

A linha INT deve ser derivada do estado real do VDP, não do vetor default de fixture.

Modelo a integrar com o VDP compartilhado já existente:

- Preservar separadamente os estados pendentes de frame/VBlank e line interrupt.
- Aplicar os enables documentados em R1 (frame) e R0 (line).
- Derivar a linha IRQ ativa a partir das fontes pendentes habilitadas.
- Leitura do status/control port deve executar os efeitos de acknowledge/clear definidos pelo VDP e atualizar a linha INT no mesmo limite temporal.
- Eventos de VDP e CPU devem compartilhar a linha temporal da máquina; não amostrar uma IRQ por polling de GUI ou timer externo.

Testes: cada fonte isolada, fontes simultâneas, enables independentes, leitura de status/acknowledge, assert/deassert da linha, interrupção mascarada no Z80 e snapshot/replay em torno do evento.

**Limite documental:** os detalhes de contagem de linha, instante de geração e diferenças entre revisões de VDP devem ser fixados por perfil de hardware e fontes SMS Power/Sega; não inferir todos os detalhes elétricos a partir do manual genérico do Z80.

## 9. E7.7 — fechamento semântico de todas as famílias

A auditoria deve percorrer a matriz existente BASE/CB/ED/DD/FD/DDCB/FDCB, sem confundir classificação do decoder com correção semântica.

Para cada célula, comparar o microsequenciador com o executor de referência interno e com referências externas fixadas. Incluir flags, PC/SP, WZ/MEMPTR, Q, R, bancos alternativos, bytes consumidos, efeitos em memória/I/O, M-cycles/T-states e ordem de requests. Estados desconhecidos ou não documentados permanecem explicitamente UNKNOWN/UNSPECIFIED e não recebem valores inventados.

Instruções de bloco devem cobrir repetição, término antecipado, BC=0000, wraparound, flags e interrupção entre iterações. Prefixos devem cobrir repetição, cancelamento, substituição de registradores, prefixos ignorados e a sequência DDCB/FDCB com displacement e subopcode não-M1 conforme o modelo validado.

## 10. Sequência de implementação e gates

A ordem continua sendo:

1. Fechar e validar E7.2E na PR #506.
2. E7.3 — HALT T4.
3. E7.4 — BUSREQ/BUSACK.
4. E7.5 — RETI/daisy-chain.
5. E7.6 — IRQ real do VDP.
6. E7.7 — auditoria semântica integral.
7. Só depois, E8 diferencial/formal e E9 temporal sistêmica.

Para cada incremento: branch e PR isoladas, testes direcionados, regressão Rust/Python, Windows x64 quando aplicável, Scheduler stress executado e classificado separadamente, documentação atualizada e evidência ligada ao SHA exato.

Gates obrigatórios de repositório:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Scheduler stress `SKIPPED` não é `PASS`. Compilação não é correção; execução visual não é precisão temporal. Não declarar LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS sem execução e evidência reproduzível no SHA avaliado.

## 11. Estado no momento desta modelagem

- A PR #506 contém a matriz E7.2E de estados inválidos, WAIT e restore. No HEAD `8d6751513f8763f5ebfca50ecbaeb9fe469f38fb`, o teste Rust `e7_2e_wait_matrix_covers_m1_memory_io_internal_and_interrupt_cycles` passou após correção do timestamp de início da instrução no caminho de conclusão de ciclo interno.
- No momento do registro, o job Windows x64 ainda estava em execução e Scheduler deterministic stress estava SKIPPED. Portanto, E7.2E permanece `INTEGRATION_PENDING`.
- Nenhuma das subfases E7.3–E7.7 é declarada implementada por este documento.

Este documento define a modelagem e os critérios para as próximas alterações; o estado remoto do GitHub e os testes no SHA final continuam sendo a fonte da verdade.
