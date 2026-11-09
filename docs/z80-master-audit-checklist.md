# Checklist mestre de auditoria e fechamento do Z80

**Documento operacional:** atualizar este checklist a cada etapa efetivamente concluída, sempre vinculando evidências ao SHA integrado na `main`.
**Baseline de runtime Z80 após PR #702:** `401bf4fb1ba8add6c84420cad5c9d2a62ceba72b` (2026-11-09). PRs documentais #703 e #704 não alteraram o runtime. Q/previous-Q DD/FD→SCF/CCF foi corrigido e validado com 2.000 vetores externos adicionais; total SingleStepTests integrado: 34.000 PASS.
**Baseline documental reconciliada:** scorecard, matriz DD/FD e checklist foram atualizados pelas PRs #687 e #688; baselines históricas permanecem preservadas e não representam a main corrente.
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
| F-028 | IXH/IXL/IYH/IYL: LD, INC/DEC, ALU e transferências | AUDIT_PARTIAL | PR #702 MERGED, main `401bf4fb1ba8add6c84420cad5c9d2a62ceba72b`: correção Q/previous-Q nos dois executores. SingleStepTests DD 37/FD 3F: 2.000/2.000 PASS; F-037: 32.000/32.000 PASS. Total 34.000 PASS; runs #42626811588/#42626765522. Permanecem auditoria das demais exceções, prefixos, HALT e estado arquitetural. E4A/E4B preservados. |
| F-029 | (IX+d)/(IY+d): loads, stores e RMW | AUDIT_PARTIAL | Displacement assinado, wrap, exceções H/L e ordem de acesso. |
| F-030 | DDCB/FDCB: BIT/RES/SET/rotate/shift e destinos | AUDIT_PARTIAL | Matriz completa por prefixo, subopcode e destino. |
| F-031 | Prefixos repetidos/alternados DD/FD e last-prefix-wins | AUDIT_PARTIAL | Verificar consumo, PC/R, M1 e semântica terminal. |
| F-032 | Prefixos DD/FD ignorados antes de BASE/CB/ED | AUDIT_PARTIAL | Verificar não refetch, sem substituição indevida e custo prefixado. |
| F-033 | Encodings ED reservados e fallback ED-NOP | GAP_CONFIRMED | O executor atômico reconhece 78 codificações e aplica fallback 8T às outras 178; testes atuais dos 178 são do caminho atômico. No microsequenciador, execute_ed_register_control termina em UnsupportedOpcode para os mesmos bytes. Falta reproduzir no caminho retomável e fechar o fallback NMOS sem ampliar suporte a clones. Matriz completa em docs/z80-ed-opcode-matrix.md. |
| F-034 | Encodings BASE/CB/DD/FD não documentados ou aliases | AUDIT_PARTIAL | Primeira classificação ED identifica aliases NEG, RETN/RETI e IM, ED70 indefinido e ED71 não documentado; falta matriz completa de todos os prefixos, distinção RETI/RETN observável e resolução de variantes. Não inventar semântica nem ampliar hardware. |
| F-035 | RESET e estado arquitetural inicial | AUDIT_PARTIAL | Testes já documentados; revisar campos preservados e contrato de host. |
| F-036 | DD/FD LD r,r' register-only com IXH/IXL/IYH/IYL | CLOSED | PR #684 integrada em `0b9b298f0f9791a6fa6db17580ad741db5c0515d`. CI #42614755935 PASS no HEAD exato `03dbd1552c8eb58ff8f9ef28d9ab0e97b615b4f3`: Z80 focused, docs hygiene e agregador PASS; D3D12/áudio/SDL3/workspace SKIPPED. 48 combinações, oracle de registradores, comparação atômica, 8T/2 M1, WAIT/snapshot, prefixos repetidos e HALT. As exceções 66/6E/74/75 permanecem no caminho E4A. |
| F-037 | DD/FD ALU A,IXH/IXL/IYH/IYL | CLOSED | PR #686 merge SHA e testes internos preservados. Validação externa adicional: PR #692, HEAD `1606556814a8746a445d3ad32fc948ab97f0fde5`, CI #42622828177 PASS; 32 fixtures SingleStepTests × 1.000 = 32.000/32.000 no microsequenciador retomável. Merge SHA/main `1304bc0438e3ea333719d0ba3a495e2db9089da3`. Corpus Ares-derived; não é prova de silício. |

## 4. Checklist de estado arquitetural e flags

| ID | Item | Estado inicial | Próxima ação |
|---|---|---|---|
| S-001 | AF/BC/DE/HL e bancos alternativos | AUDIT_PARTIAL | Cobrir leitura, escrita, EX/EXX e preservação. |
| S-002 | IX/IY e metades de 8 bits | AUDIT_PARTIAL | Matriz completa de uso e exceções. |
| S-003 | SP/PC/I/R | AUDIT_PARTIAL | Wrap, refresh e efeitos de fetch/interrupt. |
| S-004 | IFF1/IFF2, IM e EI delay | AUDIT_PARTIAL | Transições, snapshot e interrupções nas fronteiras. |
| S-005 | WZ/MEMPTR | AUDIT_PARTIAL | Tabela por família com vetores independentes. |
| S-006 | Q/previous-Q | AUDIT_PARTIAL | DD/FD→SCF/CCF RESOLVED pela PR #702; 2.000 vetores externos PASS, incluindo DD 37 e FD 3F, com snapshot/restore. O estado Q/previous-Q global nas demais famílias permanece sujeito à auditoria E7. |
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
| D-004 | WAIT pendente e retomada | CLOSED | PR #680 integrada: testes focados e regressão completa passaram no HEAD `e20ec4b6c858bcaca265114ba543c19301dbb2e0`, run #42609912469; merge SHA `04527a59e0695a800d4fd048d2da85c51126de68`. WAIT e snapshot/restore das operações DD/FD cobertas pela PR foram validados no SHA integrado. |
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
| C-010 | Warnings, lint e testes de workspace | CLOSED | PR #680 e PR #683 passaram nos HEADs exatos. #683 executou regressão ampla após alteração do workflow: workspace, D3D12/DXGI, WASAPI/XAudio2, SDL3, Linux audio, PowerShell, documentação e agregador PASS; merge SHA `43772d5cd5395de273bd3898895cf1deed81aa53`. A validação focada Z80 será comprovada em PR #684. |
| C-011 | Documentação/scorecard/matrizes | GAP_CONFIRMED | Baselines documentais estavam atrás da main; reconciliar por PR. |

## 9. Estado inicial e bloqueios confirmados

### 9.1 Baseline

- Snapshot da `main` imediatamente após a PR documental #681, usada como baseline desta varredura: `db6b4ec6d2f01ddaabccc124fe9ca7d95a856320`.
- Baseline de código usada na abertura do checklist: `b41fa66e01e329d45f7704a229479c8ac3c2c9ea`; a PR #681 foi documental e não alterou runtime.
- Núcleo canônico: `native/z80_cpu`.
- E4C, E5 e E6 aparecem como integradas nos documentos consultados.
- E7.7 continua em andamento; E8 continua bloqueada.
- Scorecard e matriz DD/FD preservam fotografias históricas anteriores; esta auditoria passa a usar `db6b4ec...` como baseline documental atual sem reescrever os resultados antigos.

### 9.2 PR #680 — bloqueio atual

- PR: https://github.com/leonardo201800478/sega-emulator/pull/680
- Branch: `feat/z80-e7-index-half-incdec-load-sequencer-20261109`.
- HEAD candidato atual: `268ece8cc4189bf70f5b1be92967a6d2de5e551f`.
- A falha inicial #5921 (`42572152572`) foi reproduzida: 241 testes passaram e `dd_fd_index_half_inc_dec_and_immediate_load_match_atomic_with_waits` falhou com `InvalidState`.
- Causa isolada: `ImmediateReadRegister` permitia DD/FD 26/2E no validador de fase, mas `indices_valid` aceitava somente prefixos ignorados.
- Correção no commit `48aad1f67b6eb4f03a87f3581249c8a51f6602bb`: aceitar precisamente 26/2E `UsesIndex` na validação de índices, sem relaxar outras invariantes. CI #5924 (`42575151294`) passou no HEAD desse commit: fmt, Clippy Z80/SMS/SG-1000 e testes focados; `sega-z80-cpu` reportou 242/242 testes.
- Commit `ed113041...` passou a acionar a regressão completa para mudanças Z80. CI #5925 (`42575339923`) passou em fmt, Clippy workspace, testes workspace, observability, Linux áudio, Windows áudio, SDL3, D3D12 e PowerShell, mas o merge foi bloqueado porque o job Z80-focused estava SKIPPED e dois checks obrigatórios eram esperados.
- Commit `268ece8...` removeu a exclusão do job Z80-focused quando `full=true`, para executar os dois gates simultaneamente. CI #5929 (`42575886280`) no HEAD atual: Z80-focused PASS (242 testes do crate e integração SMS/SG-1000), Rust workspace PASS (fmt, Clippy workspace, `cargo test --workspace --locked`, observability). Linux áudio, SDL3 e PowerShell PASS; D3D12 e WASAPI/XAudio2 ainda em execução nesta fotografia.
- A PR #680 cobre DD/FD 24,25,26,2C,2D,2E (INC/DEC e LD imediato IXH/IXL/IYH/IYL). Não integrar até CI #5929 concluir com sucesso no HEAD exato e os checks obrigatórios estarem presentes.

### 9.3 Prioridade imediata

1. Diagnosticar e corrigir PR #680 sem ampliar permissivamente o validador de snapshot.
2. Integrar #680 somente com CI verde no HEAD exato.
3. Após a integração, auditar e implementar a próxima família IXH/IXL/IYH/IYL: LD r,r' e operações ALU, preservando exceções H/L quando há (IX+d)/(IY+d).
4. Prosseguir em ordem de prioridade até todas as famílias e efeitos aplicáveis estarem fechados.
5. Somente então desbloquear E8.

### 9.4 Varredura estática DD/FD — lacunas seguintes confirmadas

A inspeção cruzada de `native/z80_cpu/src/prefix.rs`, `cpu.rs` e `sequencer.rs` identificou duas lacunas no caminho retomável:

1. **LD r,r' register-only com metades IX/IY:** o classificador reconhece os opcodes DD/FD 40–7F que usam H/L como `UsesIndex`. O executor atômico possui o caminho correspondente. O microsequenciador, porém, só aceita famílias 16-bit, displacement e os seis opcodes cobertos por #680; os demais `UsesIndex` são rejeitados explicitamente.
2. **ALU com IXH/IXL/IYH/IYL:** os 16 opcodes 80–BF cuja fonte é código 4 ou 5 são classificados como `UsesIndex` e executados pelo caminho atômico, mas ainda não possuem dispatch específico no microsequenciador.

**Exceções obrigatórias:** DD/FD 66, 6E, 74 e 75 com (IX+d)/(IY+d) usam H/L reais, não metades IXH/IXL/IYH/IYL. HALT 76 também não deve ser tratado como LD indexado. O caminho de memória indexada permanece separado.

**Modelo temporal esperado:** são operações register-only. Após os dois M1 (prefixo + opcode), podem atualizar estado e aposentar a instrução sem ciclos de barramento adicionais; o total nominal é 8 T-states com um prefixo, antes de WAIT. Não inventar ciclos internos para elas.

**Profundidade de evidência atômica localizada:**
- `native/sms_core/tests/z80_indexed_half_register_matrix.rs` percorre os opcodes 40–7F para DD e FD, excluindo 76/HALT; verifica formas register-only e memória indexada, incluindo as exceções H/L.
- `native/sms_core/tests/z80_indexed_half_register_alu.rs` cobre os oito operadores ALU com IXH/IXL e carry 0/1, mas usa 13 operandos selecionados; não é uma varredura combinatória de A×operand nem evidência independente de silício.
- Esses testes comprovam a rota atômica nos cenários exercitados. Eles não cobrem o dispatch retomável, WAIT/snapshot no microsequenciador ou todos os estados de entrada das flags.

**Próxima implementação após #680:** adicionar dispatch direto de LD r,r' e ALU half-register, com cobertura por DD/FD, destino/fonte, flags, Q, IX/IY não selecionado, H/L e PC/R, WAIT nos dois M1 e snapshot/restore nos safe points. Comparar com o executor atômico como diferencial interno e complementar com vetores externos independentes.

## 10. Histórico de preenchimento

| Data | SHA da main | Etapa | Resultado | Evidência / PR |
|---|---|---|---|---|
| 2026-11-09 | `b41fa66e01e329d45f7704a229479c8ac3c2c9ea` | Abertura do checklist e auditoria documental inicial | IN_PROGRESS | Scorecard, matriz DD/FD, auditoria ED e PR #680 examinados; detalhes na seção 9. |
| 2026-11-09 | `db6b4ec6d2f01ddaabccc124fe9ca7d95a856320` | Publicação do checklist mestre pela PR #681 e varredura estática inicial DD/FD | AUDIT_PARTIAL | Fotografia histórica; supersedida pelas integrações #680 e #683 registradas abaixo. |
| 2026-11-09 | `04527a59e0695a800d4fd048d2da85c51126de68` | PR #680 integrada: INC/DEC IXH/IXL/IYH/IYL e LD imediato; run #42609912469 PASS no HEAD `e20ec4b6c858bcaca265114ba543c19301dbb2e0` | CLOSED | D-004 fechado; E7 segue em auditoria global. |
| 2026-11-09 | `43772d5cd5395de273bd3898895cf1deed81aa53` | PR #683 integrou seleção inteligente de CI, jobs focados PSG/VDP/input e agregador; run #42613123290 PASS no HEAD `eaa88a249989b2ca6aee931e2a963df75a396008` | CLOSED | PR #637 encerrada como supersedida; CI otimizado integrado. |
| 2026-11-09 | `0b9b298f0f9791a6fa6db17580ad741db5c0515d` | PR #684 integrada: F-036 LD r,r' register-only no microsequenciador; CI #42614755935 PASS no HEAD `03dbd1552c8eb58ff8f9ef28d9ab0e97b615b4f3` | CLOSED | Merge squash confirmado; D3D12/áudio/SDL3 SKIPPED e merge aceito pelo GitHub. |
| 2026-11-09 | `e7e37290e1baa9119326e9e7213f77cadcc37f5e` | PR #686 integrada: F-037 ALU A,IXH/IXL/IYH/IYL no microsequenciador | CLOSED | HEAD validado `a0937e8fa1bb4547d1ee53a9e3431392a2fa443a`; CI #42619522899 PASS; merge squash confirmado. |

### Modelo obrigatório para cada atualização

Adicionar uma linha no histórico e atualizar os IDs afetados:

- Data e SHA da main.
- IDs fechados/reabertos.
- Divergência encontrada e referência técnica.
- Testes executados com classificação individual e SHA.
- PR, HEAD candidato, resultado CI e SHA de merge.
- Pendências remanescentes e próxima tarefa.


## 18. Reconciliação após PR #680/#683 e candidato F-036 — 2026-11-09

**Main remota confirmada:** `43772d5cd5395de273bd3898895cf1deed81aa53`.

- PR #680 foi integrada por squash. HEAD validado `e20ec4b6c858bcaca265114ba543c19301dbb2e0`; execução #42609912469 PASS; merge SHA `04527a59e0695a800d4fd048d2da85c51126de68`. INC/DEC e LD imediato das metades IX/IY estão integrados. As exceções DD/FD 66, 6E, 74 e 75 continuam no caminho de memória indexada e usam H/L reais.
- PR #683 foi integrada por squash. HEAD validado `eaa88a249989b2ca6aee931e2a963df75a396008`; execução #42613123290 PASS; merge SHA `43772d5cd5395de273bd3898895cf1deed81aa53`. O CI agora seleciona Z80, PSG, VDP, input, Scheduler, hosts e tooling por dependências; o agregador diferencia jobs aplicáveis de jobs SKIPPED. A PR #637 foi encerrada como supersedida.
- F-036 foi integrada pela PR #684. HEAD validado `03dbd1552c8eb58ff8f9ef28d9ab0e97b615b4f3`; execução #42614755935 PASS; merge SHA `0b9b298f0f9791a6fa6db17580ad741db5c0515d`.
- A PR #684 comprovou a seleção Z80-only: D3D12, áudio, SDL3, VDP, PSG, Scheduler stress e workspace foram SKIPPED; Z80 focused, documentação e agregador passaram. O merge foi aceito pelo GitHub apesar do job D3D12 ignorado, confirmando o comportamento do check condicional.
- F-037 permanece GAP_CONFIRMED e será implementada em PR separada após F-036.
- E7.7 permanece IN_PROGRESS; E8/ZEXDOC/ZEXALL permanece BLOCKED. Não iniciar E8.


## 19. Fechamento F-036 — 2026-11-09

- **PR:** #684 — `feat/z80-e7-indexed-register-ld-20261109`.
- **HEAD validado:** `03dbd1552c8eb58ff8f9ef28d9ab0e97b615b4f3`.
- **Execução final:** #42614755935 — PASS. Z80 focused PASS; Documentation patch hygiene PASS; Rust quality and unit regression PASS. Jobs não aplicáveis foram SKIPPED.
- **Falhas intermediárias:** #42614347670 FAIL apenas em rustfmt; #42614617507 FAIL em Clippy por `field_reassign_with_default` no teste. Ambas corrigidas; não foram promovidas a PASS. A execução final no HEAD exato passou.
- **Merge SHA / main atual:** `0b9b298f0f9791a6fa6db17580ad741db5c0515d`.
- **Prova do CI seletivo:** D3D12, WASAPI/XAudio2, Linux audio, SDL3, VDP, PSG, Scheduler stress e workspace ficaram SKIPPED em uma PR exclusivamente Z80. O merge squash foi aceito pelo GitHub; o check condicional D3D12 não bloqueou.
- **F-036:** CLOSED. **F-037:** GAP_CONFIRMED. **F-028:** AUDIT_PARTIAL. **E7.7:** IN_PROGRESS. **E8:** BLOCKED.

A próxima tarefa é F-037 — ALU A,IXH/IXL/IYH/IYL — em PR independente. Não iniciar ZEXDOC/ZEXALL.


## 20. Candidato F-037 — ALU com metades IX/IY — 2026-11-09

- **PR:** #686 — `feat/z80-e7-indexed-half-alu-20261109`.
- **Main de entrada:** `5aaa74af3fbf6fa7f70ecb1e6b71f3a966bb91e4`.
- **Commit de implementação:** `5f9ca825feb45633c2baf200e502f1a05bf99c7d`; último commit de teste/otimização antes desta atualização documental: `2ef8d4c0c73cf5a339f8d258ba9e314bfeebcdd2`. O HEAD final da PR será consultado no GitHub antes do merge.
- O dispatch cobre as 16 combinações DD/FD de ADD/ADC/SUB/SBC/AND/XOR/OR/CP com IXH/IXL/IYH/IYL, reutilizando a ALU compartilhada. O operando código 6 permanece no caminho de memória indexada E4B.
- A suíte `indexed_half_register_alu_e7.rs` contém 10.816 casos diferenciais, vetores fixos para flags completas e X/Y de CP, preservação de A/IX/IY/H/L, Q/previous-Q, PC/R/WZ, WAIT nos dois M1, snapshot/restore e prefixos repetidos.
- **Estado:** IMPLEMENTED_PENDING_VALIDATION. Nenhuma suíte da PR #686 é PASS até o resultado no HEAD final. F-036 permanece CLOSED; F-028 permanece AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.

A próxima ação é validar o HEAD final da PR #686. Não iniciar E8.


## 21. Fechamento F-037 — 2026-11-09

- **PR:** #686 — `feat/z80-e7-indexed-half-alu-20261109`.
- **HEAD validado:** `a0937e8fa1bb4547d1ee53a9e3431392a2fa443a`.
- **CI final:** run #42619522899 PASS no HEAD exato. Change impact classification, Z80 focused validation, documentation hygiene e agregador PASS. D3D12, áudio, SDL3, VDP, PSG, Scheduler stress, PowerShell, input e workspace SKIPPED por não aplicabilidade.
- **Falha intermediária:** run #42615731297 FAIL somente por rustfmt no novo teste; corrigida antes do HEAD final. Não foi promovida a PASS.
- **Merge SHA / main atual:** `e7e37290e1baa9119326e9e7213f77cadcc37f5e`.
- **Cobertura:** 10.816 casos diferenciais; vetores fixos de A/F e CP X/Y; carry-in; Q/previous-Q; PC/R/WZ/IFF/I; IX/IY/H/L; WAIT nos dois M1; snapshot/restore após prefixo; DD/FD repetidos/alternados; 8T nominal para prefixo único e sem ciclos internos artificiais.
- **Limite de evidência:** comparação com executor atômico é diferencial interno. Vetores fixos têm expected A/F explícitos e independentes do resultado do caminho microsequenciado, mas a suíte ainda não importa o corpus externo SingleStepTests. A validação externa permanece pendente na auditoria mestre E7.

**Estado:** F-036 CLOSED; F-037 CLOSED; F-028 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.

**Próxima tarefa:** auditoria residual Z80 orientada por evidência externa e cobertura semântica/temporal. Não iniciar ZEXDOC/ZEXALL.


## 22. Reconciliação da baseline documental — PR #688

- Main de entrada: `4d8a862f254097e9dd1d0d1640edcd74f40ab12c`.
- Scorecard e matriz DD/FD tinham cabeçalhos com baselines antigas apesar de conterem os fechamentos #684/#686 em seções posteriores.
- Esta PR atualiza os cabeçalhos para a baseline corrente, mantendo as baselines anteriores como histórico e sem alegar reexecução de testes antigos.
- Estado semântico: F-036 CLOSED; F-037 CLOSED; F-028 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.


## Atualização de evidências — PR #692 (2026-11-09)

- Baseline main: `1304bc0438e3ea333719d0ba3a495e2db9089da3`.
- F-037: 32.000 vetores SingleStepTests PASS no HEAD `1606556814a8746a445d3ad32fc948ab97f0fde5`, run `4262282817`.
- F-028: nova hipótese de divergência Q/previous-Q documentada; sem correção implementada antes de reproduzir.
- F-033: divergência de cobertura entre executor atômico e microsequenciador identificada; teste retomável pendente.
- F-034: classificação inicial ED iniciada; inventário global permanece parcial.
- E7 permanece IN_PROGRESS. E8/ZEXDOC/ZEXALL permanece BLOCKED.


## 24. Reconciliação após PR #702 — Q/previous-Q

- Main: `401bf4fb1ba8add6c84420cad5c9d2a62ceba72b`.
- PR #702 MERGED; HEAD validado `34e85637eaac8ed2777ecc5cb9cd2e91ba137235`.
- Runs #42626811588 e #42626765522: CI SUCCESS.
- F-028: DD/FD→SCF/CCF corrigido no executor atômico e no microsequenciador. 2.000 vetores SingleStepTests PASS; snapshot/restore entre prefixo e opcode terminal PASS.
- F-037: 32.000 vetores SingleStepTests PASS no mesmo HEAD. Total externo integrado: 34.000/34.000.
- F-028 permanece AUDIT_PARTIAL; F-033 GAP_CONFIRMED; F-034 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.
