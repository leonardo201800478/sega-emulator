# Auditoria consolidada dos espaços de opcode Z80 — SMS

**Data da auditoria:** 2026-10-28  
**Base examinada:** `main` em `6649ffb6dd6ac8fa78c6886df6157de7d0045411`  
**Escopo:** inventário documental do decoder Rust ativo em `native/sms_core/src/cpu.rs`, dos testes de referência e da matriz de cobertura existente. Esta alteração é documental; não modifica o runtime nem declara concluída a validação semântica de toda a CPU.

## 1. Conclusão executiva

O núcleo possui caminhos de decodificação para os espaços base, CB, ED, DD/FD e DDCB/FDCB. Os testes existentes incluem smoke tests de classificação dos bytes e vetores semânticos dirigidos. **Classificar todos os bytes não equivale a validar semanticamente todos os opcodes**, e validar um opcode em um estado inicial não prova todos os seus estados, flags ou condições.

A suíte externa contabilizada pela matriz atual tem **162 vetores fixos**: 108 vetores gerais e 54 vetores de I/O em bloco. Essa contagem não significa 162 opcodes únicos, não é uma matriz combinatória e não demonstra fidelidade física.

| Espaço de decodificação | Bytes/segundos bytes possíveis | Evidência de classificação registrada | Evidência semântica observada | Conclusão |
|---|---:|---|---|---|
| Base | 256 | Matriz existente registra execução dos 256 bytes no teste geral de classificação | Vetores dirigidos para famílias como loads, aritmética, saltos, pilha, flags, bancos alternativos e controle de fluxo | Classificação registrada; validação semântica completa **não demonstrada** |
| CB | 256 | Matriz existente registra execução dos 256 segundos bytes | Exemplos dirigidos para rotação, BIT e RES, incluindo registrador e memória | Classificação registrada; cobertura semântica ampla **pendente** |
| ED | 256 | Testes percorrem todos os segundos bytes; há teste dedicado para o fallback | 78 codificações explícitas agrupadas em famílias; vetores para aritmética 16-bit, registradores especiais, I/O, operações de bloco, aliases e nibble | 78 codificações explícitas têm cobertura dirigida por família; 178 usam fallback ED-NOP. Isso não prova equivalência em silício para bytes reservados |
| DD/FD | 256 segundos bytes por prefixo | Matriz existente registra execução das codificações indexadas no teste geral | Exemplos para IX/IY, loads indexados, operações aritméticas, substituição de registradores e indexed-CB | Regras de prefixo, aliases e combinações precisam de cobertura semântica mais profunda |
| DDCB/FDCB | 256 bytes finais por prefixo indexado | Matriz existente registra teste de classificação para IX e IY | Vetores dirigidos para BIT indexado, flags e WZ; implementação contém rotate/shift, BIT, RES e SET sobre memória | Classificação registrada; falta validar todas as operações, destinos, flags e estados |

**Importante:** DD/FD não representa 512 novas instruções independentes em todos os casos. Muitas instruções ignoram o prefixo e voltam ao decoder base, enquanto outras substituem HL/H/L por IX/IY/IXH/IXL ou IYH/IYL. A auditoria deve distinguir codificação, semântica efetiva e custo adicional do prefixo.

## 2. Espaço base — plano de conclusão semântica

O decoder base deve ser auditado por família, registrando para cada byte: forma de decodificação, operandos, efeitos de registradores e memória, flags, PC/SP, WZ/MEMPTR quando aplicável, R, T-states nominais e estados iniciais discriminantes.

| Família | Casos de aceitação mínimos | Lacunas que não devem ser presumidas resolvidas |
|---|---|---|
| Loads de 8/16 bits | registradores, imediato, indireto, little-endian, preservação de flags e wraparound | todas as combinações de origem/destino e limites de endereço |
| INC/DEC e ALU 8-bit | 00/01/7F/80/FF, carry de entrada, half-carry, overflow e preservação de C quando aplicável | varredura sistemática de valores e todos os destinos |
| Aritmética 16-bit | carry/borrow, half-carry, overflow, limites 0000/FFFF | pares de registradores e flags de entrada em múltiplas combinações |
| Rotações, shifts, DAA e flags | bits extremos, carry inicial, paridade, flags preservadas/alteradas | sequências dependentes do latch Q e bits X/Y não documentados |
| Saltos e loops | condição tomada/não tomada, deslocamentos -128/+127, wraparound de PC | todas as condições e fronteiras para cada opcode |
| CALL/RET/RST e pilha | ordem de bytes, SP em 0000/FFFF, endereços de retorno e WZ | sequência de barramento ordenada e todas as condições |
| EX/EXX e bancos alternativos | AF'/BC'/DE'/HL', preservação de estado e Q | sequências combinadas e interações com interrupções |
| HALT, DI/EI e interrupções | atraso de EI, NMI, INT, IFF1/IFF2, modos IM0/1/2, Rollover de R | oráculo independente e sequência temporal de acknowledge/stack |

## 3. CB — cobertura exigida para os 256 bytes

O espaço CB deve ser avaliado por operação e destino. Para cada operação, testar destinos B/C/D/E/H/L/(HL)/A, com exceções documentadas da codificação, e estados de entrada que discriminem flags.

| Grupo CB | Evidência mínima por operação | Efeitos a comparar |
|---|---|---|
| RLC/RRC/RL/RR | bit 7/bit 0 definidos e limpos; carry de entrada para RL/RR | resultado, C, S/Z/PV/X/Y, memória ou registrador |
| SLA/SRA/SLL/SRL | MSB/LSB e padrões 00/01/80/FF | resultado, carry, flags e comportamento da variante-alvo para SLL |
| BIT 0–7 | bit testado definido e limpo; alvo registrador e (HL) | Z/PV/H/N, C preservado, S para BIT 7 e origem de X/Y |
| RES/SET 0–7 | bit previamente definido/limpo | resultado e preservação das flags |
| (HL) | endereços e dados de fronteira | leitura/escrita real, ausência de escrita em BIT e wraparound relevante |

A existência de um teste para `RLC B`, `RLC (HL)`, `BIT 7,(HL)`, `BIT 0,(HL)` e `RES 0,(HL)` comprova apenas esses casos dirigidos; não substitui os casos restantes.

## 4. ED — classificação explícita e fallback

A auditoria ED existente identifica 78 codificações distintas explicitamente tratadas e 178 que caem no retorno de 8 T-states. As famílias explícitas registradas incluem:

- `IN r,(C)` e `OUT (C),r` / `OUT (C),0`;
- `SBC HL,rr` e `ADC HL,rr`;
- `LD (nn),rr` e `LD rr,(nn)`;
- `NEG`, `RETN/RETI` e aliases de `IM`;
- `LD I,A`, `LD R,A`, `LD A,I` e `LD A,R`;
- `RRD/RLD`;
- `LDI/LDD/LDIR/LDDR`, `CPI/CPD/CPIR/CPDR`;
- `INI/IND/INIR/INDR` e `OUTI/OUTD/OTIR/OTDR`.

O fallback é comportamento da implementação para a variante-alvo, não evidência universal de todos os Z80 e clones. A política ED-NOP deve continuar testada separadamente das instruções com handler explícito.

### Divergência de flags de I/O em bloco

O manual oficial ZiLOG documenta C como não afetado para INI/INIR/IND/INDR e descreve algumas flags como desconhecidas. A referência secundária *The Undocumented Z80 Documented* fornece fórmulas mais específicas para flags não documentadas. O corpus SingleStepTests/Z80 é derivado do núcleo Ares segundo seu README fixado, portanto não constitui um oráculo independente de Ares.

**Decisão atual:** não mudar a implementação por inferência documental isolada. Resolver com evidência independente, preferencialmente ROM de teste independente com procedência verificada ou medição de hardware. Manter a hipótese de variante explicitamente documentada.

## 5. DD/FD — regras de indexação

O inventário deve separar:

1. Instruções que usam IX/IY como par de 16 bits.
2. Instruções que substituem H/L por IXH/IXL ou IYH/IYL.
3. Instruções que acessam `(IX+d)`/`(IY+d)`, com deslocamento assinado e wraparound de 16 bits.
4. Instruções que ignoram o prefixo e executam a semântica base, contabilizando o custo nominal adicional de cada prefixo.
5. Prefixos DD/FD repetidos e a precedência do prefixo indexador relevante.
6. Interação com ED e CB, incluindo o caso especial DD/FD CB d op.
7. Casos em que H/L continuam sendo H/L em vez de se tornarem metade do índice, conforme a forma específica da instrução.

Vetores mínimos devem cobrir IX/IY em 0000/FFFF, deslocamentos -128, -1, 0, +1 e +127, endereços resultantes com wraparound, flags, R por fetch de prefixo e instruções indexadas com acesso à memória.

## 6. DDCB/FDCB — operações finais

O decoder implementa grupos de rotate/shift, BIT, RES e SET sobre o endereço indexado. A validação precisa cobrir:

- as oito operações de rotate/shift e suas flags;
- BIT para cada bit 0–7, incluindo a origem de X/Y e o bit 7;
- RES/SET para cada bit;
- todos os campos de registrador final, incluindo código 6 (sem cópia para registrador);
- escrita de memória para rotate/shift, RES e SET, e ausência de escrita para BIT;
- IX e IY, deslocamentos positivos/negativos e wraparound;
- WZ/MEMPTR, R e T-states nominais;
- prefixos repetidos e interação com o decoder de prefixos.

Os vetores existentes de BIT indexado não validam automaticamente os grupos rotate/shift, RES e SET.

## 7. Registradores internos, flags e interrupções transversais

Esses itens cruzam múltiplos espaços e não podem ser considerados concluídos somente por contar opcodes:

- **F:** flags documentadas, bits X/Y e preservação seletiva;
- **Q/P:** sequência de escrita de flags, SCF/CCF e LD A,I/R perto de interrupção;
- **WZ/MEMPTR:** valores em caminhos tomados/não tomados, endereços absolutos, saltos, pilha, blocos e BIT indexado;
- **R:** incremento por fetch M1/prefixo, bit 7 e rollover FF→80, HALT e acknowledge;
- **IFF1/IFF2 e EI delay:** fronteiras antes/depois da instrução protegida;
- **NMI/INT IM0/IM1/IM2:** PC/SP, dados injetados, vetores, acknowledge e estados de borda;
- **RAM/I/O:** efeitos colaterais, endereço de porta, ordenação e wraparound.

## 8. Matriz de evidência e estados

| Estado | Critério para atribuição |
|---|---|
| `IMPLEMENTED` | Há caminho de implementação no runtime para a semântica declarada; não implica validação completa. |
| `QUICK_PASS` | Verificação direcionada rápida executada e resultado registrado no SHA exato. |
| `SHORT_PASS` | Suíte curta pertinente executada com sucesso e resultado registrado no SHA exato. |
| `LONG_PASS` | Suíte longa/estresse executada com sucesso e resultado registrado no SHA exato. |
| `INTEGRATION_PENDING` | Falta confirmação de integração ou de testes no SHA final. |
| `TEMPORAL_PASS` | Exige validação de eventos temporais ordenados, waits e acessos relevantes; T-states agregados não bastam. |

Para esta revisão documental, não foram executados testes localmente. A evidência anterior do projeto registra 162 vetores fixos e resultados de CI em SHAs específicos, mas não foram confirmados nesta auditoria os workflows pós-merge do SHA `6649ffb6dd6ac8fa78c6886df6157de7d0045411`. Portanto, nenhum novo `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS` ou `TEMPORAL_PASS` é declarado aqui.

## 9. Critérios de aceitação para declarar a auditoria semântica concluída

- [ ] Inventário automatizado associa cada codificação aos handlers, aliases, fallback e regras de prefixo reais.
- [ ] Cada família possui vetores semânticos discriminantes e não apenas um smoke test por byte.
- [ ] CB, ED, DD/FD e DDCB/FDCB cobrem resultados, flags, memória, registradores internos e limites.
- [ ] Estados de interrupção, HALT, EI delay, R, IFF, WZ e Q são cobertos transversalmente.
- [ ] Corpus, licenças, procedência e ancestralidade dos oráculos são documentados.
- [ ] Divergências relevantes são resolvidas ou mantidas explicitamente abertas, sem suprimir falhas.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked` passam no SHA candidato.
- [ ] Workflows aplicáveis passam no SHA candidato exato e pós-merge.
- [ ] Testes de traço de barramento cobrem ordem dos eventos antes de declarar precisão ciclo a ciclo.
- [ ] O gate de conclusão Z80 do SG-1000 permanece bloqueado até esses critérios serem satisfeitos.

## 10. Fontes

1. ZiLOG, [Z80 CPU User Manual UM008011-0816](https://www.zilog.com/docs/z80/um0080.pdf).
2. [The Undocumented Z80 Documented](https://z80.info/zip/z80-documented.pdf), referência secundária para comportamento não documentado.
3. [SingleStepTests/Z80 no commit fixado](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7/v1), licença MIT; ver README para procedência e ancestralidade.
4. Código auditado: [`native/sms_core/src/cpu.rs`](https://github.com/leonardo201800478/sega-emulator/blob/main/native/sms_core/src/cpu.rs).
5. Matriz preexistente: [`docs/architecture/z80-sms-reference-coverage.md`](https://github.com/leonardo201800478/sega-emulator/blob/main/docs/architecture/z80-sms-reference-coverage.md).
6. Auditoria ED preexistente: [`docs/z80-ed-opcode-audit.md`](https://github.com/leonardo201800478/sega-emulator/blob/main/docs/z80-ed-opcode-audit.md).

## Resultado desta etapa

**DOCUMENTAÇÃO PUBLICÁVEL; conclusão semântica do Z80 ainda não comprovada.** Este documento consolida os espaços de opcode, a evidência existente, as lacunas e os critérios de fechamento. Ele não altera o runtime, não transforma classificação em correção e não declara precisão ciclo a ciclo.
