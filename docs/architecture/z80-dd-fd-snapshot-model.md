# Modelo técnico: prefixos Z80 DD/FD, microsequenciamento e snapshots

**Status:** especificação de modelagem e protótipo conceitual; não declara implementação completa nem `TEMPORAL_PASS`.
**Escopo:** CPU Z80 compartilhada, com foco na fatia DD/FD da PR #447 e nos contratos necessários para snapshots retomáveis.
**Base de código auditada:** branch `feature/z80-dd-fd-indexed-sequencer`, HEAD `74c822afe4dafd892d370956c537b263a34f6d25`; estado da `main` consultado em 2026-11-03.
**Fontes primárias:** Zilog Z80 CPU User Manual UM008011-0816, especialmente Timing (pp. 7–10), Indexed Addressing (pp. 36–37) e instruções ADD IX/IY, INC/DEC IX/IY e indexed CB (pp. 194–202, 217, 247–249).

> Este documento separa fatos do manual Zilog, comportamento não documentado/reverse-engineered e observações do código. Uma hipótese de pesquisa não é promovida a fato sem evidência.

## 1. Revisões especializadas paralelas

A modelagem foi dividida em três frentes independentes e reconciliada nesta especificação.

### Frente A — Arquitetura de snapshots e persistência

Responsabilidade:
- inventariar estado arquitetural, fase do microsequenciador e handshake do barramento;
- distinguir snapshot de CPU, snapshot da fatia temporal SMS e save state integral da máquina;
- definir codec canônico, validação de integridade e restauração sem mutação parcial.

Conclusão:
- `native/z80_cpu/src/snapshot.rs` já implementa um codec binário próprio, versionado, com magic `Z80SEQ\0\0`, comprimento do corpo, CRC32 do corpo e rejeição de truncamento/bytes excedentes;
- o codec valida o estado reconstruindo-o por `Z80MicroSequencer::restore_state`;
- o estado serializado inclui registradores principais/alternativos, IX/IY, I/R, flags e estados de interrupção representados pelo sequenciador, fase, opcode, índices M/T, handshake, requisição pendente e WAIT acumulado;
- isso cobre somente o subconjunto retomável representado pelo sequenciador; não é um save state completo do Z80 nem do SMS;
- `SmsTemporalMachineState` é atualmente um DTO in-memory que agrega CPU, barramento, memória, coordenador/Scheduler, M1 e continuations. Não foi identificado codec persistente integral para esse DTO.

Decisão: manter o codec existente. Não introduzir `SMSK v1`, Serde/Bincode ou um segundo envelope paralelo. Qualquer expansão dos campos persistidos exige atualizar deliberadamente `Z80_MICRO_SEQUENCER_STATE_VERSION` e os testes de compatibilidade.

### Frente B — Semântica e FSM DD/FD

Responsabilidade:
- modelar repetição e precedência de prefixos;
- distinguir prefixos que modificam operandos, prefixos ignorados e cancelamento por ED;
- auditar flags, WZ e sequências indexadas.

Conclusão:
- a PR #447 já cobre LD IX/IY,nn; prefixos DD/FD repetidos na sequência testada; INC/DEC IX/IY; ADD IX/IY,rr; estados internos suspensíveis e round-trip do codec v18;
- o campo `opcode` funciona atualmente como latch do prefixo anterior durante a busca; as fases `IndexImmediateLow/High` e `Ed16ArithmeticInternal.index_prefix` carregam o prefixo selecionado nos estados dessas instruções;
- o desenho atual é uma fatia incremental, não um decodificador completo DD/FD;
- DD/FD seguido de ED e DD/FD CB ainda não têm semântica implementada na fatia atual. Prefixos antes de instruções não cobertas podem terminar em `UnsupportedOpcode`;
- não se deve inferir suporte integral a IXH/IXL/IYH/IYL ou a todas as substituições de HL/H/L/(HL).

### Frente C — Integração temporal, barramento e validação

Responsabilidade:
- verificar o contrato CPU ↔ bus ↔ Scheduler;
- separar M1, memória, ciclos internos e WAIT;
- definir replay diferencial e critérios de aceitação.

Conclusão:
- `SmsTemporalBus` mantém requisição e conclusão latched até acknowledgement, prevenindo repetição de efeitos de escrita;
- `SmsTemporalMachine` agrega CPU, bus/coordinator, estado M1 e ciclos internos, e valida coerência temporal no restore in-memory;
- a fatia temporal atual declara explicitamente exclusões: VDP, PSG, portas, IRQ, DMA e WAIT físico. WAIT é fornecido por fixture;
- o microsequenciador não representa ainda todas as bordas elétricas de qualquer T-state. O contrato atual expõe posições selecionadas e ciclos suspensíveis do subconjunto.

Decisão: a PR #447 não pode ser descrita como integração temporal completa do Z80/SMS nem como `TEMPORAL_PASS`.

## 2. Base técnica confirmada no manual Zilog

### 2.1 M-cycles, T-states e WAIT

O manual Zilog define instruções como sequências de machine cycles (M) compostos por T-states. Um M1 de busca de opcode tem normalmente quatro T-states; T3 e T4 são usados para refresh. A linha WAIT é amostrada durante T2 e em cada TW subsequente, prolongando o ciclo enquanto estiver ativa. Ciclos de memória comuns têm normalmente três T-states, podendo ser estendidos por WAIT.

Consequências para o modelo:
- M1 não pode ser reduzido a uma leitura abstrata de opcode sem preservar sua fase de refresh quando o objetivo for precisão de sinais;
- WAIT é parte do ciclo em andamento, não uma pausa genérica entre instruções;
- o estado retomável precisa distinguir requisição pendente, conclusão latched, WAIT acumulado e fase M/T;
- uma resposta de barramento já aplicada não pode ser executada novamente no restore/replay.

### 2.2 Timings oficiais usados como vetores

| Instrução | M-cycles | T-states | Distribuição publicada |
|---|---:|---:|---|
| ADD IX,pp / ADD IY,rr | 4 | 15 | 4, 4, 4, 3 |
| INC IX/IY | 2 | 10 | 4, 6 |
| DEC IX/IY | 2 | 10 | 4, 6 |
| LD IX/IY,nn | 4 | 14 | 4, 4, 3, 3 |
| RLC (IX+d)/(IY+d) | 6 | 23 | 4, 4, 3, 5, 4, 3 |
| BIT b,(IX+d)/(IY+d) | 5 | 20 | 4, 4, 3, 5, 4 |

Os timings de indexed-CB variam por família de instrução. Portanto, não se deve atribuir um único total ou decomposição a todo o conjunto DD/FD CB.

### 2.3 Flags documentadas

- ADD IX,pp / ADD IY,rr: S, Z e P/V não são afetadas; H é carry do bit 11; N é resetada; C é carry do bit 15.
- INC IX/IY e DEC IX/IY: o manual declara que nenhuma flag é afetada.
- Flags não documentadas, como bits X/Y em operações indexadas, devem ser fundamentadas separadamente em documentação de engenharia reversa e testes diferenciais; não devem ser atribuídas ao manual oficial.

### 2.4 Endereçamento indexado

O deslocamento `d` é um inteiro de complemento de dois somado ao registrador IX/IY para formar o endereço efetivo. O registrador índice não é alterado por esse cálculo.

## 3. Modelo de estados proposto

### 3.1 Princípio

O decodificador deve manter o contexto do prefixo até que um opcode final seja reconhecido. O estado transitório deve ser serializável e não depender de ponteiros, closures, iteradores, endereços de heap ou callbacks do host.

Modelo lógico de referência (não é uma declaração de tipos já implementados):

```rust
enum IndexPrefix {
    Ix, // DD
    Iy, // FD
}

enum PrefixContext {
    None,
    Index(IndexPrefix),
    Cb,
    Ed,
    IndexCb {
        index: IndexPrefix,
        displacement: i8,
    },
}

struct PrefixDecodeState {
    context: PrefixContext,
    final_opcode: Option<u8>,
    displacement_latch: Option<i8>,
    effective_address: Option<u16>,
}
```

Regras de transição:
1. DD seleciona IX; FD seleciona IY.
2. Um DD/FD subsequente substitui o prefixo indexado anterior; o último seleciona o índice.
3. Cada byte DD/FD consumido como prefixo ocupa um M1 próprio e incrementa R.
4. ED cancela a substituição indexada anterior e inicia o espaço ED.
5. CB após DD/FD entra no caminho indexado CB, cuja ordem de bytes é especial.
6. Um opcode não afetado por DD/FD conserva a semântica base, mas o custo temporal do prefixo permanece.
7. O contexto de prefixo só é limpo no ponto definido pela conclusão/retirada da instrução ou pela regra de cancelamento correspondente.

### 3.2 Máquina de estados de alto nível

```text
READY
  |
  v
FETCH_M1
  |-- DD ----------------------> PREFIX_IX --+
  |-- FD ----------------------> PREFIX_IY --+
  |-- ED ----------------------> PREFIX_ED   |
  |-- CB ----------------------> PREFIX_CB   |
  |-- opcode base ------------- > DECODE_BASE|
                                             |
PREFIX_IX / PREFIX_IY                         |
  |-- DD/FD --------------------> replace prefix; FETCH_M1
  |-- ED ----------------------> cancel index; PREFIX_ED
  |-- CB ----------------------> INDEX_CB_DISPLACEMENT
  |-- opcode final ------------> DECODE_INDEXED
                                             |
INDEX_CB_DISPLACEMENT                        |
  |-- read signed d ------------------------> INDEX_CB_OPCODE
  |-- preserve d and M/T progress             |
INDEX_CB_OPCODE                               |
  |-- read final opcode ---------------------> INDEX_CB_EXECUTE
                                             |
DECODE_INDEXED / DECODE_BASE / PREFIX_ED     |
  |-- issue bus/internal operation           |
  v                                          |
SUSPENDED_BUS_OR_INTERNAL                    |
  |-- WAIT / pending completion ------------+
  |-- acknowledge exactly once
  v
NEXT_MICRO_OP -> ... -> RETIRE -> READY
```

A máquina de estados real deve continuar sendo dirigida por operações e respostas explícitas do barramento. O diagrama é uma decomposição funcional; não substitui a tabela M/T por opcode.

### 3.3 Estado mínimo de continuação

Para o escopo indexado futuro, o estado lógico deverá preservar, conforme aplicável:
- prefixo ativo e classe do espaço de opcode;
- opcode final já consumido;
- displacement signed já lido;
- endereço efetivo calculado;
- byte lido, resultado ALU e valor de escrita ainda não confirmado;
- fase de micro-operação, M-cycle e T-state;
- PC e R já atualizados até aquele ponto;
- WZ/MEMPTR no valor intermediário correto;
- request ID, tipo/endereço/dados da operação pendente;
- WAIT acumulado, completion latched e indicador de acknowledgement;
- qualquer evento/continuation ID pendente do Scheduler.

Não criar um campo redundante se o mesmo fato já estiver codificado numa fase validada. Ao generalizar o decoder, preferir um `PrefixDecodeState` explícito a usar `opcode` como latch implícito de múltiplas responsabilidades; a migração requer bump do schema.

## 4. Auditoria objetiva da PR #447

### Implementado na branch consultada

- `LD IX,nn` e `LD IY,nn` com leituras low/high separadas.
- Prefixos repetidos DD/FD e seleção do último prefixo nos vetores adicionados.
- `INC IX/IY` e `DEC IX/IY`, com preservação das flags.
- `ADD IX/IY,BC/DE/IX|IY/SP`, com sete T-states internos, commit final, flags e WZ.
- Codec Z80 v19 com serialização das fases novas e testes de round-trip nos pontos intermediários.
- WAIT inserido na requisição pendente em testes de sequenciador.

### Ainda fora da fatia implementada

- Cancelamento DD/FD por ED e execução normal do opcode ED subsequente.
- DD/FD CB com displacement antes do opcode final.
- BIT indexado e flags X/Y provenientes do comportamento não documentado.
- Instruções indexadas com IXH/IXL/IYH/IYL e substituição de HL/H/L/(HL) em toda a tabela.
- Todas as instruções base não afetadas após prefixo DD/FD.
- Interrupções completas integradas ao sequenciador retomável.
- WAIT físico e temporização elétrica completa.
- Integração do microsequenciador experimental ao runtime temporal SMS completo.

### Observação de nomenclatura

A fase `Ed16ArithmeticInternal` é reutilizada pela branch para ADD IX/IY, além de ADC/SBC HL,ss do espaço ED. Funciona como estado de continuação no escopo atual, mas o nome não representa a semântica ampliada. Uma refatoração de nomenclatura pode ser considerada quando o conjunto de operações for expandido; não deve ser misturada com correções funcionais sem necessidade.

## 5. DD/FD CB: regra contra a modelagem incorreta

A pesquisa anexada apresenta duas tabelas incompatíveis entre si e uma delas enumera oito M-cycles apesar de declarar seis. Essa decomposição não deve ser implementada.

O manual Zilog confirma a codificação dos bytes `DD CB d op` e publica timings distintos por família: por exemplo, RLC indexed é 6 M / 23 T e BIT indexed é 5 M / 20 T. O documento de modelagem não presume uma distribuição elétrica por byte além do que a fonte comprova. Antes de implementar:
1. extrair a sequência completa de barramento de uma referência independente (Fuse/MAME/Z80Ex) e, quando possível, captura lógica;
2. confrontar cada borda com os diagramas Zilog;
3. registrar separadamente os caminhos BIT (sem write-back) e rotate/shift/RES/SET (com write-back);
4. validar o valor X/Y de BIT indexed contra fonte de engenharia reversa específica, não contra o manual Zilog.

## 6. Estratégia de prototipagem incremental

### Protótipo P0 — Prefix latch isolado

Criar testes unitários puros para a transição do contexto de prefixos, sem tocar no barramento:
- DD -> IX;
- FD -> IY;
- DD FD -> IY;
- FD DD -> IX;
- DD DD FD DD -> IX;
- DD ED -> contexto ED, sem índice ativo.

Critério: prefixo final correto, contagem de M1/R preservada pelo sequenciador e nenhum estado residual após retirada.

### Protótipo P1 — Decoder com cobertura de instruções

Adicionar vetores por família:
- instruções indexadas já implementadas;
- instruções base não afetadas por prefixo;
- ED após DD/FD;
- IXH/IXL/IYH/IYL;
- indexadas com deslocamento;
- CB indexado.

Critério: opcode/operando/flags/PC/R/WZ e número de M/T-states coincidem com oráculo independente.

### Protótipo P2 — Continuação serializável

Para cada transição de P0/P1:
1. executar até cada safe point observável;
2. exportar estado;
3. codificar e decodificar;
4. restaurar em outra instância;
5. continuar ambas com as mesmas respostas de barramento;
6. comparar eventos e estado final.

Incluir corrupção de CRC, magic, versão, truncamento, bytes extras, tags inválidas e combinações impossíveis entre fase e request pendente. A falha de restore não pode alterar o sistema de destino.

### Protótipo P3 — Integração Scheduler/bus

Testar separadamente:
- WAIT no M1 e em ciclos de memória;
- completion latched antes do acknowledgement;
- restore entre completion e acknowledgement;
- restore em cada fase M1 T1/T2/TW/T3/T4;
- ciclo interno suspenso;
- ordem de eventos com timestamp igual e hardware phase.

Critério: replay produz a mesma sequência ordenada de eventos e efeitos de memória exatamente uma vez.

## 7. Matriz de aceitação

| ID | Caso | Critério | Estado atual |
|---|---|---|---|
| PF-01 | DD/FD isolado | Seleção IX/IY | Implementado na fatia |
| PF-02 | Prefixos repetidos | Último prefixo vence; cada M1/R contado | Vetores presentes na PR #447 |
| PF-03 | DD/FD + ED | Índice cancelado e ED executado normalmente | Pendente |
| PF-04 | Prefixo + opcode não afetado | Semântica base preservada, custo prefixado | Pendente |
| IX-01 | LD IX/IY,nn | 14 T + prefixos extras | Vetores presentes na PR #447 |
| IX-02 | INC/DEC IX/IY | 10 T; flags preservadas | Implementado na fatia |
| IX-03 | ADD IX/IY,rr | 15 T; S/Z/PV preservadas; H/N/C corretas | Implementado na fatia; diferencial final pendente |
| IX-04 | WZ em ADD IX/IY | WZ = índice anterior + 1 | Implementado na fatia; oráculo independente pendente |
| CB-01 | DD/FD CB d op | Ordem de bytes e timings por família | Pendente; especificação detalhada em validação |
| CB-02 | BIT indexed | Sem write-back; flags conforme referência validada | Pendente |
| CB-03 | RLC/RES/SET indexed | Read/modify/write e cópia para registrador z quando aplicável | Pendente |
| SS-01 | Snapshot em cada safe point | Continuação idêntica | Round-trip de subset presente; cobertura integral pendente |
| SS-02 | CRC/magic/versão/truncamento/trailing bytes | Rejeição antes de mutação | Codec Z80 possui verificações; máquina completa pendente |
| BUS-01 | WAIT e completion latched | Exactly-once | Bus temporal tem contrato; WAIT físico pendente |
| SYS-01 | CPU+VDP+PSG+I/O+IRQ+Scheduler | Estado persistente integral | Não implementado |

## 8. Regras de engenharia para a implementação

1. Não criar uma segunda CPU Z80 ou um sequenciador paralelo para SMS.
2. Preservar `Z80::step()` como oráculo funcional atômico enquanto o sequenciador é expandido.
3. O núcleo Rust é proprietário de todos os estados digitais e do tempo.
4. O Python não avança T-states, não arbitra barramento e não restaura registradores.
5. Nenhum callback Python por instrução ou por T-state.
6. Não usar JIT, frame skip ou otimizações especulativas.
7. Cada expansão de estado persistido exige versão explícita, validação de invariantes e testes negativos.
8. Nenhuma PR deve ser integrada sem rustfmt, Clippy com `-D warnings`, workspace tests, Scheduler stress e jobs Windows exigidos pelo projeto.
9. `SKIPPED`, `CANCELLED`, execução local não realizada e testes sem oráculo não são PASS.
10. Estados formais do componente devem permanecer explícitos: IMPLEMENTED, QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING e TEMPORAL_PASS.

## 9. Referências

### Primária — fabricante

- Zilog, *Z80 CPU User Manual*, UM008011-0816 (2016). [PDF oficial](https://www.zilog.com/docs/z80/um0080.pdf).
  - Timing: pp. 7–10 (M1, refresh, WAIT e ciclos de memória).
  - Indexed Addressing: pp. 36–37.
  - ADD IX,pp / ADD IY,rr: pp. 194–197.
  - INC IX/IY e DEC IX/IY: pp. 199–203.
  - RLC (IX+d): pp. 217–218.
  - BIT b,(IX+d): pp. 247–248.

### Engenharia reversa e comportamento não documentado

- Sean Young et al., *The Undocumented Z80 Documented*, versão 0.91 — MEMPTR/WZ, flags não documentadas e prefixos.
- Cristian Dinu, *Z80 Instruction Decoding System* — substituições DD/FD por opcode.
- Woodroll / Thomas Scherrer, *Z80 Care Package* — diagramas de barramento e capturas.
 
### Implementações de referência e validação

- Fuse Z80 core — referência de comportamento e sequência de barramento.
- MAME Z80 core — referência independente de emulação.
- Z80Ex — referência de execução orientada a ciclos.
- ZEXDOC/ZEXALL — conformidade funcional; não substituem testes temporais.
- Testes de timing e rastros de barramento — necessários para provar T-state accuracy.

## 10. Veredito

A PR #447 já contém um avanço concreto e testável para um subconjunto de instruções DD/FD e um codec Z80 v18 que preserva fases desse subconjunto. A documentação técnica confirma os timings e regras funcionais de ADD IX/IY e INC/DEC IX/IY e permite construir vetores confiáveis.

O próximo incremento não é reescrever snapshots nem introduzir Bincode. É generalizar explicitamente o contexto de prefixos, implementar cancelamento por ED e instruções indexadas restantes, reconstruir DD/FD CB com rastros confiáveis e ampliar o replay serializado para o estado integrado da máquina. Até que esses gates sejam executados no SHA candidato, manter a classificação global como INTEGRATION_PENDING e não declarar TEMPORAL_PASS.
