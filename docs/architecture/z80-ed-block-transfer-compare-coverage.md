# Cobertura de LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR

**Escopo:** as oito instruções de transferência e comparação em bloco do espaço ED no núcleo Z80 usado pelo SMS.  
**Runtime auditado:** `native/sms_core/src/cpu.rs`, `execute_ed()`, `execute_block_transfer()` e `execute_block_compare()`.  
**Estado:** implementação existente; cobertura de regressão ampliada; validação no SHA candidato pendente. Este documento não declara as oito instruções concluídas em termos de fidelidade total.

## 1. Fonte normativa e limites

O manual ZiLOG `UM008011-0816` (Z80 CPU User Manual, edição 08/2016) descreve estas instruções no grupo Exchange, Block Transfer, and Search:

| Instrução | Página impressa do manual | Contrato documentado |
|---|---:|---|
| LDI | 130–131 | Copia (HL) para (DE), incrementa HL/DE, decrementa BC |
| LDIR | 132–133 | Repete a transferência enquanto BC após decremento não for zero |
| LDD | 134–135 | Copia (HL) para (DE), decrementa HL/DE/BC |
| LDDR | 136–137 | Repete a transferência decrementando endereços enquanto BC após decremento não for zero |
| CPI | 138 | Compara A com (HL), incrementa HL e decrementa BC |
| CPIR | 139–140 | Repete até encontrar igualdade ou BC chegar a zero |
| CPD | 141 | Compara A com (HL), decrementa HL e BC |
| CPDR | 142–143 | Repete em direção decrescente até igualdade ou BC chegar a zero |

O manual afirma que, se BC começa em zero nas formas repetitivas, o contador decrementa para FFFF e a operação pode percorrer 64 KiB antes de terminar. Também documenta S/Z/H/PV/N/C para essas famílias, mas não documenta integralmente os bits não documentados X/Y nem o comportamento interno de WZ/MEMPTR.

O corpus fixado [SingleStepTests/Z80](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7) tem proveniência externa ao repositório, mas seu README diz que os vetores foram traduzidos do núcleo Z80 do Ares e corrigidos. Portanto, não é um oráculo de implementação independente do Ares. Os vetores dessa origem devem ser tratados como regressões diferenciais úteis, não como evidência de hardware independente.

## 2. Mapa das oito instruções

| Opcode | Instrução | Efeito principal | T-states nominais do núcleo |
|---|---|---|---:|
| ED A0 | LDI | Copia, HL++, DE++, BC-- | 16 |
| ED A8 | LDD | Copia, HL--, DE--, BC-- | 16 |
| ED B0 | LDIR | Igual a LDI; repete enquanto BC != 0 após decremento | 21 ao repetir; 16 ao terminar |
| ED B8 | LDDR | Igual a LDD; repete enquanto BC != 0 após decremento | 21 ao repetir; 16 ao terminar |
| ED A1 | CPI | Compara A com (HL), HL++, BC-- | 16 |
| ED A9 | CPD | Compara A com (HL), HL--, BC-- | 16 |
| ED B1 | CPIR | Igual a CPI; repete enquanto BC != 0 e A != (HL) | 21 ao repetir; 16 ao terminar |
| ED B9 | CPDR | Igual a CPD; repete enquanto BC != 0 e A != (HL) | 21 ao repetir; 16 ao terminar |

Os números são os retornos agregados atuais do núcleo. Não representam um traço ordenado de M-cycles, T-states, acessos de barramento ou estados de espera.

## 3. Contratos semânticos a verificar

### Transferência

- A origem é o byte em (HL); o destino é (DE).
- LDI/LDIR incrementam HL e DE; LDD/LDDR decrementam ambos.
- BC decrementa módulo 16 bits. BC inicial igual a zero produz FFFF após a primeira iteração; não significa zero iterações.
- S, Z e C são preservados; H e N são limpos; PV reflete se o BC decrementado é diferente de zero.
- X/Y dependem do resultado A + byte transferido nas formas simples. Na iteração que repete, o núcleo atual usa os bits X/Y do byte alto do PC após recuar PC em dois bytes. Esse comportamento não documentado precisa de validação diferencial adicional.

### Comparação

- A não é alterado; compara-se A com (HL).
- HL avança em CPI/CPIR e recua em CPD/CPDR; BC decrementa módulo 16 bits.
- N é setado; S/Z/H derivam da subtração; C é preservado; PV indica se BC após decremento é diferente de zero.
- X/Y são derivados do resultado ajustado pela meia-subtração no caminho simples, conforme o comportamento implementado no núcleo. Nas iterações que repetem, X/Y são substituídos pelos bits correspondentes do byte alto do PC.
- CPIR/CPDR param ao encontrar igualdade ou ao fazer BC chegar a zero. Com BC inicial zero e sem igualdade, a repetição pode continuar após o contador envolver para FFFF.
- O núcleo atual atualiza WZ/MEMPTR com PC+1 nos caminhos de repetição e incrementa/decrementa o WZ existente nos caminhos simples. Isso deve continuar sendo comparado a vetores específicos e não deve ser presumido correto apenas porque os testes de controle de fluxo passam.

## 4. Evidência no repositório

A suíte de referência `native/sms_core/tests/z80_singlestep_reference_vectors.rs` agora contém dois vetores fixos para cada uma das oito codificações ED A0, A1, A8, A9, B0, B1, B8 e B9. Esta PR acrescenta dez vetores do corpus fixado: ED B8 0000 ([fonte](https://github.com/SingleStepTests/z80/blob/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7/v1/ed%20b8.json)), ED B9 0000 ([fonte](https://github.com/SingleStepTests/z80/blob/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7/v1/ed%20b9.json)) e um estado adicional por instrução ([arquivos de origem](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7/v1)). Há pelo menos dois estados por codificação, mas isso não é cobertura combinatória e o corpus não é independente do Ares.

A suíte de regressão `native/sms_core/tests/z80_ed_block_transfer_compare_boundaries.rs` contém 16 casos dirigidos que cobrem todas as oito codificações, incluindo:

- wraparound de HL/DE e endereços de memória;
- BC inicial zero e decremento para FFFF;
- terminação com BC decrementado para zero;
- repetição e atualização de PC/WZ;
- preservação de flags documentadas e casos de H/PV;
- bits X/Y no resultado de comparação ajustado e no caminho de repetição.

Os 16 casos do arquivo de fronteiras são invariantes calculados manualmente. Os vetores diferenciais externos têm dois estados por codificação, mas a ancestralidade comum ao Ares reduz sua independência como oráculo. Os testes desta PR precisam passar no SHA candidato exato.

## 5. Lacunas para fechar

| Lacuna | Evidência necessária |
|---|---|
| LDDR e CPDR | Acrescentar estados de término por correspondência, BC=0/1, flags, WZ e limites de PC/HL/DE; CPDR ainda requer mais vetores externos de término |
| Flags X/Y | Vetores que discriminem o resultado ajustado, o PC alto e os limites de PC |
| Aritmética de comparação | Casos de igualdade, desigualdade, meia-subtração, resultado 00/7F/80/FF e preservação de C |
| Contador e endereços | BC=0000/0001/0002/FFFF; HL/DE em 0000/FFFF; destino/origem coincidentes |
| WZ/MEMPTR | Vetores para caminhos simples e repetidos, inclusive wraparound e PC próximo de 0000/FFFF |
| Interrupções | Evidência de aceitação de INT/NMI entre iterações, preservação de PC e ordenação dos eventos |
| Temporização | Comparação dos eventos ordenados de barramento; totais de T-states, isoladamente, não são precisão ciclo a ciclo |
| Independência | Oráculo adicional com proveniência distinta ou resultados de hardware para as divergências relevantes |

## 6. Critérios de aceitação

Não marcar este bloco como semanticamente concluído até:

1. Cada uma das oito instruções ter vetores externos fixos para caminho simples e repetitivo, quando aplicável.
2. Os estados finais compararem registradores principais e alternativos, F, WZ/MEMPTR, R, Q/P, memória modificada e T-states.
3. Os casos de limite de BC, HL, DE e PC cobrirem wraparound e término/repetição.
4. A proveniência e a ancestralidade dos oráculos estarem registradas; uma origem comum não ser descrita como independente.
5. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked` passarem no SHA candidato.
6. As verificações de CI aplicáveis estarem verdes no mesmo SHA.
7. A precisão temporal permanecer classificada separadamente: T-states agregados não autorizam `TEMPORAL_PASS`.

## Referências

- ZiLOG, *Z80 CPU User Manual*, `UM008011-0816`, páginas impressas 130–143 (instruções LDI, LDIR, LDD, LDDR, CPI, CPIR, CPD e CPDR); páginas impressas 68–69 para descrições gerais das flags.
- [SingleStepTests/Z80, commit fixado](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7) — licença MIT; o README declara a tradução a partir do núcleo Z80 do Ares.
- `docs/z80-ed-opcode-audit.md` — inventário das codificações ED explícitas e do fallback.
- `docs/architecture/z80-sms-reference-coverage.md` — matriz de evidência e limitações dos vetores.
