# Z80 compartilhado — scorecard de conclusão

**Baseline de main consultada:** `9d5d83d0df16a1c6fa9fb82bdd953af54369c613` (2026-11-09). As PRs documentais #703 e #704 reconciliaram o ledger sem alterar o runtime. A validação externa no HEAD da PR #702 totalizou 34.000 vetores PASS; evidências nas seções 22–24.
**Reconciliação:** PR #672 integrou GUI/D3D12; PR #673 adicionou teste ED; PR #674 reconciliou evidências; PR #675 adicionou o B-loop de flags; PRs #677/#678 adicionaram observação IM2, handler PUSH/POP AF, EI/RETI e retomada do bloco. Nenhuma alterou a semântica de runtime do Z80.
**Nota de reconciliação:** as PRs #615 (filtro/reset SN76489) e #617 (clarificação documental Rust/egui) não alteraram o núcleo Z80, decoder DD/FD ou suas evidências. O conteúdo técnico e as conclusões semânticas abaixo permanecem conforme as auditorias de Z80 registradas.
**Política vigente:** [ADR-0016 — fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).
**Escopo:** um único núcleo Z80 compartilhado pelos consoles que realmente utilizam essa CPU.

Este scorecard mede o que é necessário para executar corretamente SG-1000 e Master System. Não define uma meta de simulação elétrica, cobertura integral de pinos ou microsequenciamento de cada instrução sem dependência de plataforma.

## 1. Critérios funcionais

| Área | Critério |
|---|---|
| Instruções documentadas | Implementar corretamente as instruções que o software dos consoles suportados pode executar |
| Registradores e flags | Preservar os resultados arquiteturais e os efeitos observáveis |
| Prefixos | Tratar corretamente CB, ED, DD, FD, DDCB e FDCB nos caminhos suportados |
| Interrupções | Reproduzir os modos e as transições usados pela plataforma |
| HALT / reset | Preservar comportamento funcional e retomada correta |
| Estado compartilhado | Uma implementação Rust, sem cópias de CPU por console |
| Regressão | Alterações no Z80 não podem quebrar SMS nem SG-1000 |

## 2. Estado conhecido na baseline consultada

- O núcleo Z80 compartilhado está em `native/z80_cpu`.
- E3A1/E3A2 e E4A/E4B/E4C constam como integradas nas auditorias recentes.
- E5 DDCB/FDCB e E6 resolução geral de prefixos constam como integradas na main.
- E7.3/HALT T4 consta como integrada.
- A entrega VBlank/Line IRQ do SMS convencional permanece coberta pelos testes integrados da PR #531. A ligação da IRQ de nível do VDP à INT do Z80 temporal foi integrada pela PR #595: combina a fonte do VDP com INT externa, preserva a fonte externa no snapshot e valida o vetor de acknowledge no restore. O CI #5178 passou nos testes VBlank/IM1/HALT e de preservação da fonte externa; isso não fecha a auditoria semântica global E7.7.
- A semântica do pino RESET do Z80 está coberta por testes unitários e de integração SMS na PR #536: PC/I/R zerados, IFF1/IFF2 desabilitados, IM 0, HALT/NMI/EI delay limpos; registradores gerais/alternativos, IX/IY, SP, linha INT externa e T-states da máquina preservados.
- A duração elétrica mínima do pulso RESET não é simulada: o reset de host é uma transição atômica de estado, conforme ADR-0016; não há consumidor de gameplay demonstrado para a largura do pulso.
- O fechamento semântico global E7.7 ainda não foi comprovado como concluído. A auditoria de I/O em bloco agora possui evidência física parcial para flags no ciclo de repetição NMOS e fixture independente no Rust; a execução do ROM v5.0 completo, flags de iteração simples em todo o espaço, CMOS/clones e validação direta em hardware SMS permanecem pendentes; a observação IM2/RETI adaptada já está coberta por testes do core. `OUT (C),0` está resolvido para o perfil NMOS das placas SMS documentadas; não generalizar para todos os clones/variantes.
- O caminho de barramento/temporização está implementado apenas em fatias; não há prova de integração temporal sistêmica completa.

Esta lista é uma fotografia histórica. Antes de alterar código ou declarar qualquer critério concluído, confirmar os arquivos, testes, PRs e workflows no SHA atual da `main`. PR aberta ou Draft não equivale a implementação integrada.

## 3. Política temporal do Z80 — prioridade de gameplay e baseline atômica

A semântica de cada instrução suportada deve ser correta, independentemente do caminho de execução. A implementação atômica é suficiente como padrão para instruções cujo estado intermediário não seja observável pelo console. A precisão temporal detalhada é prioritária para sequências executadas por jogos quando a ordem/duração altera uma interação com o hardware.

| Caminho | Nível obrigatório | Critério |
|---|---|---|
| ALU/registradores internos sem acesso externo | Atômico | Resultado, flags, PC/R e duração nominal correta; sem microfases obrigatórias |
| Fetch/decode e prefixos | Atômico por instrução como base | Bytes consumidos, seleção de prefixo, PC/R e semântica exatos; detalhar M1 somente se consumidor exigir |
| Acesso a RAM/ROM sem contenção ou efeito temporal | Atômico | Endereço, dado, side effects e duração nominal |
| Escritas/leitura de I/O e registradores VDP/PSG | Temporal prioritário quando o instante afeta gameplay | Ordenação relativa à CPU/VDP/áudio e efeito no instante correto |
| Mapper/banco de cartucho | Atômico por transação, temporal se a troca interagir com execução em andamento | Estado de banco e ordem observável corretos |
| INT/NMI, EI delay, HALT, acknowledge e leitura de status | Temporal nos pontos de amostragem/aceitação observáveis | Prioridade, fronteira de instrução e efeitos colaterais corretos |
| Raster/H-counter, scroll e writes intralinha | Temporal quando jogos usam a interação | Reproduzir a menor fronteira temporal demonstrada |
| WAIT/arbitragem/DMA | Condicional | Exigir ligação real no console e efeito reproduzível |
| Pinos e fases elétricas | Fora do escopo | Não são requisito de conclusão |

**Regra de leitura do scorecard:** `IMPLEMENTED_ATOMIC` pode satisfazer integralmente o requisito funcional de uma família. `NOT_IMPLEMENTED` no microsequenciador indica apenas ausência daquele caminho temporal, não ausência da instrução no executor atômico. Só registrar uma lacuna temporal como bloqueio quando houver consumidor de gameplay e evidência que exija essa granularidade.

Uma representação diagnóstica de sinais pode ser derivada do estado do core, mas nunca deve criar uma segunda CPU ou caminho de execução.

## 4. Testes e validação

- Testes unitários por instrução/família e por estado arquitetural alterado.
- Testes de integração com os barramentos e periféricos dos consoles suportados.
- Testes de regressão SG-1000 e SMS.
- Testes diferenciais e ROMs de diagnóstico quando agregarem evidência independente.
- Testes temporais detalhados apenas para interações cuja ordem/duração altere o resultado observável.
- Desempenho Release medido quando houver mudança no caminho crítico.

ZEXDOC, ZEXALL, traces completos de pinos e validação de silício podem ser ferramentas auxiliares, não gates universais de produto. Nenhum resultado de teste deve ser declarado sem execução no SHA correspondente.

## 5. Trabalho temporal em pausa

As PRs #512, #514 e #526 permanecem abertas/Draft na consulta de 2026-11-05 e não estão integradas na main. Seus contratos de M-cycle/T-state e timeline racional não são tarefas ativas até que uma dependência concreta seja demonstrada sob a ADR-0016.

A PR #527 é uma proposta documental não integrada. Não usar os relatórios dessa PR como autoridade normativa.

## 6. Regra de atualização

Atualizar este scorecard somente quando uma mudança integrada, um teste executado ou uma evidência de hardware alterar o estado de um critério funcional relevante. Não criar fases, percentuais de completude ou bloqueios de precisão elétrica sem necessidade comprovada.

## 7. Reconciliação de evidências — 2026-11-06

Baseline histórica: `main` = `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`. As evidências específicas de cada gate permanecem vinculadas aos SHAs e workflows indicados nas auditorias.

- PR #562 integrou a correção do erratum NMOS de P/V em `LD A,I` / `LD A,R` no microsequenciador; INT imediato, instrução intermediária, NMI e snapshot/restore têm testes dedicados.
- PR #563 integrou as matrizes de regressão de I/O em bloco e dos aliases DD/FD half-register (LD, ALU e INC/DEC), além das auditorias correspondentes.
- A auditoria `docs/audits/z80-e7-block-io-flags-2026-11-05.md` caracteriza as fórmulas de flags de INI/IND/INIR/INDR/OUTI/OUTD/OTIR/OTDR para o perfil Zilog NMOS.
- CMOS/clones em geral e validação direta em hardware SMS permanecem NOT_VERIFIED; `OUT (C),0` está RESOLVED_FOR_DOCUMENTED_SMS_NMOS_PROFILE, com variantes fora desse perfil ainda não verificadas.
- E7.7 continua IN_PROGRESS e E8 continua BLOCKED.

## 8. Registro histórico após PR #595 — fotografia de 2026-11-06

A main estava em `733152f61b706b2243914ae017643cef949d4ae1`. A PR #595 alterou integração temporal SMS e testes/documentação; não alterou o decoder Z80, as famílias DD/FD ou os resultados E4C/E5/E6.

- A conexão VDP IRQ → Z80 INT temporal foi integrada, com preservação da fonte externa e validação de snapshot/acknowledge.
- E7.7 permaneceu IN_PROGRESS; E8 permaneceu BLOCKED.

## 9. Reconciliação após PR #597 — fotografia histórica de 2026-11-06

A main estava em `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`. A PR #597 alterou a fronteira do worker de áudio SMS, não o núcleo Z80. E7.7 permaneceu IN_PROGRESS; E8 permaneceu BLOCKED.

## 10. Baseline atual — 2026-11-07

A main atual consultada é `3ad1781f84b7824b5aa889a583ae6d4545987ec1`. As PRs #615 e #617 não alteraram a implementação ou a evidência do Z80. As PRs #613/#614 ainda não foram integradas na fotografia desta baseline.

- E4C, E5 e E6 permanecem integradas conforme evidências anteriores.
- E7.7 permanece IN_PROGRESS.
- E8 permanece BLOCKED; não iniciar ZEXDOC/ZEXALL antes do fechamento E7.
- A auditoria semântica deve continuar com lacunas específicas, incluindo `OUT (C),0` variante-dependente, sem exigir microsequenciamento universal.


## 11. Reconciliação documental — baseline `main` de 2026-11-07

**Baseline auditada:** `e77eff26d80fd77a64546373c3e92326a0cc4e7a` (merge da PR #628).

Esta seção supersede as referências anteriores de baseline como fotografia corrente; as seções históricas permanecem preservadas para rastreabilidade.

- PR #613 (`ec08703d8e07ed7f91283e7db0787d2457273853`) integrou o runtime SMS temporal/vídeo à aplicação e alterou `native/sega_app`, `native/video_runtime` e `rust/runtime/sms-runtime/src/worker.rs`. Não alterou `native/z80_cpu`, o decoder DD/FD ou as fórmulas de flags do Z80.
- PR #614 (`63eb154acedb993cdaf944746cb1f46cc8ccb3c6`) integrou áudio e controles na aplicação; não alterou o núcleo Z80.
- PR #628 (`e77eff26d80fd77a64546373c3e92326a0cc4e7a`) alterou o workflow CI e documentação de validação; não alterou runtime Z80, decoder DD/FD, microsequenciador ou evidências semânticas.
- As conclusões de E4C, E5 e E6 permanecem as registradas nas auditorias anteriores. Esta reconciliação não reexecuta nem promove seus testes.
- **E7.7 permanece IN_PROGRESS.** A auditoria semântica global não está encerrada; continuam pendentes comprovação para variantes CMOS/clones e validação direta em hardware SMS; `OUT (C),0` está resolvido apenas para o perfil NMOS das placas SMS documentadas.
- **E8 permanece BLOCKED.** Não iniciar ZEXDOC/ZEXALL antes do fechamento documentado de E7.
- A integração de aplicação/runtime SMS não comprova por si só a precisão temporal sistêmica do Z80. Não declarar TEMPORAL_PASS global nem equivalência contínua/em blocos sem testes específicos.

Nenhuma alteração de código Z80 decorre desta reconciliação documental.


## 12. Baseline remota e fechamento da subquestão OUT (C),0 — 2026-11-07

**Baseline atual auditada:** `main` = `43323df653497a98c7afae52a6f3ea198f46cdb0`.

- PR #632 integrou esta reconciliação documental de scorecard/matriz no merge SHA `415861b7c550512e0380ac85b273748de796b5f3`.
- PR #634 integrou o registro das validações de CI no merge SHA `43323df653497a98c7afae52a6f3ea198f46cdb0`. Nenhuma das duas PRs alterou o runtime Z80.
- CI pós-merge da PR #634: run [42379523450](https://github.com/leonardo201800478/sega-emulator/actions/runs/42379523450), SUCCESS no SHA exato desta baseline. O job de Z80 focused e Scheduler stress foi SKIPPED por classificação de impacto; não é validação semântica do Z80.
- A investigação E7.7 específica de `ED 71` (`OUT (C),0`) está registrada em [z80-e7-77-out-c-zero-variant-2026-11-07.md](audits/z80-e7-77-out-c-zero-variant-2026-11-07.md).
- Para as placas SMS documentadas e examinadas, as peças listadas são Zilog Z0840004PSC e, em uma revisão inicial PAL, NEC D780C-1 ou Zilog Z0840004PSC. A evidência do fabricante classifica Z08400 como NMOS; a família Z84C00 é CMOS. A referência experimental/comparativa atribui saída zero ao ED 71 no Z80 NMOS e FF no Zilog CMOS, com exceções entre clones.
- O runtime integrado escreve `0x00` para ED 71 e tem teste de I/O que confirma a transação `(BC, 0x00)`. Esse comportamento corresponde ao perfil NMOS das placas SMS documentadas. Não foi necessária alteração de runtime.
- **Limite:** o levantamento de placas não é um censo de todas as unidades/revisões produzidas. Não inferir que todos os clones CMOS ou todas as variantes Z80 tenham comportamento idêntico. A exceção Sharp LH5080A demonstra que uma regra binária “CMOS = FF” não é universal.
- **Estado E7.7:** a subquestão ED 71 fica RESOLVED_FOR_DOCUMENTED_SMS_NMOS_PROFILE, com evidência documental de componentes e regressão de runtime. A auditoria semântica integral E7.7 continua IN_PROGRESS; CMOS/clones em geral e outras famílias/efeitos ainda exigem reconciliação. **E8 permanece BLOCKED.**


## 13. Reconciliação da main — 2026-11-09

**Baseline auditada:** `main` = `ca1b26089bd649a9248fab4ecbe47b78bc2eb49d`.

- PR #672 integrou a composição dos menus egui sobre o viewport D3D12; sem alterações no Z80.
- PR #673 integrou `ed_absolute_word_transfers_wrap_address_and_wz_at_ffff`, cobrindo ED 43/53/63/73 e ED 4B/5B/6B/7B no limite `nn=0xFFFF`: wrap FFFF→0000, ordem little-endian, WZ=0000, PC, F preservada e 20 T-states. CI #42568163213 passou no HEAD exato `b6f94ff010782a5dd18e6c684e625c2eec9d7a6e`; merge SHA `ca1b26089bd649a9248fab4ecbe47b78bc2eb49d`.
- A auditoria de I/O em bloco agora referencia o teste independente `Z80 Block Flags Test v5.0`, de Peter Helcmanovsky. O projeto de teste documenta validação em hardware Z80 NMOS e o wiki registra resultados em diversos chips. Seu escopo é específico: flags de instruções de bloco quando a repetição é interrompida por IM2, incluindo PF/HF para INxR/OTxR. O próprio autor ressalta que o teste é limitado e não exaustivo.
- Essa evidência reduz a lacuna de proveniência para a subquestão de flags da repetição em hardware NMOS; não constitui validação direta no Master System, nem prova exaustiva das flags de uma iteração simples, de todos os valores/estados ou de clones CMOS.
- Os testes atuais `block_io_repeat_flags_cover_half_carry_and_parity_edges` e `block_io_normal_flags_ports_and_wz_match_documented_edge_vectors` são regressões dirigidas. A comparação atomic↔microsequencer não é um oráculo independente, pois os caminhos podem compartilhar a mesma hipótese semântica.
- **E7.7 permanece IN_PROGRESS.** A auditoria semântica global ainda exige reconciliação de todas as famílias e efeitos observáveis, além de execução independente dos testes ROM/hardware pertinentes.
- **E8 permanece BLOCKED.** Não iniciar ZEXDOC/ZEXALL antes do fechamento formal de E7.
- E4C, E5 e E6 não foram alteradas nem reabertas por estas PRs.

Auditoria complementar: [E7.7 — evidência de hardware para flags de I/O em bloco](audits/z80-e7-block-io-hardware-evidence-2026-11-09.md).


## 14. Reconciliação após PR #675 — 2026-11-09

**Baseline atual:** `main` = `1e14d32d5871326cc7f904cef7c5ad4feb1cd86c`.

- PR #506 (merge SHA `54f881872ec9f5a9923ebdb17643b37a19954ec5`) integrou E7.2E; o estado histórico QUEUED da matriz foi supersedido. Os checks aplicáveis no merge SHA passaram: Rust quality/unit regression, Windows x64 D3D12 compile e Python lint/regression.
- PR #675 integrou `block_io_repeat_flags_match_hardware_banks_formula_across_b_loop`. O teste cobre B inicial de `0x10` a `0xD0` em passo 3 (65 valores), quatro posições PC que cobrem XY derivados de PC.13/PC.11 e seis casos INIR/INDR/OTIR/OTDR. O esperado é calculado pela álgebra publicada com o ROM Z80 Block Flags Test v5.0, incluindo L pós-atualização para OUTI/OUTD.
- O teste compara executor atômico e microsequenciador com a fórmula independente e verifica flags, B, PC, WZ, 21 T-states e transações de I/O. CI #42568885363 passou no HEAD exato `33d3dcf8d41f2f18d0826a1401013e9e1582503e`; merge SHA `1e14d32d5871326cc7f904cef7c5ad4feb1cd86c`.
- O teste reproduz a fórmula/domínio do ROM; não executa o ROM v5.0 nem constitui uma nova execução em hardware. A evidência física publicada continua limitada ao escopo NMOS e aos casos descritos na auditoria complementar.
- `OUT (C),0` permanece RESOLVED_FOR_DOCUMENTED_SMS_NMOS_PROFILE. CMOS/clones fora do levantamento e SMS físico permanecem NOT_VERIFIED/NOT_RUN.
- E7.7 permanece IN_PROGRESS por outras famílias e efeitos semânticos ainda não reconciliados. E8 permanece BLOCKED; não iniciar ZEXDOC/ZEXALL.


## 15. Reconciliação IM2/RETI — 2026-11-09

**Baseline atual:** `main` = `043ecb61d8d1cc77245fa72639d7f210d88cab26`.

- PR #677 integrou o cenário IM2 que assertiva INT no último T-state de `EdBlockIoRepeatInternal`, atende o vetor `I:FF`, executa `PUSH AF` e confirma F observável no handler.
- PR #678 estendeu o handler para `PUSH AF; POP AF; EI; RETI` e executou a iteração de bloco seguinte, confirmando retorno ao PC ED, restauração de SP, IFF1/IFF2, EI delay, B, PC e WZ.
- A cobertura contempla INIR, INDR, OTIR e OTDR e compara executor atômico e microsequenciador. Tempos verificados: 21 T da iteração repetitiva, 19 T da entrada IM2, 11 T PUSH AF, 10 T POP AF, 4 T EI, 14 T RETI e 21 T da iteração retomada.
- CI #42569668886 (PR #677) e #42569931640 (PR #678) passaram nos HEADs exatos; merge SHA atual `043ecb61d8d1cc77245fa72639d7f210d88cab26`.
- O teste respeita a semântica level-sensitive de `set_int_line`: o dispositivo desasserta INT após o serviço. A API one-shot `request_int` do executor atômico é limpa após o acknowledge para comparação.
- Isso conclui o fixture CPU-only de observação e retomada. O ROM v5.0 completo ainda não foi executado no core, e não houve validação física no Master System.
- E7.7 permanece IN_PROGRESS por outras famílias/efeitos e validações pendentes. E8 permanece BLOCKED.


## 16. Checklist mestre de auditoria integral — 2026-11-09

**Baseline de código reconciliada:** `main` = `b41fa66e01e329d45f7704a229479c8ac3c2c9ea`.

Foi criado o [Checklist mestre de auditoria e fechamento do Z80](z80-master-audit-checklist.md). Ele passa a ser o ledger operacional da auditoria: inventário de instruções e prefixos, estado arquitetural/flags, barramento/temporização, interrupções, snapshots/determinismo, referências, testes e estrutura do código. Cada fechamento deve ser registrado com IDs, evidências, testes classificados e SHA integrado na main.

As baselines antigas mantidas nas seções anteriores são fotografias históricas e não devem ser reescritas como se tivessem sido auditadas no SHA atual. A partir desta seção, novas atualizações devem referenciar o SHA efetivamente consultado.

**Estado:** auditoria global IN_PROGRESS; PR #680 permanece aberta e bloqueada por CI FAIL; E8/ZEXDOC/ZEXALL permanece BLOCKED.


## 17. Reabertura do inventário operacional — snapshot documental após PR #681

**Baseline da main consultada para esta varredura:** `db6b4ec6d2f01ddaabccc124fe9ca7d95a856320` (PR #681, checklist mestre, merge concluído).

- O checklist `docs/z80-master-audit-checklist.md` passa a ser o registro de cobertura e evidências por ID.
- A primeira varredura estática DD/FD confirmou que o executor atômico cobre famílias IXH/IXL/IYH/IYL que ainda não têm dispatch equivalente no microsequenciador: LD r,r' register-only e ALU A,IXH/IXL/IYH/IYL.
- A PR #680 permanece em andamento para INC/DEC e LD imediato das metades indexadas. A falha de snapshot foi corrigida no HEAD `48aad1f...`; CI #5924 passou nos gates focados. O HEAD atual da PR é `ed113041...`, com CI #5925 executando regressão integral; fmt, Clippy workspace e testes workspace já passaram, mas validações Windows ainda estão em execução.
- Não declarar F-028/F-036/F-037 fechados nem liberar E8. O fechamento exige merge e atualização do ledger no SHA resultante.

**Estado:** E7.7 IN_PROGRESS; E8 BLOCKED.


## 18. Reconciliação após PR #680/#683 e candidato F-036 — 2026-11-09

**Main remota confirmada:** `43772d5cd5395de273bd3898895cf1deed81aa53`.

### Integrações concluídas

- **PR #680:** squash merge confirmado em `04527a59e0695a800d4fd048d2da85c51126de68`. HEAD validado `e20ec4b6c858bcaca265114ba543c19301dbb2e0`; CI #42609912469 PASS. INC/DEC IXH/IXL/IYH/IYL e LD imediato das metades indexadas estão no microsequenciador. WAIT/snapshot dos safe points cobertos pela PR foram validados.
- **PR #683:** squash merge confirmado em `43772d5cd5395de273bd3898895cf1deed81aa53`. HEAD validado `eaa88a249989b2ca6aee931e2a963df75a396008`; CI #42613123290 PASS. A seleção por dependências, jobs focados Z80/PSG/VDP/input, validações de hosts e agregador foram integrados. A PR #637 foi encerrada como supersedida.

### Estado atual do fechamento E7

| Família | Estado | Evidência / pendência |
|---|---|---|
| F-028 — IXH/IXL/IYH/IYL | AUDIT_PARTIAL | INC/DEC e LD imediato #680; F-036 LD r,r' #684 CLOSED; F-037 ALU #686 CLOSED. Auditoria residual de precedência, exceções e efeitos temporais ainda pendente. |
| F-036 — LD r,r' register-only | CLOSED | PR #684 integrada em `0b9b298f0f9791a6fa6db17580ad741db5c0515d`; HEAD `03dbd1552c8eb58ff8f9ef28d9ab0e97b615b4f3`, run #42614755935 PASS. 48 combinações DD/FD, oracle, comparação atômica, 8T/2 M1, WAIT, snapshot/restore, prefixos repetidos e HALT. D3D12/áudio/SDL3/workspace SKIPPED e agregador PASS. |
| F-037 — ALU A,IXH/IXL/IYH/IYL | CLOSED | PR #686 merge SHA `e7e37290e1baa9119326e9e7213f77cadcc37f5e`; HEAD validado `a0937e8fa1bb4547d1ee53a9e3431392a2fa443a`; run #42619522899 PASS. 10.816 casos diferenciais, vetores fixos, carry-in, flags/X/Y/CP, Q, WAIT, snapshot/restore, prefixos repetidos. Corpus externo SingleStepTests ainda pendente como evidência complementar E7. |
| E7.7 — auditoria semântica integral | IN_PROGRESS | Outras famílias, efeitos observáveis, proveniência independente e auditoria temporal ainda pendentes. |
| E8 — ZEXDOC/ZEXALL | BLOCKED | Não iniciar antes do fechamento formal de E7. |

**Regra de promoção:** F-036 foi promovida a CLOSED somente após CI verde no HEAD final da PR #684 e merge confirmado. Os jobs D3D12/áudio/SDL3/workspace foram SKIPPED, classificados internamente como NOT RUN, e o GitHub aceitou o merge; comportamento comprovado em PR real.

**Próxima tarefa objetiva:** auditoria residual do Z80 com vetores externos, revisão de flags/Q/WZ, prefix FSM, interrupções e microtiming. Não avançar para E8.


## 19. Fechamento F-036 — evidência final

- PR #684: merged.
- HEAD validado: `03dbd1552c8eb58ff8f9ef28d9ab0e97b615b4f3`.
- CI final: #42614755935 — PASS no HEAD exato. Z80 focused, documentation hygiene e agregador PASS; jobs não aplicáveis SKIPPED.
- Falhas intermediárias registradas: #42614347670 (rustfmt) e #42614617507 (Clippy field_reassign_with_default). Corrigidas antes do HEAD final.
- Merge SHA e main atual: `0b9b298f0f9791a6fa6db17580ad741db5c0515d`.
- F-036 CLOSED; F-037 GAP_CONFIRMED; F-028 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.

**Próxima tarefa:** F-037 — ALU A,IXH/IXL/IYH/IYL — com vetores independentes para ADD/ADC/SUB/SBC/AND/XOR/OR/CP, carry de entrada, flags completas, Q, caminho retomável, WAIT e snapshot/restore.


## 20. Candidato F-037 — ALU com metades IX/IY — 2026-11-09

**Main remota de entrada:** `5aaa74af3fbf6fa7f70ecb1e6b71f3a966bb91e4`.

- **PR:** #686 — `feat/z80-e7-indexed-half-alu-20261109`.
- **Commit de implementação:** `5f9ca825feb45633c2baf200e502f1a05bf99c7d`.
- **Último commit de teste/otimização antes desta atualização documental:** `2ef8d4c0c73cf5a339f8d258ba9e314bfeebcdd2`. O HEAD final da PR será consultado no GitHub antes do merge.
- **Estado:** IMPLEMENTED_PENDING_VALIDATION. O microsequenciador agora tem allowlist e dispatch para as 16 combinações DD/FD de ADD, ADC, SUB, SBC, AND, XOR, OR e CP com IXH/IXL/IYH/IYL.
- A implementação reutiliza `read_indexed_register()` e `execute_alu_operation()`; não duplica a ALU nem cria ciclos internos artificiais.
- A suíte `indexed_half_register_alu_e7.rs` contém 10.816 casos diferenciais (8 operações, 2 fontes, DD/FD, 13 acumuladores, 13 operandos e 2 carry-in), 16 vetores fixos por prefixo/fonte, WAIT nos dois M1, snapshot/restore, prefixos repetidos e verificações de A/F/Q/previous-Q/WZ/PC/R/IFF/I/IX/IY/H/L.
- CP mantém A e valida X/Y derivados do operando. As formas com fonte código 6 continuam no caminho E4B de memória indexada.
- F-036 permanece CLOSED. F-037 aguarda CI/merge. F-028 permanece AUDIT_PARTIAL. E7 IN_PROGRESS; E8 BLOCKED.

**Próximo gate:** validar o HEAD final da PR #686 com a suíte focada Z80 e os consumidores SMS/SG-1000; comprovar novamente que hosts não relacionados são SKIPPED. Só depois integrar F-037 e prosseguir à auditoria mestre residual.


## 21. Fechamento F-037 — evidência final

- PR #686 squash-merged em `e7e37290e1baa9119326e9e7213f77cadcc37f5e`.
- HEAD validado: `a0937e8fa1bb4547d1ee53a9e3431392a2fa443a`; run #42619522899 PASS.
- Rust fmt focado, Clippy focado, testes Z80 + SMS + SG-1000, documentation hygiene e agregador: PASS.
- D3D12, WASAPI/XAudio2, Linux audio, SDL3, VDP, PSG, Scheduler stress, PowerShell, input e workspace: SKIPPED / NOT RUN por não aplicabilidade.
- Run intermediário #42615731297 FAIL em rustfmt foi corrigido antes do HEAD final.
- F-036 CLOSED; F-037 CLOSED; F-028 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.
- Pendência de evidência: integrar vetores externos (SingleStepTests Z80, variante NMOS e licença MIT) à suíte diferencial, sem tratar o executor atômico como oracle independente.


## 22. Validação externa SingleStepTests F-037 — 2026-11-09

**Main após integração:** `1304bc0438e3ea333719d0ba3a495e2db9089da3`
**PR:** #692 — merge squash
**HEAD da PR validado:** `1606556814a8746a445d3ad32fc948ab97f0fde5`
**CI:** run `4262282817` — PASS nos checks aplicáveis
**Upstream:** SingleStepTests/z80, revisão `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`, licença MIT.

- 32 fixtures: DD/FD × 16 segundos bytes.
- 1.000 vetores por fixture; **32.000/32.000 PASS**, sem amostragem.
- Executados no microsequenciador retomável em `native/z80_cpu/tests/singlestep_indexed_alu.rs`.
- Comparados registradores principais/alternativos, F, Q/P, PC, R, WZ, IFF, IM, EI, IX/IY e RAM.
- Duas requisições lógicas OpcodeFetch e 8 T-states agregados conferidos.
- Os traces de barramento não foram tratados como equivalência elétrica de pinos.

**Limite de proveniência:** o README upstream informa que os vetores foram traduzidos do núcleo Z80 do Ares e corrigidos. O corpus é externo ao repositório, mas não é independente do Ares nem constitui prova direta de silício.

**Estado após integração:** F-036 CLOSED; F-037 CLOSED com validação externa complementar; F-028 AUDIT_PARTIAL; F-033 CLOSED para o perfil NMOS clássico delimitado após PR #706; F-034 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.

## 23. F-028 — reprodução da divergência Q/previous-Q — 2026-11-09

- Reprodução: PR #695, workflow run #42624833386.
- Fixture DD 37 0001: esperado F=0x65; obtido F=0x45. **FAIL reproduzido** no microsequenciador retomável.
- A execução interrompeu no primeiro mismatch; FD 3F está presente no corpus, mas não foi alcançado.
- O problema decorre da preservação de q_previous através de DD/FD antes de SCF/CCF no perfil Zilog NMOS.
- PR de correção Q/previous-Q limpa q_previous nos caminhos atômico e retomável, atualiza o teste interno e valida snapshot/restore após o prefixo. CI ainda pendente nesta baseline documental.
- F-028 permanece AUDIT_PARTIAL até CI PASS e auditoria das demais exceções/variantes. E7 IN_PROGRESS; E8 BLOCKED.


## 24. F-028 — fechamento da divergência Q/previous-Q em DD/FD — 2026-11-09

**Main após merge:** `401bf4fb1ba8add6c84420cad5c9d2a62ceba72b`
**PR:** #702 — MERGED (squash)
**HEAD exato validado:** `34e85637eaac8ed2777ecc5cb9cd2e91ba137235`
**Workflow runs:** #42626811588 e #42626765522 — CI SUCCESS.

- Executor atômico: limpa `q_previous` ao entrar em `execute_indexed`.
- Microsequenciador: limpa `q_previous` ao transitar/continuar no estado Index; cobre prefixos DD/FD únicos, repetidos e alternados.
- Testes F-036/F-037 foram reconciliados para a semântica de prefixo; E4A/E4B não foram alterados.
- Teste diferencial interno DD 37 / FD 3F verifica F, Q/previous-Q, 8T e snapshot/restore entre prefixo e opcode terminal.
- SingleStepTests/Z80: 32.000 vetores F-037 + 2.000 vetores DD 37/FD 3F = **34.000/34.000 PASS**, executados no microsequenciador retomável.
- O log do job Z80 focused validation confirma os dois testes externos executados e aprovados; o harness compara estado arquitetural e T-states agregados, não equivalência elétrica de pinos.
- rustfmt, Clippy, testes Z80, integração SMS/SG-1000, workspace regression e documentação: PASS nos runs registrados.

**Classificação:** a divergência específica Q/previous-Q de DD/FD→SCF/CCF está RESOLVED. F-028 permanece AUDIT_PARTIAL porque a auditoria integral de todas as exceções DD/FD, prefixos, HALT, flags e efeitos arquiteturais ainda não foi concluída. F-036 CLOSED; F-037 CLOSED; F-033 GAP_CONFIRMED; F-034 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.

**Pendências E7 que não são resolvidas por esta PR:** microsequenciador para os 178 bytes ED reservados (F-033), classificação/validação dos aliases e encodings não documentados (F-034), auditoria global das famílias F-001..F-034, interrupções/WAIT/barramento e determinismo sistêmico.


## 25. Candidata F-033 — aposentadoria ED reservada no microsequenciador — 2026-11-09

**Main de entrada:** `9d5d83d0df16a1c6fa9fb82bdd953af54369c613`
**PR:** #706 — `fix/z80-f033-ed-reserved-resume-20261109`
**HEAD candidato:** `cab1603e9699e44b5f2edd418186f63410cad65a`
**Estado histórico:** IMPLEMENTED_PENDING_VALIDATION na abertura da PR; a implementação foi posteriormente integrada.

- O fallback residual de `execute_ed_register_control()` agora aposenta a instrução após os dois fetches M1 já consumidos, sem ciclos internos artificiais nem transações extras de memória/I/O.
- A suíte `native/z80_cpu/tests/ed_reserved_f033.rs` percorre os 178 bytes fora das 78 codificações explícitas e verifica 8 T-states, PC com wrap, dois refresh increments, F/Q/previous-Q/WZ/P/IFF/IM, fase Ready e ausência de request pendente.
- A suíte também cobre WAIT aditivo e snapshot/restore antes e depois do byte terminal ED.
- Regressões antigas no sequenciador e no SMS temporal foram atualizadas de `UnsupportedOpcode` para aposentadoria determinística.
- A hipótese de 8T/no-effect é delimitada ao perfil Zilog NMOS clássico. UM0080 não especifica universalmente todos os bytes reservados; MAME e fontes secundárias são evidência de implementação/referência, não prova de silício. CMOS, R800, Z80N e clones permanecem fora da afirmação.
- CI #42631919433 no HEAD candidato: Z80 focused PASS; Rust quality and unit regression PASS; Windows x64 D3D12 PASS. Rust workspace regression, documentação e outros jobs aparecem SKIPPED/NOT RUN por não aplicabilidade; a política de CI determina pacote focado para mudanças exclusivamente Z80. Nenhum resultado de outro SHA é transferido.
- A revisão automática Codex da PR não concluiu por limite de uso; não há review threads inline abertas nem reviews submetidas na consulta. Isso não equivale a aprovação de revisão.

**Classificação após merge:** F-033 CLOSED exclusivamente para o perfil de compatibilidade Zilog NMOS clássico descrito nesta seção. F-034 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED.


## 26. Reconciliação pós-merge da PR #706 — 2026-11-09

**Main:** `2d15a2311cda9d087bbdd6c05f6c3851d52f4150`
**PR integrada:** #706, HEAD validado `b2291a83848e236d2354d8f03e22b831f9030b72`.
**CI no HEAD exato:** run #42635309051 SUCCESS. Z80 focused validation, Rust quality/unit regression e Windows x64 D3D12 PASS. Jobs não aplicáveis foram SKIPPED/NOT RUN pela política de impacto; não são classificados como PASS.

- O microsequenciador aposenta os 178 bytes ED reservados após os dois M1 já consumidos, sem ciclos internos ou transações extras.
- `native/z80_cpu/tests/ed_reserved_f033.rs` cobre todos os 178 bytes, estado arquitetural discriminante, WAIT e snapshot/restore.
- A implementação é uma política de compatibilidade para Zilog NMOS clássico. UM0080 não especifica universalmente esses encodings; MAME e referências secundárias não são prova de silício. CMOS, R800, Z80N e clones permanecem fora da afirmação.
- O fallback do executor atômico permanece distinto e não é usado como substituto da evidência temporal do microsequenciador.

**Classificação atualizada:** F-033 CLOSED com limite de evidência; F-034 AUDIT_PARTIAL; E7 IN_PROGRESS; E8 BLOCKED. E8 não pode começar antes do fechamento formal de E7.
