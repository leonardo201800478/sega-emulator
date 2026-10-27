> **Estado vigente (2026-10-27):** runtime SG-1000 ativo em Rust. O hardware Python antigo e os testes/ferramentas que dependiam dele estão arquivados em `legacy/python_reference/` e não devem ser executados/importados. Este documento preserva observações históricas; comandos apontando para os caminhos arquivados não são comandos ativos.

# Modelo total do Z80 em Rust — núcleo comum e quatro perfis de plataforma

- **Status:** proposta técnica baseada nas fontes existentes; requisitos para implementação e validação, não declaração de que a CPU já esteja completa.
- **Escopo:** SG-1000, Master System, Game Gear e Z80 auxiliar do Mega Drive/Genesis.
- **Branch de documentação:** `docs/z80-total-model-four-platform-profiles`.
- **Base auditada:** `main` após o merge da PR #118 (`4263f3871a1729c16e643e98132c74221ad8feec`). A fatia de opcodes da #118 está integrada; o núcleo Z80 continua parcial.
- **Linguagem oficial do núcleo:** Rust. Python fica para UI, carregamento de ROM/perfis, fachada PyO3 e infraestrutura de testes/ferramentas que valida Rust; os oráculos de hardware Python estão arquivados.

## 1. Decisão central

O alvo é **um modelo semântico Z80 completo em Rust, reutilizado por quatro perfis de plataforma**, não quatro cópias independentes da CPU. Os documentos atuais de arquitetura determinam que o núcleo de CPU seja separado do console e que as diferenças reais de barramento, temporização e periféricos sejam modeladas na plataforma. As fontes consultadas não demonstram quatro revisões de silício Z80 distintas nesses quatro consoles; portanto, “quatro variantes” significa inicialmente quatro configurações de integração. Diferenças de silício/clone só serão separadas quando houver evidência específica.

O Mega Drive/Genesis usa um Motorola 68000 como CPU principal e um Z80 auxiliar para funções associadas ao subsistema de áudio. A variante Genesis deste plano é o **perfil de integração do Z80 auxiliar**, não uma substituição do 68000. O núcleo 68000 permanece fora do escopo desta especificação Z80.

## 2. Três frentes de pesquisa e síntese

As frentes abaixo foram conduzidas como trilhas de análise paralelas a partir dos documentos existentes. Elas não são apresentadas como agentes autônomos externos que tenham executado trabalho independente.

### Frente A — ISA, decoder e prefixos

**Fontes:** manual Zilog *Z80 CPU User Manual*, UM008011-0816 (2016), capítulos de arquitetura, temporização e instruções; `docs/audits/z80-opcode-matrix-review.md`; `docs/architecture/z80-coverage-gap-model.md`; código Rust atual em `native/sg1000_core/src/cpu.rs`.

**Achados sustentados pelas fontes:**
- O decoder deve distinguir espaço base, CB, ED, DD, FD, DD-CB e FD-CB. Contar encodings que retornam sem erro não prova semântica correta.
- DD/FD pode ser prefixo efetivo, prefixo ignorado ou forma que substitui IX/IY. É necessário classificar cada encoding, não assumir que todo DD/FD altera H/L.
- As formas indexadas com `(IX/IY+d)` têm exceções importantes: em operações específicas com memória indexada, os registradores H/L não significam automaticamente IXH/IXL ou IYH/IYL.
- O espaço ED deve distinguir instruções atribuídas, aliases documentados e bytes não atribuídos; o comportamento NOP de ED não pode ser confundido com implementação de uma instrução.
- O núcleo Rust continua incremental e retorna `UnsupportedOpcode` para famílias ainda não implementadas. A PR #118 integrou parte das instruções base (rotações do acumulador, DAA, CPL/SCF/CCF, DJNZ e RST); CB/DD/ED/FD, registradores alternativos e interrupções continuam pendentes.

**Saída necessária:** matriz gerada e versionada de todos os espaços de opcode, com bytes/prefixos, mnemônico canônico, classificação, comprimento, T-states nominais, efeitos de estado e referência do oráculo.

### Frente B — Estado arquitetural, flags e eventos

**Fontes:** manual Zilog citado; `docs/architecture/z80-coverage-gap-model.md`; `docs/adr/0007-rust-first-development-policy.md`; auditoria diferencial SingleStepTests documentada no repositório.

**Modelo mínimo de estado:**
- Registradores principais: AF, BC, DE, HL; SP e PC.
- Conjunto alternativo: AF', BC', DE', HL' e operações de troca.
- Registradores de índice IX/IY e formas de acesso a suas metades quando suportadas pelo modelo de compatibilidade.
- I e R (incluindo a regra de incremento/refresh observável por instrução e prefixo).
- Estado de interrupção: IFF1/IFF2, modo IM 0/1/2, EI-delay, NMI pendente/aceita e sinais de INT.
- HALT, estado de reset e estado de execução necessário para retomar deterministamente.
- Estado auxiliar interno quando necessário para reproduzir resultados observáveis, incluindo Q e WZ/MEMPTR se a evidência/testes adotados exigirem esses latches.
- Contadores temporais monotônicos; não misturar estado arquitetural com instrumentação de diagnóstico.

**Flags:** S, Z, Y/F5, H, X/F3, P/V, N e C. Cada família de instrução deve declarar quais flags preserva, limpa, define ou calcula. O modelo deve separar flags documentados de comportamentos não documentados comprovados por oráculo ou evidência de hardware.

**Eventos obrigatórios:** reset; interrupções mascaráveis e não mascaráveis; aceitação/rejeição de interrupções nos limites de instrução; EI com atraso de aceitação; HALT; chamadas de interrupção; repetição de instruções de bloco; wraparound de PC/SP/endereço; efeitos de I/O e memória. O modelo não deve deduzir comportamento de uma família com base em outra família “parecida”.

**Saída necessária:** testes de transição de estado por família, com entradas e saídas explícitas para registradores, flags, PC/SP, R, Q/WZ quando aplicável, memória, portas e interrupções.

### Frente C — Temporização, barramento e integração de plataforma

**Fontes:** seção de temporização e ciclos de interrupção do manual Zilog; `docs/architecture/z80-coverage-gap-model.md`; `docs/architecture.md`; ADR 0006/0007; workflows de auditoria existentes.

**Conclusão importante:** retornar T-states nominais por instrução não equivale a executar os ciclos de barramento em ordem. A documentação existente já registra que o contrato atual acessa memória em nível de instrução e não possui traço temporal explícito de M-cycles/T-states. Logo, não declarar precisão ciclo a ciclo antes de implementar e validar esse contrato.

**Evolução sugerida em duas camadas:**
1. Contrato funcional estável para leitura/escrita de memória, leitura/escrita de I/O, sinais de interrupção, HALT/reset e avanço determinístico.
2. Contrato temporal capaz de expor M1/opcode fetch, memory read/write, I/O read/write, interrupt acknowledge, estados de espera/contensão quando aplicáveis e ordem de eventos. Não simular contenção se a plataforma não a exigir ou a fonte não a sustentar.

**Saída necessária:** interface Rust independente da plataforma, esquema de traço de barramento e oráculos para validar primeiro estados finais e ciclos nominais, depois ordem de acessos e temporização fina.

## 3. Arquitetura Rust proposta

Estrutura conceitual; os nomes finais de crates/módulos devem ser alinhados ao código real antes de implementar.

```text
native/
  z80_core/                  # núcleo semântico compartilhável e API Rust pura
    registers.rs
    flags.rs
    decoder/
      base.rs
      cb.rs
      ed.rs
      index.rs               # DD/FD e substituições IX/IY
      index_cb.rs            # DD-CB/FD-CB
    execute/
      alu8.rs
      alu16.rs
      load.rs
      branch.rs
      stack.rs
      block.rs
      io.rs
      interrupt.rs
    timing.rs                # T-states nominais e fases de barramento
    state.rs                 # CPU state, reset, HALT e latches necessários
    bus.rs                   # contratos sem dependência de console
    tests/
  sega_platforms/             # composição por plataforma (nome ilustrativo)
    sg1000_z80.rs
    master_system_z80.rs
    game_gear_z80.rs
    genesis_sound_z80.rs
```

O núcleo não deve importar classes do SG-1000, Master System, Game Gear, Genesis, PyO3 ou Python. O código da plataforma fornece implementação dos contratos de barramento, clocks/scheduler e sinais. Os bindings PyO3 devem chamar operações de granularidade alta e não atravessar a fronteira por instrução/byte no caminho crítico.

O núcleo deve oferecer, no mínimo:
- `reset()`, captura/restauração de estado serializável versionado, `step_instruction()` e `run_until_event()` quando a semântica do scheduler estiver definida;
- resultado explícito para estado de parada, evento de interrupção, erro de configuração e instrução não classificada durante a migração;
- estado de CPU observável para teste diferencial, sem expor mutabilidade insegura de registradores à plataforma;
- execução determinística para mesma configuração, estado inicial, sequência de sinais e traço de barramento.

Não esconder opcode ausente como NOP. O comportamento ED-NOP só pode ser usado nos bytes que a matriz e a evidência classifiquem assim.

## 4. Quatro perfis de plataforma

| Perfil | Papel do Z80 | Responsabilidades específicas fora do núcleo semântico | Questões que precisam de fonte/teste |
|---|---|---|---|
| **SG-1000** | CPU principal | Mapa de memória/cartucho, RAM, I/O, linhas de interrupção, integração TMS9918A/PSG e scheduler | Configuração real de clock/reset, portas, bordas de interrupção e ordem temporal |
| **Master System** | CPU principal | Mapa de memória e slots/bancos, I/O do console, VDP, PSG/FM quando aplicável e scheduler regional | Diferenças por revisão/região, mapa de portas, comportamento do VDP/interrupts e acesso ao barramento |
| **Game Gear** | CPU principal | Perfil SMS compatível onde comprovado, mapa/portas portáteis, VDP/display e I/O específico | Diferenças portáteis, portas adicionais, timing/região e eventuais particularidades de hardware |
| **Mega Drive / Genesis** | CPU auxiliar de som | Z80 address space, RAM Z80, portas de comunicação com o 68000, controle de barramento/reset, integração com YM2612/PSG e sincronização com o scheduler principal | Ownership e arbitragem do barramento, janela de acesso, linhas de controle e ordem de eventos de áudio |

As fontes de arquitetura dão suporte à reutilização do Z80 entre plataformas compatíveis e à necessidade de separar mapa, barramento, periféricos e scheduler. Esta tabela é um plano de modelagem; não afirma que as quatro integrações já existam ou que todas as particularidades listadas estejam validadas. Os valores concretos de clocks, portas e timing devem vir das fontes técnicas já documentadas e de testes rastreáveis, nunca de suposição.

### Contrato do perfil

Cada perfil deverá fornecer:
- identidade e revisão de hardware escolhida;
- implementação do espaço de memória e portas;
- origem e política dos sinais INT/NMI/RESET/BUSREQ/HALT conforme existirem naquela máquina;
- regra de avanço temporal e conversão entre ciclos do Z80 e relógio/scheduler do sistema;
- política explícita para acesso negado, wait states e contensão;
- dispositivos conectados e eventos externos que podem afetar a CPU;
- testes de integração e vetores de regressão exclusivos da plataforma.

O núcleo compartilhado não deve conhecer “qual console” está executando. Se uma diferença comprovada exigir comportamento semântico diferente de CPU, representá-la como configuração/variant policy nomeada e testada; só criar implementação divergente se a diferença não puder ser modelada com segurança como parâmetro.

## 5. Matriz total de cobertura

O critério de completude será semântico, não apenas “todos os bytes executaram”.

| Dimensão | Cobertura exigida |
|---|---|
| Decoder | Base, CB, ED, DD, FD, DD-CB e FD-CB; todos os bytes classificados |
| Semântica | Operações aritméticas/lógicas, loads, stack, branches/calls/returns, exchanges, block transfer/search/I/O, rotates/shifts, bit set/reset/test, input/output, control-flow |
| Flags | Todas as flags afetadas/preservadas por instrução, incluindo regras de overflow/paridade, half-carry, N e X/Y |
| Prefixos | Substituição IX/IY, prefixos ignorados legítimos, deslocamentos assinados e exceções H/L com memória indexada |
| Estado oculto | R, I, alternativos, Q/WZ quando requeridos, IFF1/IFF2, EI delay, HALT e estado de interrupção |
| Endereçamento | Immediate, register, register-indirect, absolute, relative, indexed displacement, port addressing, stack |
| Limites | PC/SP/address wraparound, deslocamento -128/+127, carry/borrow, contadores de bloco 0/1, alinhamentos e memória/portas na fronteira |
| Interrupções | IM 0/1/2, NMI, prioridade, acknowledge, retorno, EI/DI, HALT e timing de aceitação |
| Temporização | T-states nominais por caminho; prefix fetches; M-cycles e ordenação do barramento conforme o contrato avançar |
| Determinismo | Estado serializado, reinício, execução repetida, checkpoints e mesma saída com mesma sequência de eventos |
| Integração | Cada perfil, interrupções de dispositivos, acesso à memória/I/O e sincronização com o scheduler da máquina |

Cada célula da matriz precisa indicar: `documented`, `observed-by-oracle`, `inferred`, `hypothesis` ou `not-implemented`; fonte/revisão; teste de referência; resultado no Rust; e diferença conhecida em relação ao comportamento documentado/oráculo independente.

## 6. Política de testes e evidência

### Camada 1 — unidade Rust
- Testes por família e tabela de opcode.
- Testes exaustivos de domínios pequenos (por exemplo, combinações de entradas/flags para ALU e DAA quando viável).
- Vectors fixos independentes, não expectativas calculadas pelo mesmo helper que implementa a operação.
- Testes de overflow, wraparound, flags preservadas, stack, memória e portas.

### Camada 2 — diferencial de estado
- Fixar commit e checksum do corpus SingleStepTests já referenciado no projeto.
- Comparar A/F, pares de registradores, IX/IY, SP/PC, I/R, flags, memória, HALT e estado de interrupção/latches quando presentes no formato do corpus.
- Reportar primeiro campo divergente e caso mínimo reproduzível. Corpus selecionado é evidência parcial, não prova de completude absoluta.

### Camada 3 — temporização e bus trace
- Primeiro comparar T-states nominais por instrução e caminho.
- Depois comparar eventos ordenados: fetch/M1, leitura/escrita de memória, I/O e acknowledge.
- Manter traces dourados só quando houver fonte/oráculo confiável e procedência documentada.
- “Cycle-accurate” permanece não comprovado até o modelo temporal e os traces passarem critérios definidos.

### Camada 4 — integração por perfil
- Testes de CPU com barramento de teste e testes com barramento de plataforma.
- ROMs sintéticas/homebrew e entradas determinísticas.
- Checkpoints determinísticos do runtime Rust contra vetores/oráculos independentes; referências Python antigas não são executadas.
- Mesma sequência de entradas/eventos deve gerar estados e hashes repetíveis; hashes iguais não substituem comparação de estados nem provam correção absoluta.

### CI e comandos
Os workflows existentes incluem auditoria funcional, integridade, inventário, corpus diferencial e checks Rust. Os jobs “Agent” já documentados são jobs automatizados do GitHub Actions, não prova de agentes de IA autônomos. Para cada PR, confirmar os workflows realmente disparados no SHA candidato, logs e artefatos.

Comandos Rust obrigatórios para mudanças no workspace:
```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Para `native/z80_core`, validar também o manifest path, build Release/Maturin e os testes de integração Python/PyO3 afetados, conforme `AGENTS.md`.

## 7. Fases de implementação

1. **Baseline e inventário:** fixar o SHA de `main` após a integração da PR #118; inventariar o estado real do núcleo Rust e os oráculos existentes.
2. **Contrato e estado completo:** definir tipos Rust para registradores, flags, interrupções, estado de execução, bus e resultado de step; preservar APIs estáveis.
3. **Decoder/matriz semântica:** gerar matriz total e classificar cada encoding antes de preencher lacunas.
4. **Instruções base e prefixos:** implementar famílias por fatias compiláveis com regressões para cada mudança; CB, ED, DD/FD e index-CB não devem ser tratados como um único megacommit.
5. **Flags/estado oculto/interrupções:** validar Q/WZ, R, EI-delay, HALT, IM0/1/2, NMI e block instructions com oráculos rastreáveis.
6. **Temporização e barramento:** ampliar de contagem nominal para eventos ordenados, sem afirmar precisão que ainda não foi demonstrada.
7. **Perfis de plataforma:** validar SG-1000 primeiro; depois Master System, Game Gear e Genesis em trilhas separadas, reutilizando o núcleo e acrescentando testes de integração próprios.
8. **Migração oficial (SG-1000):** Rust é a única implementação ativa; as referências Python foram arquivadas após remover consumidores de produção.

## 8. Critérios de “modelo total”

O modelo só poderá ser marcado como completo quando:
1. todos os encodings documentados do modelo Z80 escolhido estiverem classificados e semanticamente implementados;
2. os comportamentos não documentados suportados tiverem política explícita de silício/compatibilidade e evidência;
3. flags, registradores alternativos, prefixos, interrupções, HALT e efeitos de memória/I/O forem cobertos por testes;
4. a matriz semântica estiver ligada a oráculos, resultados e checksums;
5. T-states nominais estiverem validados; a precisão de barramento for declarada separadamente;
6. os quatro perfis passarem testes de integração específicos;
7. CI obrigatória estiver verde no SHA revisado e as lacunas restantes forem classificadas, não ocultadas;
8. documentação e implementação coincidirem.

## 9. Fontes examinadas

### Manuais anexados para esta pesquisa
- Zilog, *Z80 CPU User Manual*, documento **UM008011-0816**, 2016, 332 páginas. Fonte primária para organização de registradores, sinais, instruções, interrupções e temporização documentadas. Referenciar os capítulos de arquitetura, timing, interrupt response e instruction set ao elaborar cada caso.
- Zilog, *Z80 Family CPU User Manual*, documento **80-XXXX-10XX** conforme o PDF fornecido, 306 páginas. Usar como corroborador e registrar diferenças de edição quando houver divergência.
- *Zilog Z80-CPU Technical Manual*, PDF fornecido, 78 páginas. O arquivo apresenta erros estruturais de PDF durante extração textual; não usá-lo como única evidência para um detalhe até confirmar a página visualmente ou corroborar no manual legível.

### Documentação do repositório
- `docs/architecture.md` — separação CPU/plataforma e alvos por console.
- `docs/adr/0006-standalone-rust-sg1000-core.md` — extração incremental para Rust puro e critérios standalone.
- `docs/adr/0007-rust-first-development-policy.md` — Rust-first, testes e fronteira PyO3.
- `docs/architecture/z80-coverage-gap-model.md` — três trilhas de auditoria, matriz semântica, estado oculto, temporização e limites da evidência.
- `docs/audits/z80-opcode-matrix-review.md` — classificações DD/FD e distinções entre formas indexadas documentadas e não documentadas.
- `AGENTS.md` e workflows `z80-extended-audit.yml`, `z80-singlestep-audit.yml`, `rust-sg1000-core.yml` e `qt-rust-z80-core.yml` — critérios de validação e automação.
- Código atual consultado: `native/sg1000_core/src/cpu.rs`; PR #118 registra explicitamente que a implementação ainda é parcial.

### Oráculos já referenciados
- SingleStepTests/Z80, commit fixado `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`, manifesto e verificações de checksum documentados no repositório. É uma fonte de vetores de estado de instrução, não por si só um oráculo universal de traces elétricos de barramento.
- As notas diferenciais já existentes sobre flags X/Y em instruções de bloco são evidência observacional do corpus e devem permanecer identificadas como tal, sem generalização automática a todo silício Z80.

## 10. Pendências explícitas

- Esta proposta não altera ainda o executor da CPU nem prova que os quatro perfis estejam integrados.
- A matriz total precisa ser gerada/validada a partir do conjunto documentado e ligada a testes concretos.
- Não foi realizada auditoria física de hardware para resolver todos os comportamentos não documentados.
- A terceira fonte PDF contém erros de extração; detalhes nela dependentes precisam de confirmação.
- Os detalhes de relógio, contenção, I/O e arbitragem dos perfis devem ser extraídos das fontes específicas de cada console já documentadas no repositório, sem extrapolar do manual Z80 genérico.
