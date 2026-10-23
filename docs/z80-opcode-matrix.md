# Matriz de cobertura do Z80

**Revisão estática original:** `main` em `4bf3aae` (22/10/2026)  
**Código-base:** `src/sega_emulator/cpu/z80.py`  
**Testes consultados:** `tests/unit/test_z80.py`, `tests/unit/test_z80_alu_immediate.py` e `docs/z80-coverage.md`

## Como interpretar esta matriz

Esta é uma **auditoria estática do despachante e dos testes existentes**, não um teste automático de todos os 256 valores de cada espaço de opcode. “Implementado” significa que existe um caminho de execução; não prova correção de todas as entradas, flags, T-states, ciclos de barramento ou comportamento de todas as revisões do silício.

| Estado | Significado |
|---|---|
| **I — Implementado** | O código possui um caminho de execução para a instrução/família. |
| **P — Parcial / precisa de auditoria** | Existe implementação, mas a cobertura funcional ou diferencial é incompleta, ou há ressalvas de prefixo/flags. |
| **A — Ausente como instrução própria** | O despachante não implementa essa instrução/codificação; pode lançar `UnsupportedOpcodeError`. |
| **F — Fallback de prefixo** | O prefixo é tratado como ignorado para instruções que não usam o registrador indexado; isso não equivale a suportar a variante indexada. |

## 1. Espaço base — opcode sem prefixo

A tabela resume as famílias reconhecidas no método `step()`. O estado **P** é intencional: a implementação existe, mas a validação não é exaustiva para toda a família.

| Opcode(s) / padrão | Família | Estado | Lacunas principais |
|---|---|---:|---|
| `00` | NOP | P | T-states nominais cobertos; faltam testes diferenciais. |
| `01,11,21,31` | LD rr,nn | P | Endianess e casos selecionados; ampliar limites. |
| `02,12,0A,1A` | LD (BC/DE),A e LD A,(BC/DE) | P | Acesso de barramento e casos-limite. |
| `03,13,23,33,0B,1B,2B,3B` | INC/DEC rr | P | Pares, wrap e preservação de flags. |
| `04,0C,14,1C,24,2C,34,3C` | INC r | P | Flags e acesso a (HL). |
| `05,0D,15,1D,25,2D,35,3D` | DEC r | P | Flags e acesso a (HL). |
| `06,0E,16,1E,26,2E,36,3E` | LD r,n | P | Operandos e acessos de memória. |
| `07,0F,17,1F` | RLCA/RRCA/RLA/RRA | P | Flags X/Y e validação diferencial. |
| `08` | EX AF,AF' | P | Testes selecionados. |
| `09,19,29,39` | ADD HL,rr | P | Flags X/Y e limites. |
| `10` | DJNZ e | P | Ramos e wrap cobertos em casos selecionados. |
| `18,20,28,30,38` | JR e / JR cc,e | P | Validar todos os deslocamentos e condições. |
| `22,2A,32,3A` | Transferências absolutas entre A/HL e memória | P | Endereços-limite e ordem de barramento. |
| `27` | DAA | P | Espaço de estados de entrada ainda precisa de validação diferencial. |
| `2F,37,3F` | CPL/SCF/CCF | P | Flags documentadas e não documentadas. |
| `40–7F`, exceto `76` | LD r,r' | P | Combinações de registradores e (HL); `76` é HALT. |
| `76` | HALT | P | Comportamento do fetch e interação com interrupções. |
| `80–BF` | ALU A,r: ADD/ADC/SUB/SBC/AND/XOR/OR/CP | P | ALU imediata tem testes sintéticos exaustivos; ALU por registrador ainda requer ampliação/diferencial. |
| `C0,C8,D0,D8,E0,E8,F0,F8` | RET cc | P | Ramos tomados/não tomados e temporização. |
| `C1,D1,E1,F1` | POP qq | P | Ordem de leitura e efeitos de POP AF. |
| `C2,CA,D2,DA,E2,EA,F2,FA` | JP cc,nn | P | Todas as condições têm caminho; ampliar limites. |
| `C3` | JP nn | P | Testes de fluxo e wrap. |
| `C4,CC,D4,DC,E4,EC,F4,FC` | CALL cc,nn | P | Ramos, pilha e temporização. |
| `C5,D5,E5,F5` | PUSH qq | P | Ordem de escrita e efeitos de barramento. |
| `C6,CE,D6,DE,E6,EE,F6,FE` | ALU A,n | P* | *A ALU imediata tem varredura sintética exaustiva em testes dedicados; falta comparação independente.* |
| `C7,CF,D7,DF,E7,EF,F7,FF` | RST p | P | Vetores cobertos; faltam testes diferenciais. |
| `C9` | RET | P | Fluxo de controle e temporização. |
| `CD` | CALL nn | P | Pilha, retorno e temporização. |
| `D3,DB` | OUT (n),A / IN A,(n) | P | Requer IOBus; validar máscara/endereço de porta e ciclos. |
| `D9` | EXX | P | Troca dos bancos alternativos. |
| `E3` | EX (SP),HL | P | Ordem dos acessos e wrap. |
| `E9` | JP (HL) | P | Caminho implementado. |
| `EB` | EX DE,HL | P | Caminho implementado. |
| `F3,FB` | DI/EI | P | Atraso de reconhecimento e interação com interrupções. |
| `F9` | LD SP,HL | P | Caminho implementado. |
| Outros bytes base | Codificações não cobertas pelas famílias acima | A | Não há instrução base correspondente no despachante atual; execução pode lançar `UnsupportedOpcodeError`. |

**Nota de inventário:** os grupos acima são uma visão por famílias, não uma lista independente de 256 células verificadas automaticamente. Alguns opcodes de uma família têm variantes de registrador ou memória, portanto a cobertura por família não significa cobertura de todos os operandos.

## 2. Prefixo CB — rotações, shifts e operações de bits

O método `_execute_cb()` decodifica os 256 valores de segundo byte por campos de opcode. Assim, existe caminho de execução para todo o espaço CB.

| Segundo byte | Operação | Estado | Ressalvas |
|---|---|---:|---|
| `00–3F` | RLC/RRC/RL/RR/SLA/SRA/SLL/SRL sobre B,C,D,E,H,L,(HL),A | P | SLL é não documentada no Z80 clássico; flags e T-states precisam de validação diferencial. |
| `40–7F` | BIT 0–7,r/(HL) | P | Flags de BIT, em especial X/Y para memória, precisam de testes abrangentes. |
| `80–BF` | RES 0–7,r/(HL) | P | Verificar todas as combinações e que flags não são alteradas. |
| `C0–FF` | SET 0–7,r/(HL) | P | Verificar todas as combinações e que flags não são alteradas. |

**Cobertura estática CB: 256/256 codificações têm caminho de execução.** Isso não representa 256 testes independentes nem validação completa do comportamento.

## 3. Prefixo ED — segundo byte

O método `_execute_ed()` reconhece as seguintes codificações/padrões. Os bytes não listados abaixo caem em `UnsupportedOpcodeError`; aliases não documentados que o Z80 real possa tratar como NOP não devem ser considerados implementados sem decisão explícita de compatibilidade.

| Segundo byte / padrão | Família | Estado | Lacunas principais |
|---|---|---:|---|
| `40,48,50,58,60,68,70,78` | IN r,(C), incluindo descarte para (código de registrador 6) | P | Portas, flags e variante sem destino. |
| `41,49,51,59,61,69,71,79` | OUT (C),r, incluindo zero para código 6 | P | Validar comportamento de OUT (C),0 em revisões/alvos pertinentes. |
| `42,52,62,72` | SBC HL,rr | P | Testes sintéticos; requer comparação diferencial. |
| `4A,5A,6A,7A` | ADC HL,rr | P | Testes sintéticos; requer comparação diferencial. |
| `43,53,63,73` | LD (nn),rr | P | Ordem de acesso, wrap e tempos. |
| `4B,5B,6B,7B` | LD rr,(nn) | P | Ordem de acesso, wrap e tempos. |
| `44,4C,54,5C,64,6C,74,7C` | NEG e aliases implementados | P | Confirmar aliases e flags contra referência. |
| `45,4D,55,5D,65,6D,75,7D` | RETN/RETI e aliases implementados | P | Diferenças entre RETN/RETI e comportamento de daisy chain não modelado. |
| `46,4E,66,6E` | IM 0 | P | Codificações/aliases e interrupções. |
| `56,76` | IM 1 | P | Interrupções. |
| `5E,7E` | IM 2 | P | Interrupções. |
| `47,4F,57,5F` | LD I,A / LD R,A / LD A,I / LD A,R | P | Flags, R e interrupções. |
| `67,6F` | RRD/RLD | P | Flags, ordem de acesso e ciclos. |
| `A0,A8,B0,B8` | LDI/LDD/LDIR/LDDR | P | Flags não usuais, condição de repetição e limites. |
| `A1,A9,B1,B9` | CPI/CPD/CPIR/CPDR | P | Flags, término antecipado e limites. |
| `A2,AA,B2,BA,A3,AB,B3,BB` | INI/IND/INIR/INDR/OUTI/OUTD/OTIR/OTDR | P | Flags documentadas/não documentadas; os testes de carry adicionados são casos dirigidos. |
| Outros bytes ED | Outros opcodes/aliases | A | Sem caminho de execução no despachante atual. |

**Política atual e lacuna de compatibilidade:** o núcleo lança `UnsupportedOpcodeError` para bytes ED não reconhecidos. Isso é uma política explícita de diagnóstico, não uma afirmação de que o Z80 físico necessariamente gera uma exceção. A documentação técnica de referência descreve muitos ED não listados como operações sem efeito equivalentes a dois NOPs, enquanto variantes e aliases não documentados podem depender da implementação/revisão do chip. Antes de trocar o comportamento, devemos decidir se o núcleo prioriza diagnóstico estrito ou compatibilidade com o Z80 clássico e adicionar testes específicos para a política escolhida. Referências: [Zilog Z80 CPU User Manual](https://www.zilog.com/docs/z80/um0080.pdf) e [resumo de opcodes não documentados](https://www.z80.info/decoding.htm).

## 4. Prefixos DD e FD — IX/IY

O método `_step_indexed()` suporta um subconjunto indexado. Para outras instruções, tenta tratar DD/FD como prefixo ignorado e executar a instrução base, adicionando 4 T-states. Esse fallback não deve ser confundido com cobertura de todas as instruções indexadas.

| Segundo byte / padrão | Família indexada | Estado | Lacunas principais |
|---|---|---:|---|
| `21` | LD IX/IY,nn | P | Testar ambas as variantes e limites. |
| `22,2A` | LD (nn),IX/IY e LD IX/IY,(nn) | P | Ordem dos acessos e wrap. |
| `23,2B` | INC/DEC IX/IY | P | Wrap e ciclos. |
| `09,19,29,39` | ADD IX/IY,rr | P | Semântica dos operandos, flags e timing. |
| `24,25,26,2C,2D,2E` | INC/DEC/LD IXH/IXL ou IYH/IYL | P | Instruções não documentadas no Z80 clássico; compatibilidade precisa ser decidida. |
| `34,35,36` | INC/DEC/LD (IX/IY+d) | P | Endereçamento assinado, wrap e timing. |
| `40–7F`, exceto `76` | LD com IXH/IXL ou IYH/IYL; variantes com memória indexada | P | Regras de substituição de H/L e combinações com (IX/IY+d). |
| `80–BF` | ALU A com IXH/IXL/IYH/IYL ou (IX/IY+d) | P | Todas as operações e variantes precisam de testes. |
| `E1,E3,E5,E9,F9` | POP, EX (SP), PUSH, JP (IX/IY), LD SP,IX/IY | P | Pilha, wrap e ciclos. |
| `CB d op` | BIT/RES/SET/rotações sobre (IX/IY+d), com cópia opcional para registrador | P | 256 variantes por índice; flags X/Y, endereçamento e ciclos. |
| Outros segundo bytes DD/FD | Prefixo ignorado + instrução base, ou prefixos repetidos | F/P | O comportamento do fallback e dos prefixos repetidos requer testes dedicados; nem toda forma indexada está implementada. |

## 5. Interrupções e estado da CPU

Estas capacidades não são cobertas apenas pela matriz de opcode:

| Capacidade | Estado | Lacunas |
|---|---:|---|
| Solicitação NMI | P | Sequência de acknowledge, prioridade e temporização. |
| Solicitação INT | P | IM 0/1/2, acknowledge, vetor e integração com periféricos. |
| DI/EI e atraso de EI | P | Sequências de interrupção e casos de fronteira. |
| HALT | P | Fetch durante HALT e saída por interrupção. |
| Registrador R | P | Incrementos em M1, prefixos e acknowledge. |
| T-states nominais | P | A maioria dos caminhos retorna uma duração; isso não modela, por si só, cada ciclo de barramento nem wait states/contention. |

## 6. Resultado e próximos passos

Esta revisão confirma **cobertura estática completa do espaço CB em termos de caminho de despacho**, subconjuntos explicitamente reconhecidos em ED e DD/FD, e um conjunto base incompleto. Não atribuí uma percentagem global porque isso exigiria uma regra explícita para contar aliases, prefixos ignorados, codificações não documentadas e combinações de operandos.

Ordem recomendada para transformar a matriz em evidência automatizada:

1. Criar um inventário executável que enumere os 256 opcodes base, os 256 segundos bytes CB/ED e as combinações DD/FD, classificando `executado`, `UnsupportedOpcodeError` e `erro inesperado`.
2. Separar opcodes documentados, aliases documentados, não documentados e indefinidos a partir do manual do Zilog e de uma referência independente.
3. Associar cada célula a pelo menos um teste de execução, depois adicionar testes de flags, T-states, PC, registradores e memória.
4. Fazer comparação diferencial com um emulador de referência confiável; os testes sintéticos atuais não provam equivalência com hardware.
5. Só então calcular percentuais e usar a matriz como critério de aceitação do marco Z80.

## 7. Inventário automatizado de despacho

Foi adicionado `tools/z80_opcode_inventory.py`, que executa um único passo isolado para cada byte nos espaços base, CB, ED, DD, FD, DD-CB e FD-CB, usando memória e I/O determinísticos.

Execute a partir da raiz do repositório:

```powershell
python tools/z80_opcode_inventory.py
python tools/z80_opcode_inventory.py --json
```

O modo normal resume cada espaço e lista as codificações que lançam `UnsupportedOpcodeError` ou geram exceções inesperadas. O modo JSON inclui um registo por codificação, incluindo estado e T-states retornados. O processo termina com código diferente de zero se encontrar uma exceção inesperada; codificações explicitamente não suportadas são relatadas, mas não tratadas como erro inesperado.

**Limitações importantes:** cada combinação é executada apenas uma vez com operandos e estado inicial controlados. A classificação `executed` significa somente que a execução retornou sem exceção. Ela não valida o resultado semântico, as flags, os ciclos de barramento, o timing exato nem a equivalência com hardware. Prefixos repetidos e instruções que consomem operandos são smoke-tested, não exaustivamente validados. O inventário é uma ferramenta de triagem para encontrar falhas de despacho, não um teste de conformidade.

**Limitação da auditoria:** a matriz funcional acima continua sendo uma avaliação estática. O novo inventário automatiza a triagem em runtime, mas os resultados reais ainda precisam ser executados no ambiente do projeto e revisados antes de atualizar percentuais ou marcar instruções como corretas.
