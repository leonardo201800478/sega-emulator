# Auditoria completa do espaço de opcodes ED do Z80

**Escopo:** auditoria estática do despachante Rust ativo em `native/sms_core/src/cpu.rs`, método `execute_ed()`, e dos testes presentes no mesmo arquivo.  
**Alvo de referência:** Zilog Z80 clássico (NMOS como hipótese de compatibilidade inicial do projeto); não generalizar para Z180, R800 ou clones.  
**Resultado:** os 256 segundos bytes ED têm caminho de execução, mas isso não significa que 256 instruções semânticas estejam implementadas ou validadas.

## 1. Resumo executivo

| Medida | Resultado da auditoria estática |
|---|---:|
| Segundo bytes possíveis após ED | 256 |
| Codificações tratadas por famílias explícitas no despachante | 78 |
| Codificações que caem no fallback ED-NOP | 178 |
| Famílias documentadas com implementação explícita | 14 grupos abaixo |
| Validação diferencial contra hardware/oráculo independente nesta auditoria | **Não executada** |
| Precisão ciclo a ciclo / traço ordenado de barramento | **Não implementada por esta interface** |

A contagem de 78 considera as codificações distintas explicitamente reconhecidas pelos padrões e casos do `match`; inclui aliases de NEG, RETN/RETI e IM. As outras 178 codificações retornam 8 T-states pelo `_ => 8` final. Isso caracteriza um fallback funcional no código, não prova de que cada byte reservado seja um NOP em toda variante de CPU.

O teste `every_primary_and_prefixed_encoding_is_classified_without_fallback_errors` percorre os 256 bytes ED e exige que `step()` não retorne erro. Como `execute_ed()` aceita bytes não reconhecidos pelo fallback, esse teste prova **aceitação estrutural**, não semântica, flags, memória, I/O, PC/R ou ciclos corretos para cada byte.

## 2. Inventário das codificações explícitas

Os tempos listados são os retornados atualmente pelo núcleo (T-states nominais agregados). Não equivalem a uma sequência de M-cycles/T-states de barramento.

| Segundo byte ED | Instrução / família | Efeito implementado observado no código | T-states nominais | Evidência e lacunas |
|---|---|---|---:|---|
| `47` | LD I,A | Copia A para I | 9 | Há caso dentro do teste ED combinado; faltam vetores independentes específicos |
| `4F` | LD R,A | Copia A para R | 9 | Sem teste dedicado encontrado nesta inspeção |
| `57`, `5F` | LD A,I / LD A,R | Copia I/R para A; mantém C, atualiza S/Z/PV/X/Y e usa IFF2 para PV; arma estado interno `p` para a borda de interrupção | 9 | LD A,I é coberto indiretamente por teste de interrupção; falta matriz de flags completa para ambos |
| `43, 53, 63, 73` | LD (nn),BC/DE/HL/SP | Escreve palavra em little-endian; WZ recebe nn+1 | 20 | Teste sintético cobre BC; DE/HL/SP, wrap e ordem por todos os pares ainda precisam de vetores dirigidos |
| `4B, 5B, 6B, 7B` | LD BC/DE/HL/SP,(nn) | Lê palavra little-endian; WZ recebe nn+1 | 20 | Teste sintético cobre BC; faltam combinações e wrap |
| `4A, 5A, 6A, 7A` | ADC HL,BC/DE/HL/SP | Usa carry de entrada e atualiza HL e flags aritméticas de 16 bits | 15 | Há helpers compartilhados; falta matriz ED completa independente |
| `42, 52, 62, 72` | SBC HL,BC/DE/HL/SP | Usa borrow/carry de entrada e atualiza HL e flags aritméticas de 16 bits | 15 | Há helpers compartilhados; falta matriz ED completa independente |
| `40,48,50,58,60,68,70,78` | IN r,(C) | Lê porta BC; destino código 6 descarta o valor; atualiza S/Z/PV/X/Y e preserva C; WZ=BC+1 | 12 | Sem matriz completa de valores/portas; IN (C) não altera registrador, mas altera flags |
| `41,49,51,59,61,69,71,79` | OUT (C),r / OUT (C),0 | Escreve registrador para porta BC; código 6 escreve zero | 12 | `OUT (C),0` requer decisão/validação por variante; sem teste completo de saída de I/O |
| `44,4C,54,5C,64,6C,74,7C` | NEG e aliases | Calcula 0−A usando helper de subtração e atualiza A/F | 8 | Um caso `ED 44` no teste combinado; aliases e fronteiras de flags não testados individualmente |
| `45,4D,55,5D,65,6D,75,7D` | RETN / RETI / aliases | Todos executam pop de PC, copiam IFF2 para IFF1 e atualizam WZ | 14 | Existe teste sintético dos oito aliases para PC/SP/F/IFFs/R. O modelo não emite evento externo distinto para RETI/daisy chain |
| `46,4E,66,6E` | IM 0 / aliases | Define modo de interrupção 0 | 8 | Testes dos modos de interrupção cobrem os modos; expandir validação por alias |
| `56,76` | IM 1 | Define modo de interrupção 1 | 8 | Testes de INT cobrem IM1; alias específico não tem teste isolado |
| `5E,7E` | IM 2 | Define modo de interrupção 2 | 8 | Testes de INT cobrem IM2; alias específico não tem teste isolado |
| `67` | RRD | Troca nibbles entre A e (HL) segundo a operação RRD | 18 | Sem teste dedicado encontrado nesta inspeção |
| `6F` | RLD | Troca nibbles entre A e (HL) segundo a operação RLD | 18 | Sem teste dedicado encontrado nesta inspeção |
| `A0,A8,B0,B8` | LDI/LDD/LDIR/LDDR | Transfere byte (HL)→(DE), ajusta HL/DE/BC e flags; formas repetitivas recuam PC e retornam 21 T quando continuam | 16 / 21 | Um caso LDI é coberto no teste combinado; flags X/Y, término, repetição, limites e estado intermediário precisam de matriz diferencial |
| `A1,A9,B1,B9` | CPI/CPD/CPIR/CPDR | Compara A com (HL), ajusta HL/BC e flags; formas repetitivas continuam enquanto BC≠0 e A≠valor | 16 / 21 | Sem cobertura completa observada; flags X/Y e WZ merecem prioridade |
| `A2,AA,B2,BA` | INI/IND/INIR/INDR | Lê porta BC, escreve em (HL), ajusta HL/B, calcula flags e repete conforme B | 16 / 21 | Falta validação independente de I/O, flags, porta e ordem de acesso |
| `A3,AB,B3,BB` | OUTI/OUTD/OTIR/OTDR | Lê (HL), escreve em porta baseada no B decrementado e C, ajusta HL/B, calcula flags e repete conforme B | 16 / 21 | Falta validação independente de porta, flags, ordem de acesso e comportamento de repetição |

**Nota sobre aliases:** os padrões de NEG e RETN/RETI reconhecem oito bytes cada e executam um mesmo corpo. Para RETI, o núcleo não distingue a semântica externa de sinalização de fim de interrupção usada por controladores com daisy chain. A equivalência interna de retorno não prova equivalência do comportamento externo.

## 3. Espaço ED completo: classificação dos 256 segundos bytes

Esta tabela classifica todo o espaço por intervalos e exceções. “Fallback ED-NOP” significa somente que o despachante atual retorna 8 T-states sem efeitos explícitos adicionais. A classificação não certifica a semântica em silício de variantes ou clones.

| Intervalo / conjunto | Classificação atual | Detalhe |
|---|---|---|
| `00–3F` | Fallback ED-NOP | Nenhuma codificação neste intervalo tem handler explícito |
| `40–7F` | Mistura de handlers e fallback | Contém IN/OUT, ADC/SBC HL, transferências de palavra, NEG, RETN/RETI, IM, LD I/R e RRD/RLD; os demais bytes retornam pelo fallback |
| `80–9F` | Fallback ED-NOP | Nenhuma codificação neste intervalo tem handler explícito |
| `A0–A3` | Operações de bloco explícitas | LDI, CPI, INI, OUTI |
| `A4–A7` | Fallback ED-NOP | Sem handler explícito |
| `A8–AB` | Operações de bloco explícitas | LDD, CPD, IND, OUTD |
| `AC–AF` | Fallback ED-NOP | Sem handler explícito |
| `B0–B3` | Operações de bloco repetitivas explícitas | LDIR, CPIR, INIR, OTIR |
| `B4–B7` | Fallback ED-NOP | Sem handler explícito |
| `B8–BB` | Operações de bloco repetitivas explícitas | LDDR, CPDR, INDR, OTDR |
| `BC–FF` | Mistura de aliases explícitos e fallback | Os handlers já listados reaparecem nos aliases de NEG/RETN/RETI/IM e registradores especiais; todos os demais bytes caem no fallback |

### Codificações explícitas, agrupadas para auditoria automática

- Registradores especiais: `47, 4F, 57, 5F`.
- Transferência absoluta de pares: `43, 53, 63, 73, 4B, 5B, 6B, 7B`.
- Aritmética de 16 bits: `42, 52, 62, 72, 4A, 5A, 6A, 7A`.
- I/O por BC: `40,48,50,58,60,68,70,78,41,49,51,59,61,69,71,79`.
- NEG: `44,4C,54,5C,64,6C,74,7C`.
- RETN/RETI: `45,4D,55,5D,65,6D,75,7D`.
- IM: `46,4E,66,6E,56,76,5E,7E`.
- Nibble: `67,6F`.
- Bloco: `A0,A8,B0,B8,A1,A9,B1,B9,A2,AA,B2,BA,A3,AB,B3,BB`.

A soma desses conjuntos é 78 codificações únicas. As outras 178 são aceitas pelo fallback. O fallback também faz com que um smoke test de todos os bytes passe sem comprovar a correção semântica das codificações reservadas.

## 4. Auditoria de semântica e riscos encontrados

### 4.1 Operações de bloco: prioridade alta

Os handlers executam toda a operação dentro de uma chamada de instrução e retornam apenas a duração nominal agregada. Isso impede validar a sequência temporal de cada leitura/escrita. Antes de afirmar conformidade, criar vetores independentes para:

- LDI/LDD: HL/DE incrementam/decrementam, BC decrementa, memória e flags preservadas/alteradas conforme especificação.
- LDIR/LDDR: continuação quando BC≠0; PC volta ao prefixo ED; término em BC=0; 16 T-states na última iteração e 21 nas iterações que repetem.
- CPI/CPD: resultado da comparação, H, N, S/Z, P/V, X/Y e WZ.
- CPIR/CPDR: saída antecipada quando A=(HL), além do término por BC=0.
- INI/IND/INIR/INDR e OUTI/OUTD/OTIR/OTDR: sequência exata de acesso à porta/memória, valor do endereço de I/O, decremento de B, flags e condições de repetição.

A lógica atual de flags de I/O de bloco contém fórmulas próprias e não deve ser considerada comprovada por mera inspeção. Usar casos discriminantes com carry/half-carry, valor transferido com bit 7 ligado/desligado, B inicial 1/2/0 e HL em limites.

### 4.2 RETN versus RETI

O mesmo handler cobre os oito bytes e sempre restaura IFF1 a partir de IFF2. Isso cobre a operação interna de retorno testada, mas não representa uma notificação externa específica de RETI. Se o sistema de interrupções do SMS/SG-1000 precisar de daisy chain, será necessária uma saída de evento separada; não simular esse sinal alterando PC/IFFs.

### 4.3 I/O por (C)

IN/OUT por BC tem caminho explícito, mas a interface `CpuBus` não representa fases de barramento nem o wait state de I/O. `IN (C)` (código destino 6) descarta o valor e atualiza flags; `OUT (C),0` escreve zero conforme a implementação. O comportamento de saída zero deve ser documentado para a variante escolhida e testado em relação à fonte independente apropriada.

### 4.4 Flags, WZ e estado de interrupção

- `LD A,I`/`LD A,R`: PV usa IFF2 e o núcleo usa um latch interno para tratar uma borda de interrupção; testar com INT pendente e todos os estados de IFF.
- `ADC/SBC HL,rr`: conferir exaustivamente carry/borrow, H, overflow, S/Z e bits X/Y do byte alto.
- `RRD/RLD`: validar os dois nibbles, memória, A, S/Z/PV, preservação de C, WZ e ciclos.
- `NEG`: testar A=00, 01, 7F, 80, FF e todos os aliases.
- `IM`: verificar todos os aliases e o efeito real em subsequente INT; codificações redundantes não podem produzir modos diferentes.
- `WZ/MEMPTR`: o código atual atualiza WZ em vários handlers ED, mas a matriz de atualizações precisa de comparação por instrução. Um valor WZ plausível não é evidência suficiente.
- `Q`: o efeito da escrita de F em sequências posteriores deve ser testado separadamente; não inferir a correção de X/Y de um único caso.

### 4.5 T-states nominais não são temporização de barramento

Os valores devolvidos pelo handler não codificam uma sequência de M1, leituras/escritas de memória, I/O, refresh ou wait states. A auditoria não encontrou um modelo de eventos ordenados dentro de `execute_ed()`. Portanto, mesmo valores nominais que coincidam com o manual não comprovam a temporização física.

## 5. Matriz de testes e evidência atual

| Área | Evidência encontrada no código | Classificação honesta |
|---|---|---|
| Aceitação dos 256 bytes ED | `every_primary_and_prefixed_encoding_is_classified_without_fallback_errors` percorre ED junto aos demais prefixos | **Cobertura estrutural**, não valida semântica |
| LD I,A; LD A,I; NEG; LD (nn),BC; LD BC,(nn); LDI | `ed_prefix_loads_special_registers_arithmetic_and_block_transfer` | **Teste sintético dirigido parcial** |
| RETN/RETI aliases | `test_retn_reti_aliases_restore_iff1_from_iff2` (registrado na documentação de cobertura) | **Teste sintético de estado**, sem sinal externo RETI |
| IM 0/1/2 | `maskable_interrupt_modes_zero_one_and_two_use_their_vectors` | **Teste sintético de integração de interrupção**, não cobertura total dos aliases ED |
| Flags e semântica de cada opcode ED | Não foi localizada uma matriz independente que valide cada entrada, estado inicial e saída | **INTEGRATION_PENDING / validação diferencial pendente** |
| Temporização ciclo a ciclo | A API retorna T-states nominais por instrução | **TEMPORAL_PASS não atingido** |

Os nomes de teste acima foram identificados por inspeção do código/documentação; esta auditoria não executou `cargo test` nem workflows. Não declarar PASS de execução com base apenas na existência dos testes.

## 6. Critérios para fechar a auditoria ED

1. Manter um vetor fixo por codificação explícita, cobrindo registradores, memória/I/O, PC, SP, F, IFFs, R, WZ e duração nominal aplicável.
2. Para instruções paramétricas, adicionar fronteiras e estados iniciais discriminantes; para blocos repetitivos, testar iteração que continua e a última iteração.
3. Validar aliases separadamente e documentar a variante-alvo.
4. Comparar com ao menos um oráculo independente de procedência/licença verificadas e, quando possível, com uma segunda referência sem ancestralidade comum conhecida.
5. Para bytes fallback, manter teste estrutural separado da decisão de compatibilidade. Confirmar a política de ED-NOP contra documentação e variante; não tratar fallback como instrução documentada sem fonte.
6. Executar e registrar checks no SHA candidato exato: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, workflows pertinentes e seus logs.
7. Criar futuramente traços de barramento para ED, incluindo I/O, blocos e interrupções, antes de declarar precisão temporal.

## 7. Fontes

1. ZiLOG, **Z80 CPU User Manual**, UM008011-0816 (2016), especialmente tabela de opcodes e descrições individuais de instruções ED, flags e temporização. [PDF oficial](https://www.zilog.com/docs/z80/um0080.pdf).
2. ZiLOG, **Z80 Family CPU User Manual** (2001), usado no projeto como comparação histórica; conferir visualmente o PDF quando a extração textual estiver corrompida.
3. [Z80.info — The Undocumented Z80 Documented](https://z80.info/zip/z80-documented.pdf), referência secundária para comportamentos não documentados; não substitui a fonte primária nem a identificação de variante.
4. Código auditado: [`native/sms_core/src/cpu.rs`](https://github.com/leonardo201800478/sega-emulator/blob/main/native/sms_core/src/cpu.rs), função `execute_ed()` e testes no mesmo arquivo.

**Conclusão:** a implementação atual reconhece explicitamente 78 dos 256 segundos bytes ED e encaminha os outros 178 para um retorno de 8 T-states. Há cobertura sintética para alguns caminhos importantes, mas não evidência suficiente para declarar o espaço ED semanticamente validado. A prioridade é transformar a matriz acima em vetores discriminantes e diferencialmente verificados, sem ampliar o escopo para temporização ciclo a ciclo antes de fechar a semântica funcional.
