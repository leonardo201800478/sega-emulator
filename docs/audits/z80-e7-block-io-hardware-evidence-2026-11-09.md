# E7.7 — Evidência independente de hardware para flags de I/O em bloco

**Data:** 2026-11-09  
**Baseline do projeto:** `main` = `ca1b26089bd649a9248fab4ecbe47b78bc2eb49d`  
**Escopo:** INI/IND/INIR/INDR e OUTI/OUTD/OTIR/OTDR, com foco no efeito de flags quando uma instrução repetitiva é interrompida antes de B chegar a zero.

## 1. Frente documental — fontes primárias e secundárias

### ZiLOG UM0080

O manual ZiLOG é a fonte primária para comportamento documentado. Para INI/IND/OUTI/OUTD, a tabela/documentação simplificada não especifica todas as flags que o silício efetivamente produz; há divergência entre a leitura literal do manual e as fórmulas históricas de flags não documentadas. O manual não deve ser usado como prova negativa de comportamento que ele próprio deixa indefinido.

Fonte: [ZiLOG Z80 CPU User Manual UM0080](https://www.zilog.com/docs/z80/um0080.pdf).

### David Banks — Undocumented Flags

A pesquisa de David Banks descreve a atualização adicional de YF/XF no M-cycle de repetição, usando PC.13/PC.11 após o recuo do PC. Para INxR/OTxR há também efeitos em HF/PF. A referência foi posteriormente confrontada com teste físico.

Fonte: [Z80Decoder — Undocumented Flags](https://github.com/hoglet67/Z80Decoder/wiki/Undocumented-Flags).

### Peter Helcmanovsky — Z80 Block Flags Test v5.0

O README e o código-fonte do teste documentam:
- teste executado em ZX Spectrum, usando IM2 para observar F durante a repetição;
- observação no ponto em que o CPU já executou ao menos uma iteração, B continua diferente de zero e o ciclo interno de repetição recua PC para o byte ED;
- LDIR/LDDR/CPIR/CPDR atualizam YF/XF com PC.13/PC.11;
- INIR/INDR/OTIR/OTDR também alteram PF/HF além das flags geradas pela suboperação INx/OUTx;
- v5.0 inclui casos adicionais de INIR/INDR e uma varredura de valores B para examinar HF/PF em condições com carry.

Fontes:
- [README do Z80 Block Flags Test](https://github.com/MrKWatkins/ZXSpectrumNextTests/blob/develop/Tests/ZX48_ZX128/Z80BlockInstructionFlags/README.md)
- [Código assembly v5.0](https://github.com/MrKWatkins/ZXSpectrumNextTests/blob/develop/Tests/ZX48_ZX128/Z80BlockInstructionFlags/z80_block_flags_test.asm)
- [Resultados e lista de hardware testado](https://github.com/redcode/Z80/wiki/Z80-Block-Flags-Test)

O README declara que os resultados confirmam a pesquisa de Banks em hardware NMOS. O wiki registra execuções em chips como GoldStar Z8400APS, Mostek MK3880N-4, Zilog Z8400APS, SGS Z8400AB1 e NEC D70008AC-6, além de outros clones/variantes. O autor alerta explicitamente que o teste é limitado e não exaustivo, e que clones ou tecnologias CMOS podem divergir.

## 2. Frente de engenharia reversa — implementação atual

A implementação atual possui dois caminhos semânticos relevantes:

- Executor atômico: `native/z80_cpu/src/cpu.rs::execute_block_io`.
- Microsequenciador retomável: `native/z80_cpu/src/sequencer.rs::finish_block_io_iteration`.

Ambos calculam flags da operação de bloco e, quando há repetição, aplicam os efeitos adicionais ao recuar PC. A implementação do microsequenciador também modela o M-cycle interno de cinco T-states e o estado retomável correspondente.

Testes já presentes na baseline:
- `block_io_normal_flags_ports_and_wz_match_documented_edge_vectors`;
- `block_io_repeat_flags_cover_half_carry_and_parity_edges`;
- `block_io_repeat_uses_pc_plus_one_memptr_and_five_extra_tstates`;
- `every_block_io_bus_phase_preserves_waits_and_snapshot_restore`;
- `all_eight_block_io_opcodes_match_atomic_executor_across_counter_wraparound`.

Limite importante: a comparação entre executor atômico e microsequenciador não é um oráculo independente quando ambos codificam a mesma fórmula. Os vetores existentes são dirigidos e não executam o ROM de teste v5.0 dentro do projeto.

## 3. Frente de arquitetura — classificação de evidência

| Questão | Classificação | Justificativa |
|---|---|---|
| Efeitos XY no ciclo de repetição para bloco LD/CP | HARDWARE_EVIDENCE_PARTIAL | Pesquisa de Banks e ROM de teste com confirmação em hardware NMOS; cobertura do ROM é limitada |
| PF/HF adicionais em INxR/OTxR no ciclo de repetição | HARDWARE_EVIDENCE_PARTIAL | O ROM v5.0 contém casos explícitos e B-loop, com resultados em hardware; não foi executado no runtime deste projeto |
| Flags da iteração simples INI/IND/OUTI/OUTD para todo o espaço de estados | DIFFERENTIAL_PARTIAL | Há vetores dirigidos e fórmula implementada; não há varredura independente completa |
| Variantes CMOS/clones | NOT_VERIFIED | O próprio autor admite diferenças possíveis; chips não foram individualmente caracterizados |
| Master System físico | NOT_RUN | Nenhum teste foi executado em hardware SMS nesta auditoria |
| Precisão elétrica/ciclo a ciclo de todos os acessos | OUT_OF_SCOPE_UNLESS_OBSERVABLE | Aplicar ADR-0016; não converter lacuna sem consumidor em requisito de simulação elétrica universal |

## 4. Critérios de avanço

1. Obter os resultados legíveis do v5.0 para cada chip/variante listado e arquivar proveniência, revisão e configuração do sistema.
2. Extrair do assembly vetores concretos de B, valor de transferência, low term, PC e F esperado, sem copiar a fórmula do runtime.
3. Adicionar um fixture independente no Rust para os casos que podem ser representados no modelo de barramento do core; manter separado do teste de equivalência atomic↔sequencer.
4. Executar uma versão adaptada do teste no core com IM2, interrompendo precisamente o ciclo de repetição e verificando F, PC, WZ e retomada.
5. Manter CMOS/clones como variantes separadas; não generalizar resultados NMOS para todos os chips.
6. Não declarar E7.7 fechado até que as outras famílias/efeitos da auditoria semântica global também estejam reconciliados.

## 5. Conclusão

A evidência independente disponível é melhor do que a registrada na auditoria anterior: existe uma ROM pública de teste, com código, licença MIT e resultados documentados em múltiplos Z80 NMOS físicos, que examina especificamente as flags no ciclo de repetição interrompido. Isso sustenta a classificação parcial das flags de repetição de I/O em bloco para o perfil NMOS.

Ainda não é PASS exaustivo, não substitui a execução do teste no núcleo deste projeto, não valida diretamente o SMS físico e não resolve comportamento de CMOS/clones. **E7.7 permanece IN_PROGRESS; E8 permanece BLOCKED.**
