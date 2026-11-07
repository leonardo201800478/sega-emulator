# Z80 compartilhado — scorecard de conclusão

**Baseline documental atual:** `main` = `3ad1781f84b7824b5aa889a583ae6d4545987ec1` (2026-11-07).  
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
- O fechamento semântico global E7.7 ainda não foi comprovado como concluído. A auditoria complementar de I/O em bloco caracteriza as flags para o perfil Zilog NMOS e não encontrou divergência de runtime; CMOS/clones, validação direta em hardware SMS e `OUT (C),0` variante-dependente permanecem sem comprovação conclusiva. A auditoria semântica integral continua aberta.
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
- CMOS/clones, validação direta em hardware SMS e `OUT (C),0` variante-dependente permanecem sem comprovação conclusiva.
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
- **E7.7 permanece IN_PROGRESS.** A auditoria semântica global não está encerrada; continuam pendentes comprovação para variantes CMOS/clones, validação direta em hardware SMS e decisão variante-dependente para `OUT (C),0`.
- **E8 permanece BLOCKED.** Não iniciar ZEXDOC/ZEXALL antes do fechamento documentado de E7.
- A integração de aplicação/runtime SMS não comprova por si só a precisão temporal sistêmica do Z80. Não declarar TEMPORAL_PASS global nem equivalência contínua/em blocos sem testes específicos.

Nenhuma alteração de código Z80 decorre desta reconciliação documental.
