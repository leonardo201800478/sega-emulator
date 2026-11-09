# Checklist mestre de auditoria e fechamento do Z80

**Documento operacional:** atualizar este checklist a cada etapa efetivamente concluída, sempre vinculando evidências ao SHA integrado na `main`.
**Baseline de código desta abertura:** `b41fa66e01e329d45f7704a229479c8ac3c2c9ea` (2026-11-09).
**Baseline documental encontrada:** scorecard e matriz DD/FD ainda apontavam para `043ecb61d8d1cc77245fa72639d7f210d88cab26`; a divergência deve ser reconciliada nesta trilha.
**Núcleo canônico:** `native/z80_cpu` (`sega-z80-cpu`).
**Perfil inicial de referência:** Zilog Z80 NMOS usado pelas plataformas Sega documentadas. Não generalizar automaticamente para Z80 CMOS, Z180, R800 ou clones.
**Estado global:** `IN_PROGRESS`. E8/ZEXDOC/ZEXALL permanece bloqueada até o fechamento formal de E7.

> Este documento é um inventário de trabalho, não uma declaração de conformidade. Código presente, decoder sem erro, smoke test, igualdade entre dois caminhos internos ou execução de jogos não bastam isoladamente para marcar um item como concluído.

## 1. Estados e regras de evidência

### 1.1 Estado da auditoria de cada item

| Estado | Significado |
|---|---|
| `NOT_AUDITED` | O item ainda não recebeu inspeção completa e rastreável. |
| `AUDIT_PARTIAL` | Há inspeção/evidência parcial; faltam casos, variantes ou fontes. |
| `GAP_CONFIRMED` | Divergência ou ausência reproduzível identificada. |
| `IMPLEMENTED_PENDING_VALIDATION` | Existe implementação candidata, mas falta validação/gate. |
| `CLOSED` | Critérios de aceite satisfeitos e alteração integrada na main. |
| `BLOCKED` | Não é possível avançar por dependência ou falta de evidência identificada. |
| `NOT_APPLICABLE` | Item não se aplica ao perfil de hardware explicitamente delimitado, com justificativa. |

### 1.2 Classificação obrigatória de execução

Cada suíte/comando executado recebe exatamente um estado:

- `PASS`: executado no SHA indicado e aprovado.
- `FAIL`: executado no SHA indicado e falhou.
- `BLOCKED`: execução impedida por dependência/ambiente identificados.
- `NOT RUN`: não executado.

`SKIPPED` nunca equivale a `PASS`. Não transferir resultados de outro SHA.

### 1.3 Evidência mínima para fechar uma família

1. Inventário de bytes/opcodes e classificação de instruções documentadas, aliases, prefixos ignorados e encodings reservados.
2. Inspeção do decoder e do executor real, inclusive fallbacks e caminhos de erro.
3. Testes de resultado arquitetural: registradores, flags, PC, SP, R, I, WZ/MEMPTR, Q, memória e I/O conforme aplicável.
4. Vetores de fronteira e varreduras exaustivas quando o domínio permitir.
5. Diferencial contra referência independente, registrando proveniência e limitações. O executor atômico do próprio projeto pode ser comparador interno, não oráculo independente.
6. Validação temporal somente na granularidade exigida por efeitos observáveis da plataforma: transações, ordenação, T-states, WAIT, interrupções e periféricos.
7. Snapshot/restore e equivalência contínua versus retomada nos pontos suspensíveis aplicáveis.
8. Regressão SMS e SG-1000 e gates de qualidade no HEAD exato.
9. Atualização deste checklist, scorecard, matriz aplicável e relatório da auditoria.

### 1.4 Regra de granularidade temporal

A semântica funcional correta é obrigatória para toda instrução suportada. Não se deve criar microfases artificiais para operações internas sem efeito intermediário observável. Microsequenciamento e traces temporais são obrigatórios onde a ordem/duração da interação com barramento, I/O, VDP/PSG, mapper, IRQ/NMI, HALT, WAIT ou scheduler altera o resultado observável. Não declarar precisão elétrica ou de pinos sem evidência específica.

## 2. Ordem de auditoria e prioridade

A sequência abaixo é uma ordem inicial de trabalho por frequência esperada e impacto em software de console, não uma medição estatística de jogos. Ajustar a prioridade quando houver dados confiáveis de perfis de execução. Nenhum item é descartado por ser raro.

| Ordem | Prioridade | Grupo |
|---:|---|---|
| 1 | P0 — muito alta | Loads, transferências e registradores de 8 bits |
| 2 | P0 — muito alta | ALU de 8 bits, flags e operações lógicas |
| 3 | P0 — muito alta | INC/DEC, pares de 16 bits e aritmética |
| 4 | P1 — alta | Fluxo de controle, chamadas, retornos e pilha |
| 5 | P1 — alta | CB: rotações, shifts e manipulação de bits |
| 6 | P1 — alta | DD/FD: IX/IY, substituições e exceções H/L |
| 7 | P1 — alta | Memória indexada, DDCB/FDCB e prefixos mistos |
| 8 | P1 — alta | I/O e efeitos observáveis de barramento |
| 9 | P2 — média | Interrupções, HALT, EI/DI, IM e registradores especiais |
| 10 | P2 — média | ED aritmético, transferências e aliases |
| 11 | P2 — média | ED operações de bloco e repetição |
| 12 | P2 — transversal | Estado, snapshots, determinismo e integração temporal |
| 13 | P3 — fechamento | Instruções não documentadas, variantes e auditoria final de cobertura |

## 3. Checklist de cobertura funcional

**Nota:** `CÓDIGO PRESENTE` não significa `CLOSED`. Os estados abaixo são a fotografia inicial baseada na inspeção documental/código da baseline; devem ser promovidos somente após a revisão específica e validação da etapa.

| ID | Item a examinar | Estado inicial | Evidência / próxima ação |
|---|---|---|---|
| F-001 | LD r,r', LD r,n, LD r,(HL), LD (HL),r/n | AUDIT_PARTIAL | Caminhos BASE existem; auditar matriz completa, flags preservadas, PC e transações. |
| F-002 | LD A,(BC)/(DE), LD (BC)/(DE),A | NOT_AUDITED | Conferir WZ/MEMPTR, wrap e ordem de acesso. |
| F-003 | LD A,(nn), LD (nn),A | NOT_AUDITED | Conferir endereços, wrap 16-bit e ordem low/high. |
| F-004 | LD rr,nn e transferências entre pares | AUDIT_PARTIAL | Conferir todos os pares e efeitos colaterais. |
| F-005 | LD SP,HL/IX/IY; LD (nn),SP; LD SP,(nn) quando aplicável | AUDIT_PARTIAL | Confirmar semântica e tempos nominais. |
| F-006 | EX DE,HL; EX AF,AF'; EXX; EX (SP),HL/IX/IY | AUDIT_PARTIAL | Conferir bancos alternativos, ordem de barramento e WZ. |
| F-007 | PUSH/POP de todos os pares, inclusive AF e IX/IY | AUDIT_PARTIAL | Conferir ordem da pilha, wrap e flags. |
| F-008 | ADD/ADC/SUB/SBC de 8 bits | AUDIT_PARTIAL | Vetores discriminantes de S/Z/H/PV/N/C/X/Y. |
| F-009 | AND/OR/XOR/CP e CP sem alteração de A | AUDIT_PARTIAL | Conferir flags, X/Y e preservações. |
| F-010 | INC/DEC de registradores e memória | AUDIT_PARTIAL | Exaustividade de valores e preservação de carry. |
| F-011 | ADD HL,rr e variantes IX/IY | AUDIT_PARTIAL | Carry/half-carry, WZ e flags preservadas. |
| F-012 | ADC/SBC HL,rr | AUDIT_PARTIAL | Todos os pares, carry de entrada e fronteiras. |
| F-013 | INC/DEC de pares de 16 bits | AUDIT_PARTIAL | Wraparound e flags preservadas. |
| F-014 | DAA, CPL, NEG, CCF e SCF | AUDIT_PARTIAL | Fórmulas e flags X/Y/Q; NEG inclui aliases ED. |
| F-015 | RLCA/RRCA/RLA/RRA | NOT_AUDITED | Conferir flags e Q. |
| F-016 | RLC/RRC/RL/RR/SLA/SRA/SLL/SRL | AUDIT_PARTIAL | Matriz CB para todos os destinos aplicáveis. |
| F-017 | BIT/RES/SET para registradores e (HL) | AUDIT_PARTIAL | Flags BIT, ausência de write-back e endereços. |
| F-018 | JP/JR/DJNZ, condicional e incondicional | AUDIT_PARTIAL | PC, offsets assinados, wrap e tempos tomados/não tomados. |
| F-019 | CALL/RET/RST e variantes condicionais | AUDIT_PARTIAL | Pilha, PC, tempos e condições. |
| F-020 | IN A,(n), OUT (n),A e ED IN/OUT (C) | AUDIT_PARTIAL | Portas, valor, flags, WZ e efeitos colaterais. |
| F-021 | INI/IND/INIR/INDR e OUTI/OUTD/OTIR/OTDR | AUDIT_PARTIAL | Flags, B, HL, WZ, L pós-atualização, repetição e evidência NMOS. |
| F-022 | LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR | AUDIT_PARTIAL | Flags, contadores, repetição, PC/WZ e fronteiras. |
| F-023 | RRD/RLD | AUDIT_PARTIAL | Nibbles, memória, flags, WZ e WAIT/snapshot se caminho temporal aplicável. |
| F-024 | LD I,A; LD R,A; LD A,I; LD A,R | AUDIT_PARTIAL | Flags, refresh, PV/IFF2 e erratum após interrupção. |
| F-025 | IM 0/1/2, RETN e RETI | AUDIT_PARTIAL | Aliases, retorno, IFF1/IFF2 e notificação externa RETI quando exigida. |
| F-026 | HALT, NOP e estado após HALT | AUDIT_PARTIAL | Fetch/refresh, R, saída por interrupção e snapshot. |
| F-027 | DD/FD substituição HL por IX/IY | AUDIT_PARTIAL | Matriz por opcode e exceções. |
| F-028 | IXH/IXL/IYH/IYL: LD, INC/DEC, ALU e transferências | IMPLEMENTED_PENDING_VALIDATION | PR #680 inicia INC/DEC e LD imediato; ainda não integrada. Cobrir também LD r,r' e ALU. |
| F-029 | (IX+d)/(IY+d): loads, stores e RMW | AUDIT_PARTIAL | Displacement assinado, wrap, exceções H/L e ordem de acesso. |
| F-030 | DDCB/FDCB: BIT/RES/SET/rotate/shift e destinos | AUDIT_PARTIAL | Matriz completa por prefixo, subopcode e destino. |
| F-031 | Prefixos repetidos/alternados DD/FD e last-prefix-wins | AUDIT_PARTIAL | Verificar consumo, PC/R, M1 e semântica terminal. |
| F-032 | Prefixos DD/FD ignorados antes de BASE/CB/ED | AUDIT_PARTIAL | Verificar não refetch, sem substituição indevida e custo prefixado. |
| F-033 | Encodings ED reservados e fallback ED-NOP | GAP_CONFIRMED | Auditoria estática reporta 78 bytes explícitos e 178 em fallback. Confirmar cada byte contra perfil e referência; não presumir que smoke test prova semântica. |
| F-034 | Encodings BASE/CB/DD/FD não documentados ou aliases | NOT_AUDITED | Classificar explicitamente documentado, alias, não documentado, ignorado ou reservado. |
| F-035 | RESET e estado arquitetural inicial | AUDIT_PARTIAL | Testes já documentados; revisar campos preservados e contrato de host. |

## 4. Checklist de estado arquitetural e flags

| ID | Item | Estado inicial | Próxima ação |
|---|---|---|---|
| S-001 | AF/BC/DE/HL e bancos alternativos | AUDIT_PARTIAL | Cobrir leitura, escrita, EX/EXX e preservação. |
| S-002 | IX/IY e metades de 8 bits | AUDIT_PARTIAL | Matriz completa de uso e exceções. |
| S-003 | SP/PC/I/R | AUDIT_PARTIAL | Wrap, refresh e efeitos de fetch/interrupt. |
| S-004 | IFF1/IFF2, IM e EI delay | AUDIT_PARTIAL | Transições, snapshot e interrupções nas fronteiras. |
| S-005 | WZ/MEMPTR | AUDIT_PARTIAL | Tabela por família com vetores independentes. |
| S-006 | Q/previous-Q | AUDIT_PARTIAL | Regras por instrução que altera/não altera flags. |
| S-007 | S/Z/H/PV/N/C | AUDIT_PARTIAL | Fórmulas e fronteiras por família. |
| S-008 | X/Y e flags não documentadas | AUDIT_PARTIAL | Proveniência NMOS/CMOS e vetores discriminantes. |
| S-009 | Preservação de flags | NOT_AUDITED | Verificar instruções que preservam total ou parcialmente F. |
| S-010 | Estado interno e campos derivados | NOT_AUDITED | Verificar invariantes e ausência de duplicidade de fonte de verdade. |

## 5. Checklist temporal, barramento e interrupções

| ID | Item | Estado inicial | Próxima ação |
|---|---|---|---|
| T-001 | M1, opcode fetch e refresh | AUDIT_PARTIAL | Confirmar integração real entre M1 e sequencer. |
| T-002 | Memory Read/Write | AUDIT_PARTIAL | Ordenação, endereço/dado, duração e retomada. |
| T-003 | Port Read/Write | AUDIT_PARTIAL | Fronteira observável de I/O com VDP/PSG. |
| T-004 | INT acknowledge | AUDIT_PARTIAL | Revisar operação, vetor, WAIT, M1/IORQ e IM0/1/2. |
| T-005 | NMI | AUDIT_PARTIAL | Prioridade, acknowledge, pilha, PC e snapshot. |
| T-006 | HALT e amostragem de interrupção | AUDIT_PARTIAL | Fronteiras de aceitação e refresh. |
| T-007 | WAIT=0/1/múltiplo | AUDIT_PARTIAL | Matriz por classe de transação e ciclo suspenso. |
| T-008 | Ciclos internos e T-states nominais | AUDIT_PARTIAL | Conferir somente ciclos documentados/observáveis; não inventar fases. |
| T-009 | Integração com scheduler e clocks | AUDIT_PARTIAL | Confirmar causalidade CPU/VDP/PSG e execução em blocos. |
| T-010 | Mapper e efeitos de escrita em memória | NOT_AUDITED | Identificar dependências concretas de timing do console. |
| T-011 | Trace temporal comparável | NOT_AUDITED | Definir cobertura proporcional a cada consumidor real; sem alegar trace elétrico completo. |

## 6. Snapshot, restore e determinismo

| ID | Item | Estado inicial | Próxima ação |
|---|---|---|---|
| D-001 | Codec versionado e compatibilidade de schema | AUDIT_PARTIAL | Conferir evolução, rejeição e migração. |
| D-002 | Snapshot em cada fase do sequenciador | AUDIT_PARTIAL | Matriz por fase e payload. |
| D-003 | Request pendente e identidade | AUDIT_PARTIAL | Preservar request ID, operação, endereço e dados. |
| D-004 | WAIT pendente e retomada | GAP_CONFIRMED | PR #680 possui falha reportada em restore durante WAIT; diagnosticar antes de integrar. |
| D-005 | Estados inválidos/inconsistentes | AUDIT_PARTIAL | Validar todos os campos e combinações por fase. |
| D-006 | Execução contínua vs. step/resume | AUDIT_PARTIAL | Comparar estado completo e sequência de transações. |
| D-007 | Execução em blocos de tamanhos diferentes | NOT_AUDITED | Testar equivalência determinística global aplicável. |
| D-008 | Save state de máquina SMS/SG-1000 | NOT_AUDITED | Distinguir snapshot CPU de estado integral de máquina/scheduler. |

## 7. Testes, referências e regressão

| ID | Item | Estado inicial | Próxima ação |
|---|---|---|---|
| V-001 | Inventário individual por opcode/variante | AUDIT_PARTIAL | Gerar matriz executável e detectar bytes sem teste semântico. |
| V-002 | Vetores externos fixados e versionados | AUDIT_PARTIAL | Registrar fonte, revisão, licença, hash e limites. |
| V-003 | Diferencial independente | NOT_AUDITED | Separar executor interno, corpus derivado de emulador e evidência de hardware. |
| V-004 | Exaustividade ALU/CB | AUDIT_PARTIAL | Conferir domínios e condições efetivamente varridos. |
| V-005 | Cobertura semântica positiva ED | AUDIT_PARTIAL | Expandir casos e distinguir aliases/fallbacks. |
| V-006 | Matriz de prefixos e indexed-CB | AUDIT_PARTIAL | Cobertura por combinação e resultado, não apenas classificação. |
| V-007 | Regressão Master System | AUDIT_PARTIAL | Gates e testes integrados no SHA candidato. |
| V-008 | Regressão SG-1000 | AUDIT_PARTIAL | Confirmar preservação da máquina legada compartilhando Z80. |
| V-009 | ZEXDOC/ZEXALL | BLOCKED | Não iniciar antes do fechamento E7; depois executar em harness Rust identificado. |
| V-010 | Testes em jogos | COMPLEMENTAR | Evidência de produto, não gate primário de opcode individual. |

## 8. Auditoria estrutural do código

| ID | Item | Estado inicial | Próxima ação |
|---|---|---|---|
| C-001 | Decoder BASE/CB/ED/DD/FD | AUDIT_PARTIAL | Mapear cada ramo, fallback e erro. |
| C-002 | Executor atômico vs. microsequenciador | AUDIT_PARTIAL | Comparar semântica e divergências de cobertura. |
| C-003 | Helpers ALU e flags | AUDIT_PARTIAL | Verificar fórmula, fonte e reutilização. |
| C-004 | Prefix FSM e classificação | AUDIT_PARTIAL | Cobrir cadeias, ignorados e último prefixo. |
| C-005 | Fases e transições do sequenciador | AUDIT_PARTIAL | Procurar estados inalcançáveis, transições ausentes e requests órfãos. |
| C-006 | Validadores e codec de snapshot | AUDIT_PARTIAL | Confrontar validador com todos os estados produzidos. |
| C-007 | Contadores temporais e overflow | NOT_AUDITED | Conferir aritmética checked e invariantes. |
| C-008 | Duplicidade de CPU por plataforma | AUDIT_PARTIAL | Confirmar uso do crate canônico e evitar divergência SMS/SG-1000. |
| C-009 | Testes tautológicos/hipóteses compartilhadas | AUDIT_PARTIAL | Identificar comparações que não são independentes. |
| C-010 | Warnings, lint e testes de workspace | NOT RUN | Executar gates no HEAD final de cada PR. |
| C-011 | Documentação/scorecard/matrizes | GAP_CONFIRMED | Baselines documentais estavam atrás da main; reconciliar por PR. |

## 9. Estado inicial e bloqueios confirmados

### 9.1 Baseline

- `main` consultada: `b41fa66e01e329d45f7704a229479c8ac3c2c9ea`.
- Núcleo canônico: `native/z80_cpu`.
- E4C, E5 e E6 aparecem como integradas nos documentos consultados.
- E7.7 continua em andamento; E8 continua bloqueada.
- Scorecard e matriz DD/FD consultados nesta abertura ainda citavam a baseline `043ecb61...`; a atualização documental precisa registrar o SHA real e reconciliar as diferenças sem reescrever evidência histórica.

### 9.2 PR #680 — bloqueio atual

- PR: https://github.com/leonardo201800478/sega-emulator/pull/680
- Branch: `feat/z80-e7-index-half-incdec-load-sequencer-20261109`.
- HEAD consultado: `a01ff76fb55f409f071ce975f885b3fb3d6c7fe3`.
- CI #5921 / run `42572152572`: `FAIL`.
- Jobs `Z80 focused validation` e `Rust quality and unit regression`: `FAIL`.
- Outros jobs aparecem `SKIPPED`; não contar como PASS.
- A descrição da PR cobre DD/FD 24,25,26,2C,2D,2E (INC/DEC e LD imediato IXH/IXL/IYH/IYL), incluindo testes diferenciais internos, WAIT e snapshot.
- O histórico de trabalho reporta falha em restore de snapshot durante WAIT no teste `dd_fd_index_half_inc_dec_and_immediate_load_match_atomic_with_waits`. Os logs detalhados do job não estavam recuperáveis nesta consulta; a causa exata ainda precisa ser reproduzida/isolada. Não integrar a PR enquanto isso não for resolvido e todos os gates obrigatórios passarem.

### 9.3 Prioridade imediata

1. Diagnosticar e corrigir PR #680 sem ampliar permissivamente o validador de snapshot.
2. Integrar #680 somente com CI verde no HEAD exato.
3. Após a integração, auditar e implementar a próxima família IXH/IXL/IYH/IYL: LD r,r' e operações ALU, preservando exceções H/L quando há (IX+d)/(IY+d).
4. Prosseguir em ordem de prioridade até todas as famílias e efeitos aplicáveis estarem fechados.
5. Somente então desbloquear E8.

## 10. Histórico de preenchimento

| Data | SHA da main | Etapa | Resultado | Evidência / PR |
|---|---|---|---|---|
| 2026-11-09 | `b41fa66e01e329d45f7704a229479c8ac3c2c9ea` | Abertura do checklist e auditoria documental inicial | IN_PROGRESS | Scorecard, matriz DD/FD, auditoria ED e PR #680 examinados; detalhes na seção 9. |

### Modelo obrigatório para cada atualização

Adicionar uma linha no histórico e atualizar os IDs afetados:

- Data e SHA da main.
- IDs fechados/reabertos.
- Divergência encontrada e referência técnica.
- Testes executados com classificação individual e SHA.
- PR, HEAD candidato, resultado CI e SHA de merge.
- Pendências remanescentes e próxima tarefa.
