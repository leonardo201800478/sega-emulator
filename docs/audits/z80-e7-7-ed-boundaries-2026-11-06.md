# E7.7 — escopo NMOS de ED 71 e vetores de fronteira ED

**Data:** 2026-11-06  
**Baseline remota:** `main` = `333900ae35a87b2430caedd2edf26218f272c8a6`  
**Branch candidata:** `test/z80-e7-7-ed-wrap-neg-boundaries-20261106`  
**Commit inicial dos testes:** `d286c35d8388208bd2bfc14c688e4656dd12f4a0`  
**Estado:** implementação de testes/documentação candidata; CI pendente.

## 1. Objetivo e limite

Esta fatia trata três pontos localizados da auditoria semântica E7.7:

1. Fixar o significado de `OUT (C),0` para o perfil de CPU efetivamente documentado nas placas SMS1 consultadas.
2. Cobrir wraparound de endereço em `LD (nn),rr` e `LD rr,(nn)` quando `nn=FFFFh`.
3. Ampliar os vetores de fronteira de `NEG` e seus aliases ED.

Não declara E7.7 concluída, não libera E8 e não introduz microsequenciamento ou modelagem elétrica adicional.

## 2. Frente 1 — engenharia reversa e referências independentes

### ED 71 — OUT (C),0

O opcode `ED 71` é a codificação não documentada cujo campo de registrador seria `110`. O manual ZiLOG UM008011-0816 descreve `OUT (C),r` e enumera B, C, D, E, H, L e A; não atribui um registrador ao código 110.

As referências técnicas secundárias consultadas convergem no comportamento por tecnologia:

- Z80 NMOS: escreve `0x00`.
- Zilog Z80 CMOS Z84C00: escreve `0xFF`.

Referências:
- [The Undocumented Z80 Documented — Sean Young](https://z80.info/zip/z80-documented.pdf)
- [redcode/Z80 — bugs e diferenças entre modelos](https://github.com/redcode/Z80/wiki/Bugs)
- [Klive IDE — Zilog Z80 undocumented instructions](https://kliveide.net/en/library/cambridge-z88/developers-notes/the-zilog-z80-cpu-instruction-set/the-zilog-z80-undocumented-instructions/)

Essas fontes são evidência comparativa/experimental secundária; não substituem identificação do chip da placa alvo.

## 3. Frente 2 — documentação de fabricante e hardware SMS1

### Evidência do manual oficial

O manual ZiLOG [UM008011-0816](https://www.zilog.com/docs/z80/um0080.pdf), nas páginas impressas 307–308, descreve a operação e o tempo nominal de `OUT (C),r` como 12 T-states. O campo de registrador listado não inclui o código 110, portanto ED 71 permanece fora da instrução documentada.

### Evidência de BOM das placas

As páginas de hardware do SMS Power identificam, nas revisões SMS1 consultadas:

- [PAL Master System 837-6097](https://www.smspower.org/Development/PALMasterSystemM4POWERBASEPAL837-6097): CPU NEC D780C-1 ou Zilog Z0840004PSC.
- [NTSC Master System 837-6067](https://www.smspower.org/Development/NTSCMasterSystemM4POWERBASENTSC837-6067): CPU NEC D780C-1.

As folhas de dados de fabricante classificam as famílias identificadas como NMOS:

- [ZiLOG Z8400/84C00 product specification](https://www.zilog.com/docs/z80/z8400.pdf): distingue Z08400 NMOS de Z84C00 CMOS.
- [NEC Product Selection Guide (1988)](https://www.bitsavers.org/components/nec/_dataBooks/1988_NEC_Product_Selection_Guide.pdf): classifica μPD780C-1 como microprocessador NMOS.

**Decisão de escopo:** a saída `0x00` é a semântica apropriada para o perfil NMOS das placas SMS1 documentadas acima. Não se está afirmando que todas as placas, clones, variantes CMOS ou revisões desconhecidas compartilhem esse comportamento.

## 4. Frente 3 — auditoria arquitetural do runtime

### Implementação atômica

Arquivo: `native/z80_cpu/src/cpu.rs`, função `execute_ed()`.

O padrão de dispatch de `OUT (C),r` identifica o código de registrador 6 e envia `0` para `CpuBus::write_port`. O endereço é BC e o handler retorna 12 T-states, sem alterar flags.

### Microsequenciador

Arquivo: `native/z80_cpu/src/sequencer.rs`, função `execute_ed_register_control()`.

O caminho retomável aplica a mesma regra: código de registrador 6 produz o valor `0` e agenda `PortWrite` para BC. O teste `all_ed_in_and_out_c_encodings_match_atomic_state_and_port_trace` percorre as codificações IN/OUT, compara estado arquitetural e trace de porta entre os dois caminhos e inclui ED 71.

### Limite arquitetural atual

O núcleo compartilhado não expõe uma seleção explícita entre NMOS e CMOS. Isso é aceitável para o perfil NMOS atualmente adotado para SMS1; não autoriza suportar Z84C00 CMOS implicitamente. Se uma plataforma futura exigir CMOS, será necessária uma decisão explícita de modelagem de variante e de serialização, sem duplicar o Z80.

Nenhuma alteração de runtime foi feita nesta fatia.

## 5. Novos vetores adicionados

Arquivo: `native/z80_cpu/src/cpu.rs`.

### 5.1 Wraparound de transferências absolutas ED

Novo teste: `ed_absolute_word_transfers_wrap_at_ffff_for_all_register_pairs`.

Para BC, DE, HL e SP, verifica:

- `LD (FFFFh),rr`: byte baixo escrito em FFFFh e byte alto em 0000h;
- `LD rr,(FFFFh)`: leitura na mesma ordem little-endian;
- wrap de WZ para 0000h;
- PC após os dois operandos;
- duração nominal de 20 T-states.

### 5.2 Fronteiras de NEG

Novo teste: `ed_neg_aliases_cover_documented_operand_boundaries`.

Para os oito aliases `ED 44/4C/54/5C/64/6C/74/7C`, cobre A = `00h`, `01h`, `7Fh`, `80h` e `FFh`, verificando resultado, flags e duração nominal de 8 T-states.

Os valores esperados são vetores explícitos de teste; não são apresentados como medição de hardware físico.

## 6. Critérios e classificação

| Critério | Estado |
|---|---|
| Identificação NMOS das CPUs listadas nas duas revisões SMS1 consultadas | PASS — documentação de fabricante + BOM SMS Power |
| ED 71 = saída zero no perfil NMOS | PASS — fontes secundárias convergentes; implementação atômica e microsequenciador concordam |
| ED 71 em Zilog CMOS Z84C00 | NOT VERIFIED para o runtime; comportamento secundário reportado como FFh |
| Wraparound ED em todos os pares BC/DE/HL/SP | PENDING CI no HEAD candidato |
| NEG e aliases nos cinco valores de fronteira | PENDING CI no HEAD candidato |
| Execução em Master System físico | NOT RUN |
| CMOS/clones e revisões não identificadas | NOT VERIFIED |
| E7.7 global | IN_PROGRESS |
| E8 | BLOCKED |

Os logs locais enviados anteriormente não identificam o SHA do checkout e antecedem esta branch; portanto, não são reutilizados como evidência dos novos testes.

## 7. Próximas lacunas

Esta fatia fecha apenas dois itens específicos da matriz ED. Permanecem outras lacunas registradas em `docs/z80-ed-opcode-audit.md`, incluindo cobertura de campos arquiteturais por família, WZ/MEMPTR, Q/P, oráculo diferencial externo e validação em hardware SMS. A auditoria global E7.7 continua aberta.

Não iniciar ZEXDOC/ZEXALL enquanto E7 não for formalmente encerrada. Não exigir precisão elétrica ou microsequenciamento universal sem consumidor de gameplay demonstrado.

## 8. Fontes

1. ZiLOG, *Z80 CPU User Manual*, UM008011-0816: https://www.zilog.com/docs/z80/um0080.pdf
2. ZiLOG, *Z8400/84C00 Product Specification*: https://www.zilog.com/docs/z80/z8400.pdf
3. NEC, *1988 Product Selection Guide*: https://www.bitsavers.org/components/nec/_dataBooks/1988_NEC_Product_Selection_Guide.pdf
4. SMS Power, PAL SMS1 837-6097: https://www.smspower.org/Development/PALMasterSystemM4POWERBASEPAL837-6097
5. SMS Power, NTSC SMS1 837-6067: https://www.smspower.org/Development/NTSCMasterSystemM4POWERBASENTSC837-6067
6. Sean Young, *The Undocumented Z80 Documented*: https://z80.info/zip/z80-documented.pdf
7. redcode/Z80, model differences: https://github.com/redcode/Z80/wiki/Bugs
8. Klive IDE, undocumented Z80 instructions: https://kliveide.net/en/library/cambridge-z88/developers-notes/the-zilog-z80-cpu-instruction-set/the-zilog-z80-undocumented-instructions/
