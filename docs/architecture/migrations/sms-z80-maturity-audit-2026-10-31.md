# Auditoria comparativa de maturidade — Z80 SMS × SG-1000

**Data:** 2026-10-31  
**Branch auditada:** `main`  
**SHA de baseline:** `d54d2751ce19a11c2591b400f1ef43b00f797b6c`  
**Escopo:** proveniência do executor, semântica, ALU, barramento, testes, integração de máquina e evidência de validação.

## 1. Conclusão executiva

**Sim: o Z80 do SMS é o baseline de engenharia e validação mais maduro.** A conclusão precisa, porém, de uma distinção:

- **Executor de produção atual:** o código executável normalizado de `native/sms_core/src/cpu.rs` e `native/sg1000_core/src/cpu.rs` é idêntico (3.257 linhas não vazias/não comentário, na mesma ordem). Não há hoje duas implementações semanticamente diferentes a escolher.
- **Proveniência:** o cabeçalho do SG-1000 declara que seu executor foi portado da implementação SMS mais madura. O histórico da consolidação #160 confirma que o SMS preservou correções WZ/MEMPTR e cobertura indexed-CB e que diferenças propostas pelo SG-1000 foram rejeitadas quando falharam em vetores independentes.
- **Verificação:** o SMS possui um conjunto substancial de testes de integração Z80 em `native/sms_core/tests`, além de um harness externo fixado. O SG-1000 não possui diretório `native/sg1000_core/tests`; tem testes inline no CPU/máquina e testes de integração de máquina mais enxutos.
- **Conclusão operacional:** tratar o SMS como baseline canônico de semântica e corpus de validação; tratar SG-1000 como consumidor/adapter que precisa de regressões próprias de integração. Não reescrever o executor nem copiar o corpus inteiro para o SG-1000.

A auditoria anterior que destacou apenas a igualdade dos arquivos estava incompleta: igualdade de código não significa igualdade de maturidade de testes, proveniência ou validação de integração.

## 2. Evidência direta no SHA auditado

| Artefato | SMS | SG-1000 | Interpretação |
|---|---|---|---|
| Executor `cpu.rs` | 3.488 linhas; blob `a8d358e2c3e28064d204da96e281cd324811f1ee` | 3.487 linhas; blob `b561e4e921b0940d37ed560785394dfdb31a83d9` | 3.257 linhas executáveis normalizadas idênticas, na mesma ordem |
| ALU local | blob `84bb9ce76c4235533b3fb4888619301c06a01216` | blob `6c726c27bad1aaae2bfcec48727417264aab77a5` | Código executável normalizado idêntico |
| Testes inline em `cpu.rs` | 49 `#[test]` | 49 `#[test]` | Mesmo conjunto, pois os arquivos de CPU são executavelmente idênticos; não são 98 provas independentes |
| Testes inline em `machine.rs` | 14 `#[test]` | 13 `#[test]` | Integração específica de cada máquina; números não medem qualidade isoladamente |
| Diretório de testes de integração Z80 | 37 arquivos em `native/sms_core/tests` | Não existe `native/sg1000_core/tests` | Forte assimetria de evidência |
| Barramento | `bus.rs` reexporta `sega-z80-bus` | `bus.rs` contém cópia local do contrato | SMS está mais avançado na consolidação do contrato |

### 2.1 O que o conjunto SMS valida

A árvore atual contém 37 arquivos de teste dedicados ao Z80. Entre os mais relevantes:

- `z80_singlestep_reference_vectors.rs`: 131 vetores fixos no arquivo atual, cobrindo estado arquitetural, registradores alternativos, flags, WZ, Q/P, IFF, IM, memória selecionada e T-states agregados.
- `z80_alu_exhaustive_vectors.rs`: 8 testes combinatórios para operações ALU e flags; o modelo matemático é interno, não captura de hardware.
- `z80_cb_exhaustive_semantics.rs`: 6 testes que percorrem famílias CB, operandos e carry; fórmula de referência interna, não oráculo de silício.
- `z80_indexed_cb_exhaustive_rotate_shift.rs`: teste exaustivo de rotate/shift indexed-CB na forma memória para valores/carry e prefixos DD/FD.
- `z80_ed_io_exhaustive_semantics.rs`: testes para IN/OUT ED, portas, WZ e estado/timing nominal.
- `z80_ed_block_io_reference_vectors.rs` e `z80_ed_block_transfer_compare_boundaries.rs`: vetores de operações de bloco, flags e efeitos em portas/memória.
- `z80_ed_block_temporal_trace.rs`: 20 testes de amostragem/trace temporal para o subconjunto de oito operações ED de bloco documentado no próprio teste.
- `z80_interrupt_semantics.rs`, `z80_nmi_halt_semantics.rs` e `z80_ei_delay_index_prefix.rs`: cenários dirigidos de estado e interrupção.
- `z80_memory_integration.rs`: execução de código de cartucho e acesso à RAM de trabalho do SMS.

O workflow `.github/workflows/sms-z80-platform-validation.yml` também integra o runner externo `raxoft/z80test`, com versões fixadas e notices de licença. A PR #201 registrou, no seu SHA candidato, 204/204 testes `z80full` e 204/204 `z80ccf`. Esse resultado é histórico do SHA candidato da PR #201; não deve ser transferido como execução no SHA atual sem consultar o workflow correspondente.

### 2.2 Limites da evidência SMS

- O corpus SingleStepTests/Z80 fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` tem ancestralidade no núcleo Z80 do Ares. É útil para regressão diferencial, mas não é independente de Ares nem prova de hardware.
- Os modelos matemáticos de ALU/CB/indexed-CB são independentes dos helpers de produção, mas continuam sendo modelos internos derivados das definições da instrução.
- O runner externo usa memória plana de 64 KiB. Não valida mapper, VDP, PSG, portas reais da plataforma ou scheduler da máquina.
- Os traces T-state atuais cobrem famílias ED específicas; não são um trace completo por opcode nem uma prova de todos os pinos físicos.
- A issues #158 e #159 continuam abertas. A matriz de classificação não equivale a semântica integral comprovada.

## 3. Proveniência e decisões históricas relevantes

### PR #160 — consolidação SG-1000 → SMS

O objetivo declarado da PR #160 foi consolidar no SMS o trabalho do SG-1000 sem perder as melhorias específicas do SMS. A revisão documentada registrou:

1. SMS já continha quase toda a implementação e a maioria dos testes do SG-1000.
2. SMS possuía atualizações adicionais de WZ/MEMPTR e matriz de classificação indexed-CB.
3. A atribuição de Q em `EX AF,AF'` proposta a partir do SG-1000 falhou no vetor SingleStepTests `08 0000` (Q esperado 0, observado 0x47) e foi removida.
4. A condição de DAA portada inicialmente do SG-1000 falhou no vetor `27 0000` (A esperado 0x93, observado 0xF3) e foi removida naquele candidato.
5. A consolidação foi integrada no merge `84dc09e0ba7b14153ad5f5b26809a072cb116faa`; a documentação registrou 57 vetores aprovados e um caso CPIR ainda ignorado naquele estágio.

### Evolução posterior

- A PR #201 corrigiu DAA no executor SMS após auditoria adicional e vetores fixos derivados do manual ZiLOG, além de adicionar varreduras combinatórias e o harness externo.
- PRs #161–#169 ampliaram vetores de memória, controle de fluxo, blocos, I/O e indexed-CB.
- PRs posteriores ampliaram prefixos, operações ED, estados especiais, traces ED e controle de fluxo.
- O código atual do SG-1000 contém a mesma lógica executável que o SMS, incluindo as correções posteriores. Portanto, não se deve afirmar que o SMS tem hoje um decoder diferente ou que o SG mantém a antiga fórmula de DAA.

Referências:
- [Auditoria histórica de consolidação SG-1000/SMS](../architecture/sg1000-sms-z80-consolidation-audit.md)
- [Matriz de cobertura de referência SMS](../architecture/z80-sms-reference-coverage.md)
- [Matriz CSV de opcodes SMS](../architecture/z80-sms-opcode-matrix.csv)
- [PR #160](https://github.com/leonardo201800478/sega-emulator/pull/160)
- [PR #201](https://github.com/leonardo201800478/sega-emulator/pull/201)
- [Issue #158 — semântica/opcodes](https://github.com/leonardo201800478/sega-emulator/issues/158)
- [Issue #159 — estado/interrupções/temporalidade](https://github.com/leonardo201800478/sega-emulator/issues/159)

## 4. Lacunas específicas do SG-1000

1. **Sem suíte dedicada de integração Z80:** não existe `native/sg1000_core/tests` no SHA auditado. Há 49 testes inline no arquivo CPU e 13 testes inline na máquina.
2. **Contrato de barramento duplicado:** SG-1000 mantém tipos locais equivalentes ao contrato de `sega-z80-bus`, enquanto SMS já reexporta o crate compartilhado.
3. **Cobertura de integração menor/diferente:** os testes inline SG incluem leitura de controle pelo Z80, escrita PSG/VDP na borda nominal de I/O e propagação/limpeza da interrupção VBlank. Eles não substituem a cobertura SMS de memória, prefixos, flags, ALU, ED, indexed-CB e vetores.
4. **Ausência de evidência por herança:** o fato de o executor SG ser uma cópia do SMS não prova que seu adaptador de barramento/memória/I/O e a integração VDP/PSG tenham os mesmos efeitos. Isso exige testes da máquina SG real.

## 5. Correção arquitetural

A direção anterior deve ser corrigida para explicitar o baseline:

- **Fonte canônica de semântica:** implementação SMS consolidada, por proveniência e maior densidade de testes de referência.
- **SG-1000:** consumidor do mesmo motor, com adapter próprio e testes de integração de máquina.
- **Novo crate:** `native/z80_cpu` / `sega-z80-cpu`, extraído do executor consolidado SMS; não escolher a cópia SG como origem independente.
- **Testes portáveis:** migrar os testes de CPU/ALU/CB/ED/prefixos/interrupções para `native/z80_cpu/tests`, adaptando o harness para o contrato público genérico. Não duplicar os 37 arquivos sob SG e SMS.
- **Testes de plataforma:** manter em `sms_core` os testes de mapper/cartucho/portas SMS; criar em `sg1000_core` testes próprios de memória, I/O, controle, VDP interrupt e PSG. Não transferir expectativas específicas de console.
- **Barramento:** migrar o SG para `sega-z80-bus` antes ou durante a extração, sem mudar o comportamento de mapeamento.
- **ALU:** a cópia `native/z80_alu` não é um oráculo independente: seu código normalizado coincide com as ALUs locais. Tratar o crate como candidato de produção a consolidar ou substituir, não como evidência independente de correção.
- **Semântica divergente:** não importar qualquer comportamento SG antigo sem vetor discriminante. A regra é preservar o comportamento consolidado SMS quando sustentado por evidência; investigar divergências reais contra documentação, vetores e testes independentes.
- **Temporalidade:** preservar stepping por instrução; o modo temporal futuro deve consumir a mesma semântica. Os traces atuais não autorizam declarar ciclo a ciclo.

## 6. Plano de correção incremental

### Z0 — baseline semântica e testes
- [x] Confirmar igualdade executável normalizada de CPU e ALU.
- [x] Identificar o SMS como fonte histórica/canônica e a assimetria dos testes.
- [x] Confirmar os 37 arquivos de teste dedicados do SMS e ausência de diretório equivalente no SG-1000.
- [ ] Recontar e classificar os testes SMS por semântica, modelo interno, vetor externo, integração e trace.
- [ ] Mapear quais testes atuais exercitam somente `Z80 + flat bus` e quais exercitam `SmsMemory`/dispositivos.

### Z1 — consolidação sem duplicação
- [ ] Criar `sega-z80-cpu` a partir do executor SMS consolidado.
- [ ] Migrar testes portáveis para o crate compartilhado, mantendo corpus, pins, licença e proveniência.
- [ ] Preservar os vetores que já falharam em tentativas de mudança de Q/DAA como regressões históricas, com expectativas atualizadas apenas por nova evidência documentada.
- [ ] Remover cópias locais de CPU/ALU somente depois de ambos os cores usarem o crate compartilhado e os testes passarem no SHA candidato.

### Z2 — integração SG-1000
- [ ] Substituir o contrato local de barramento pelo crate compartilhado.
- [ ] Criar testes de integração SG-1000 para execução de código, memória/cartucho, I/O, controller, VDP VBlank/INT e PSG.
- [ ] Verificar ordem de side effects e limites de endereços/portas.
- [ ] Reexecutar regressões SG-1000 e SMS; gameplay exige evidência no emulador.

### Z3 — validação
- [ ] fmt, Clippy e `cargo test --workspace --locked`.
- [ ] Workflows SMS Z80 Platform, SMS Core Fast e checks SG-1000 no SHA candidato exato.
- [ ] Testes diferenciais da mesma CPU por adapters SG/SMS com o mesmo programa/estado inicial, separando as diferenças esperadas de barramento.
- [ ] Revisão independente A4.
- [ ] Manter `SHORT_PASS`, `LONG_PASS` e `TEMPORAL_PASS` separados; a extração não eleva status automaticamente.

## 7. Estado após a auditoria

| Componente | Estado |
|---|---|
| Semântica executável CPU SMS/SG | Código normalizado idêntico; baseline de proveniência SMS |
| Maturidade da suíte semântica | SMS superior |
| Integração específica SMS | Maior corpus de testes dedicados |
| Integração específica SG | Parcial; precisa de testes próprios adicionais |
| ALU compartilhada | Código duplicado idêntico; consolidação pendente |
| Bus contract compartilhado | SMS usa crate; SG ainda mantém cópia |
| Crate CPU compartilhado | INTEGRATION_PENDING |
| Precisão ciclo a ciclo global | NOT DECLARED |
| Issues #158/#159 | OPEN |

**Correção principal:** o projeto não deve tratar SMS e SG-1000 como duas fontes equivalentes de maturidade. O SMS é o baseline semântico validado; SG-1000 deve ser preservado como plataforma e validado por adapters próprios. A consolidação deve centralizar a implementação e o corpus de CPU, sem duplicar testes nem perder as regressões específicas de cada máquina.
