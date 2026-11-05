# E7.7 — auditoria complementar das flags de I/O em bloco

**Data:** 2026-11-05  
**Baseline de código:** `main` = `1da784f556b9524f5a6f43ce8a7c525e68354f9e`  
**Escopo:** INI, IND, INIR, INDR, OUTI, OUTD, OTIR e OTDR.  
**Política:** ADR-0016 — precisão funcional obrigatória e precisão temporal proporcional à dependência observável.

Este documento complementa os relatórios históricos dos três agentes da PR #527. A PR #527 não está integrada e sua baseline era anterior. As conclusões abaixo foram confrontadas com o código integrado na baseline indicada.

## 1. Conclusão executiva

1. O manual ZiLOG UM008011-0816 não especifica integralmente as flags das operações de I/O em bloco. Em INI, por exemplo, declara C não afetado e H/PV desconhecidos.
2. Pesquisas experimentais em hardware Z80 NMOS caracterizam um comportamento real mais específico para essas flags.
3. MAME e redcode/Z80 implementam fórmulas compatíveis com essa caracterização para as flags normais e para o ciclo adicional de repetição.
4. O código integrado em `native/z80_cpu/src/cpu.rs` e `native/z80_cpu/src/sequencer.rs` corresponde às fórmulas NMOS pesquisadas. Não foi identificada uma correção de runtime justificada por esta investigação.
5. O projeto não deve afirmar que a semântica de flags é universal para CMOS e todos os clones. Essa diferença permanece sem comprovação específica.
6. A subquestão de flags de I/O em bloco fica **caracterizada para o perfil Zilog NMOS**, mas E7.7 global continua aberta por outras famílias e efeitos semânticos ainda não auditados.

## 2. Frente 1 — engenharia reversa e evidência experimental

### 2.1 David Banks — flags do ciclo de repetição interrompido

Fonte: David Banks, *Undocumented Z80 Flags*, revisão 1.0, 2018-08-21.

O documento descreve alterações de flags durante os cinco T-states adicionais usados para repetir instruções de bloco. Para INxR/OTxR, quando o contador B permanece diferente de zero:

- X/Y recebem os bits 11 e 13 do PC da instrução;
- N continua refletindo o bit 7 do dado transferido;
- H e P/V sofrem ajustes dependentes de C, N e B;
- quando C está setado, H depende do nibble baixo de B e do bit 7 do dado;
- P/V combina o resultado auxiliar da operação com B e B±1.

Fonte primária da pesquisa:
https://zxe.io/depot/documents/technical/Undocumented%20Z80%20Flags%20rev1.0%20%282018-08-21%29%28Banks%2C%20David%29%28en%29%5B%21%5D.pdf

### 2.2 Peter Helcmanovsky — teste em hardware real

Fonte:
https://github.com/MrKWatkins/ZXSpectrumNextTests/tree/8adcbc8384b2311fb6d36d424478aef583d5c307/Tests/ZX48_ZX128/Z80BlockInstructionFlags

O teste:
- executa INIR, INDR, OTIR e OTDR;
- gera uma interrupção IM2 durante a repetição;
- captura F no handler;
- verifica X/Y, H, P/V, N e C;
- inclui casos de carry e de bit 7 do dado;
- contém um segundo teste variando B para verificar H.

O README registra resultados em hardware real com CPUs Z80 NMOS e clones compatíveis, incluindo GoldStar Z8400APS e Mostek MK3880N-4. Também registra possíveis diferenças em CMOS/clones e limitações do próprio teste.

**Limitação:** a suíte é deliberadamente limitada e não prova todas as combinações possíveis. Ela não foi executada nesta sessão em um Master System físico. É evidência experimental de comportamento Z80 NMOS, não uma medição direta de cada revisão do SMS.

### 2.3 redcode/Z80 — implementação comparativa

Fonte:
https://github.com/redcode/Z80/blob/19f5d65ab4c77f739fe15fdbb4c4f99aa942f92a/sources/Z80.c

A implementação explicita:
- cálculo de T para INI/IND a partir de C±1;
- cálculo de T para OUTI/OUTD a partir do L já atualizado;
- H e C derivados de T > 255;
- P/V derivado da paridade de `((T & 7) XOR B)`;
- ajustes adicionais de H/PV no ciclo de repetição interrompido;
- X/Y derivados do PC durante esse ciclo.

O próprio código cita a pesquisa de David Banks e o teste de Peter Helcmanovsky. Portanto, é uma implementação comparativa coerente com a evidência experimental, não uma segunda medição independente de silício.

### 2.4 MAME

Fontes:
- https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.lst
- https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.cpp

O MAME usa as mesmas fórmulas-base para T, H/C, P/V e o ajuste de flags no ciclo adicional. O método `block_io_interrupted_flags()` implementa a alteração de X/Y, H e P/V.

**Divergência de MEMPTR:** no corpo de execução consultado, o MAME conserva o WZ/MEMPTR derivado do endereço de I/O ao entrar no caminho repetitivo. Não usar esse trecho como evidência conclusiva para MEMPTR no ciclo repetitivo.

### 2.5 Patrik Rak — MEMPTR em hardware

Fonte:
https://github.com/raxoft/z80test

O histórico da versão 1.2a, publicada em 2023-12-02, registra a correção dos CRCs de INIR/INDR interrompidos no teste MEMPTR, creditando redcode.

Histórico:
https://github.com/raxoft/z80test/blob/master/changes.txt

A implementação redcode documenta MEMPTR = PC + 1 durante o ciclo adicional de repetição das quatro instruções INxR/OTxR. A implementação Sega também usa PC + 1. A evidência de z80test fortalece essa escolha, mas o teste físico foi concebido para hardware ZX Spectrum/Z80 e não substitui a validação direta no SMS.

## 3. Frente 2 — documentação primária e variante do SMS

### 3.1 Manual oficial ZiLOG

Documento: *Z80 CPU User Manual*, UM008011-0816, fornecido no contexto do projeto.

Nas descrições de INI/IND/INIR/INDR/OUTI/OUTD/OTIR/OTDR, o manual não especifica todas as flags reais:

- S/Z são descritas em relação ao decremento de B;
- N é setada;
- H/PV são desconhecidas;
- C aparece como não afetada nas descrições correspondentes.

Isso é uma descrição documental, não prova de que o silício NMOS deixe C invariável. A pesquisa experimental demonstra um comportamento não documentado mais específico.

PDF oficial:
https://www.zilog.com/docs/z80/um0080.pdf

### 3.2 Identificação do processador SMS

SMS Power descreve o Mark III como usando Z80A a aproximadamente 3,58 MHz:
https://www.smspower.org/Development/SMSOfficialDocs%26num%3D5

A documentação de serviço de uma revisão do Master System, reproduzida na fonte do MAME, identifica o componente Z0840004PSC:
https://github.com/mamedev/mame/blob/master/src/mame/sega/sms.cpp

A família Z0840004 é a variante Zilog NMOS; a documentação de produto Zilog distingue a família NMOS Z08400 da CMOS Z84C00:
https://www.zilog.com/docs/z80/z8400.pdf

**Decisão de perfil:** usar o comportamento Zilog NMOS como baseline de compatibilidade para o SMS documentado. Não generalizar essa identificação a todas as revisões de placa sem BOM/esquema específico.

### 3.3 CMOS e clones

O teste de Helcmanovsky alerta que CMOS e clones podem divergir, mas não fornece uma matriz conclusiva de flags de I/O em bloco para Zilog CMOS Z84C00.

A documentação Zilog consultada evidencia diferenças de variante em outras instruções, mas não estabelece uma fórmula alternativa de INI/IND/INIR/INDR/OUTI/OUTD/OTIR/OTDR.

**Classificação:** diferença de flags de I/O em bloco entre NMOS e CMOS = **NOT VERIFIED**. Não criar perfil alternativo sem experimento ou documentação específica.

## 4. Semântica adotada para o perfil NMOS

Definições:

- `M`: byte lido do porto ou escrito no porto;
- `B'`: B após decremento;
- `L'`: L após incremento/decremento de HL;
- `C'`: C original ajustado em +1 para INI/INIR ou −1 para IND/INDR, com wrap de 8 bits;
- `T`: soma auxiliar de 9 bits.

Operações de entrada:

```text
T = M + ((C ± 1) & 0xFF)
```

Operações de saída:

```text
T = M + L'
```

Flags após a operação normal:

| Flag | Regra NMOS pesquisada |
|---|---|
| S | bit 7 de B' |
| Z | B' == 0 |
| X/Y | bits 5/3 de B' |
| H | setada se T > 255 |
| C | setada se T > 255 |
| P/V | paridade par de ((T & 7) XOR B') |
| N | bit 7 de M |

Quando uma operação repetitiva ainda deve repetir (B' != 0), o ciclo adicional de cinco T-states altera o estado observado:

- PC retorna ao endereço do prefixo ED;
- X/Y recebem os bits correspondentes do PC da instrução;
- N, S e Z permanecem associados ao dado/contador;
- H/PV são recalculadas segundo C, N e B' pelas regras de Banks;
- WZ/MEMPTR é PC + 1 segundo redcode e a evidência MEMPTR citada.

O estado de flags após esse ciclo é o que pode ser observado por uma interrupção aceita antes da próxima iteração. A iteração seguinte, se executada, atualiza as flags novamente.

## 5. Frente 3 — auditoria do código integrado

Baseline: `1da784f556b9524f5a6f43ce8a7c525e68354f9e`.

### 5.1 Executor atômico

Arquivo: `native/z80_cpu/src/cpu.rs`  
Função: `execute_block_io()`

A implementação:
- calcula C±1 para entrada;
- usa L após a atualização de HL para saída;
- decrementa B;
- calcula H/C com o limite de 255;
- calcula P/V pela paridade de `((T & 7) XOR B')`;
- usa o bit 7 do dado para N;
- aplica o ajuste de flags da repetição;
- usa PC + 1 para WZ no caminho repetitivo.

### 5.2 Microsequenciador

Arquivo: `native/z80_cpu/src/sequencer.rs`  
Função: `finish_block_io_iteration()`

A implementação reproduz a mesma regra para:
- direção;
- valor auxiliar T;
- flags normais;
- ajustes de H/PV no ciclo de repetição;
- X/Y do PC;
- WZ e cinco T-states adicionais.

Não foi encontrada divergência entre executor atômico e microsequenciador nos cálculos auditados.

### 5.3 Testes já existentes

- `ed_block_io_vectors_check_ports_memory_counter_and_repeat`: efeitos dos oito opcodes, endereços e repetição.
- `block_io_normal_flags_ports_and_wz_match_documented_edge_vectors`: flags normais e WZ.
- `block_io_repeat_flags_cover_half_carry_and_parity_edges`: cenários de H/PV no ciclo repetitivo.
- `block_io_repeat_uses_pc_plus_one_memptr_and_five_extra_tstates`: WZ/PC+1 e duração do ciclo repetitivo.
- `native/sms_core/tests/z80_ed_block_io_reference_vectors.rs`: vetores externos SingleStepTests para as oito instruções.

**Limitação do corpus SingleStepTests:** os vetores foram traduzidos do núcleo Ares e corrigidos posteriormente. São úteis para regressão, mas não constituem oráculo independente de Ares ou medição de silício.

### 5.4 Incremento de teste nesta branch

Foi acrescentado:
`native/sms_core/tests/z80_ed_block_io_flag_reference_model.rs`

O teste executa uma matriz de 48.000 combinações determinísticas, cobrindo:
- os oito opcodes;
- B=00/01/02/0F/10/7F/80/FF;
- C=00/01/7F/FE/FF;
- limites de HL e wrap;
- dados 00/01/7F/80/FE/FF;
- PC com combinações X/Y e wrap;
- flags, B, HL, PC, WZ, T-states e endereço de I/O.

O modelo de referência foi escrito separadamente dos helpers de produção a partir das regras documentadas por Banks e do teste físico de Helcmanovsky.

**Classificação do novo teste:** referência matemática derivada de pesquisa externa; não é um novo oráculo de silício. Só poderá ser classificado PASS após CI no HEAD exato desta branch.

## 6. Decisão técnica

### Comportamento adotado

Manter a semântica atual do executor e do microsequenciador para o perfil Zilog NMOS.

### Alteração de runtime

**Nenhuma alteração de runtime é justificada nesta auditoria.** O código integrado corresponde às fórmulas NMOS encontradas. Corrigir a implementação sem uma divergência demonstrada seria uma mudança especulativa.

### Incertezas restantes

1. A fórmula de flags em CMOS Z84C00 e clones não está comprovada.
2. Não foi executado o teste de Helcmanovsky em hardware Master System nesta sessão.
3. A evidência física citada é de hardware Z80/Spectrum, não uma medição direta de cada placa SMS.
4. MAME diverge de redcode/raxoft na interpretação de WZ/MEMPTR do ciclo repetitivo; manter a escolha PC+1, que é a adotada pelo runtime atual e tem apoio do teste MEMPTR citado, mas preservar a divergência na matriz de evidências.
5. A auditoria de flags de I/O em bloco não fecha outras pendências semânticas E7.7.

## 7. Critérios de aceitação e classificação

| Critério | Classificação nesta auditoria |
|---|---|
| Texto oficial UM008011-0816 | PASS — consultado; documenta limites/flags indefinidas |
| Pesquisa Banks 2018 | PASS — fórmula documentada para repetição interrompida |
| Teste físico Helcmanovsky | PASS — fonte e procedimento consultados; não executado nesta sessão |
| Comparação MAME/redcode | PASS — fórmulas de flags concordantes |
| Confronto com executor atômico | PASS por inspeção de código; novo teste ainda aguarda CI |
| Confronto com microsequenciador | PASS por inspeção de código; novo teste ainda aguarda CI |
| Matriz de 48.000 casos | NOT RUN localmente; aguardando CI no HEAD candidato |
| CMOS e clones | BLOCKED — evidência específica insuficiente |
| Hardware SMS físico | NOT RUN |

Não classificar esta auditoria como LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS.

## 8. Impacto no roadmap

- O subitem E7.7 relativo às flags de I/O em bloco fica **caracterizado para Zilog NMOS**, condicionado ao CI do novo teste.
- E7.7 global permanece **IN_PROGRESS** até concluir as demais famílias e efeitos arquiteturais.
- E8 permanece **BLOCKED** até fechamento formal de E7.
- Nenhuma mudança na ADR-0016 é necessária.
- Nenhuma timeline racional genérica, BUSREQ/BUSACK ou microsequenciamento universal é reaberto por esta investigação.
- SG-1000 permanece preservado; o teste usa o Z80 compartilhado no crate SMS e não cria implementação específica de console.

## 9. Fontes

- ZiLOG UM008011-0816: https://www.zilog.com/docs/z80/um0080.pdf
- David Banks, *Undocumented Z80 Flags* rev. 1.0: https://zxe.io/depot/documents/technical/Undocumented%20Z80%20Flags%20rev1.0%20%282018-08-21%29%28Banks%2C%20David%29%28en%29%5B%21%5D.pdf
- Peter Helcmanovsky, Z80 Block Flags Test: https://github.com/MrKWatkins/ZXSpectrumNextTests/tree/8adcbc8384b2311fb6d36d424478aef583d5c307/Tests/ZX48_ZX128/Z80BlockInstructionFlags
- redcode/Z80: https://github.com/redcode/Z80/blob/19f5d65ab4c77f739fe15fdbb4c4f99aa942f92a/sources/Z80.c
- MAME Z80: https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.lst
- Patrik Rak z80test: https://github.com/raxoft/z80test
- z80test changelog: https://github.com/raxoft/z80test/blob/master/changes.txt
- SMS Power SMS Official Docs: https://www.smspower.org/Development/SMSOfficialDocs%26num%3D5
- MAME SMS hardware/BOM: https://github.com/mamedev/mame/blob/master/src/mame/sega/sms.cpp
- ZiLOG Z8400/Z84C00 product specification: https://www.zilog.com/docs/z80/z8400.pdf
