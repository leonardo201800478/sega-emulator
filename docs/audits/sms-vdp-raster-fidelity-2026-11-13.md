# Auditoria de fidelidade raster do SMS VDP — 2026-11-13

**Branch:** `audit/sms-vdp-raster-fidelity-20261113`  
**Base auditada:** `main` em `45684820d57bf5a80a0ff4a33df7a7990c566f5d`  
**Engine ativo:** `native/video_core/src/sms/vdp.rs`  
**Integração temporal:** `SmsTemporalMachine -> SmsTemporalBus -> SmsVdp`  
**Estado:** auditoria estática inicial; nenhuma correção de runtime declarada ou validada nesta revisão.

## 1. Sintomas de usuário que motivam a auditoria

- O V Counter Test apresenta sequências/valores diferentes do Ares, incluindo diferença observada no campo VBL.
- O Scroll Test foi reportado como malsucedido: os sprites aparecem, mas seu movimento/posicionamento diverge fortemente do Ares.
- A execução do host está nominalmente estável (~60 uploads/s nas capturas anteriores), mas upload/s não comprova Present/scanout físico. O problema desta auditoria é a semântica e temporização interna do VDP, não o pacing do host.

Esses sintomas são observações comparativas, ainda não são um diagnóstico causal completo.

## 2. Restrições arquiteturais

Esta auditoria segue ADR-0016:
- Priorizar precisão temporal quando uma interação CPU/VDP/raster é observável por software.
- Manter operações restantes funcionais e atômicas quando não houver efeito intermediário observável.
- Escolher a menor granularidade suficiente; não implementar pipeline por pixel/dot por completude teórica.
- O MachineScheduler continua sendo a autoridade temporal.
- Não introduzir callbacks por instrução na GUI, um segundo scheduler, temporização paralela ou hacks de frame pacing.
- Preservar Z80, runtime SMS/SG-1000, áudio e integração host já validados.
- Não alterar o escopo Z80/E7/E8; ZEXALL continua bloqueado até fechamento demonstrável de E7.

## 3. Achados confirmados no código do baseline

### VDP ativo e fronteiras temporais
O engine compartilhado está em `native/video_core/src/sms/vdp.rs`; `native/sms_core/src/vdp.rs` é apenas reexportação. A máquina temporal ativa está em `native/sms_core/src/temporal_bus.rs`.

`SmsTemporalBus::dispatch_next()` avança o domínio VDP do Scheduler pelo número de T-states do boundary despachado antes de aplicar o efeito da operação de barramento. Portanto, o contrato temporal de chegada das escritas de porta já existe no caminho temporal. A auditoria não deve substituir esse contrato por sincronização paralela.

### Renderização e memória
`SmsVdp::advance()` acumula T-states até 228 por scanline e chama `complete_scanline()`. No fechamento, `complete_scanline()` entrega ao renderer:
- snapshot corrente dos registradores (com R8/R9 substituídos pelos latches);
- VRAM corrente;
- CRAM corrente;
- linha completa.

`write_data()` modifica VRAM/CRAM imediatamente. O teste existente `midline_cram_write_is_sampled_for_the_whole_scanline_by_current_model` caracteriza explicitamente que uma escrita no meio da linha pode afetar a linha inteira, inclusive pixels logicamente anteriores à escrita. Esse teste é uma caracterização do modelo atual, não um oráculo de hardware.

Consequência: o timestamp do write chega ao VDP, mas o renderer não preserva uma fronteira horizontal de visibilidade para VRAM/CRAM nem reconstrói o trecho de linha já emitido. A mesma limitação afeta dados de pattern/name/SAT quando escritos antes do fechamento da linha.

### Sprites Mode 4
O renderer atual faz a avaliação completa da SAT dentro de `render_scanline_for_revision()`, no momento em que a linha é renderizada:
- percorre até 64 entradas Y;
- usa terminador D0 no modo 192 linhas;
- resolve X/tile em arrays separados a partir do offset 0x80;
- aplica deslocamento de sprite R0;
- resolve 8x8/8x16 e zoom;
- aplica limite de oito sprites, overflow, colisão e prioridade;
- desenha sprites no mesmo passe que compõe a linha.

Não há estado explícito de line buffer de sprites previamente avaliado, nem fase de avaliação SAT separada da fase de composição da linha. Portanto, mutações de SAT feitas ao longo da linha são observadas pelo renderer como estado final no fechamento da linha, não como o conjunto de sprites já selecionado pelo hardware em uma fase anterior.

A documentação de caracterização T0 já registra que a SAT é avaliada durante uma linha para selecionar sprites da próxima linha. O código atual não representa essa separação como estado persistente. A comparação com MAME também mostra seleção de sprites em rotina separada (`select_sprites`) e desenho posterior; isso é referência de implementação, não prova física isolada.

### Scroll
R8 possui latch em HPos 21 (T14 no modelo 342 HPos / 228 T-states), baseado em SmsVdpTest e comparação com MAME. Escritas no limite e após o limite têm testes unitários.
R9 é mantido como `pending_scroll_y` durante active display e aplicado na transição da área visível.

Foi identificado também um erro funcional específico na região de fine-scroll: o renderer preenchia os pixels iniciais com o backdrop de R7. A discussão do Scroll Test com Charles MacDonald esclarece que os pixels iniciais não usam o valor de R7; a implementação MAME descreve um comportamento mais específico: a cor 0 das duas paletas de background é selecionada pelo bit 1 do plano 1 do tile `$100`, pixel 4, na linha anterior. O código foi ajustado para esse seletor e o teste foi substituído por um vetor que cobre as duas paletas. Isso corrige um comportamento demonstravelmente incorreto, mas não comprova ainda a causa de qualquer divergência de sprites relatada pelo usuário.

Esses testes verificam boundaries isolados, mas não cobrem uma ROM completa com alterações sequenciais de R8/R9, HINT, SAT e VRAM durante o raster.

### V Counter / VBlank / HINT
O V Counter usa fórmulas baseadas em scanline/região/modo. VBlank e line IRQ são atualizados em `complete_scanline()`. Não existe neste engine uma fase horizontal explícita para os efeitos de status/IRQ. Os limites precisam de vetores independentes e não devem ser corrigidos por offsets visuais.

## 4. Matriz inicial de risco

| Área | Estado do baseline | Risco / lacuna | Próxima evidência exigida |
|---|---|---|---|
| SAT evaluation | avaliação na renderização da linha | estado SAT tardio/retroativo; falta buffer de sprites por linha | ROM que reescreve SAT em raster + trace de write |
| Sprite pattern fetch | pattern consultado no fechamento da linha | write tardio pode alterar pixels da linha inteira | teste de alteração de pattern no meio da linha |
| Sprite X/Y/tile | endereços funcionais e composição por linha | fase de amostragem de atributos não representada | comparação por scanline com Ares/MAME e teste dedicado |
| Overflow | calculado durante renderização | momento de flag/status pode divergir | leitura de status antes/depois do boundary |
| Collision | calculada por sobreposição na composição | instante de latch/clear pode divergir | ROM sintética com leitura de status em timestamps controlados |
| R8 | latch HPos 21 implementado | regra isolada não cobre sequência raster | Scroll Test e testes de fronteira |
| R9 | latch diferido para fim da área ativa | fase de aplicação ainda aproximada | VScroll test e trace de writes |
| VRAM/CRAM | mutação imediata, render no fim da linha | sem visibilidade horizontal de write | teste de write em posições diferentes da mesma linha |
| V Counter | cálculo por scanline | fase e limites possivelmente incorretos | V Counter Test, vetor documentado e hardware quando disponível |
| VBlank/HINT | evento por fechamento de linha | fase sublinha/status/IRQ não modelada | ROM de IRQ com timestamp e observação CPU |
| NTSC/PAL e ASIC | perfis existentes | regras não demonstradas para todas as revisões | matriz por 315-5124/315-5246 e região |

## 5. Referências comparativas e limites

- SMS Power — Sprites: https://www.smspower.org/Development/Sprites
- SMS Power — VDP Registers: https://www.smspower.org/Development/VDPRegisters
- SMS Power — SMS Official Docs: https://www.smspower.org/Development/SMSOfficialDocs
- SmsVdpTest (FluBBaOfWard): https://github.com/FluBBaOfWard/SmsVdpTest
- Scroll Test Program / discussão técnica: https://www.smspower.org/Homebrew/ScrollTestProgram-SMS e https://www.smspower.org/forums/2221-CharlesMacDonaldsScrollTest
- MAME 315-5124: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- Ares VDP SMS: https://github.com/ares-emulator/ares/tree/master/ares/ms/vdp

MAME/Ares/Gearsystem são implementações independentes úteis para localizar divergências, mas nenhuma delas isoladamente é um oráculo de hardware. Comentários e constantes não verificados devem continuar classificados como hipóteses. Os testes do projeto devem separar comportamento documentado, modelo comparativo, ROM de diagnóstico e medição física.

## 6. Plano de correção incremental

### Fase A — reproduções e traces (sem mudar runtime)
1. Fixar ROM/hash, região, perfil ASIC, resolução, frame inicial e sequência de entrada.
2. Capturar lossless frames e comparar Ares versus nosso VDP na mesma fase.
3. Instrumentar writes CPU->VDP com timestamp do Scheduler, scanline/T-state/HPos, porta, endereço, valor e estado do command latch.
4. Instrumentar eventos de avaliação de sprites, overflow/collision, R8/R9, VBlank e HINT.
5. Criar ROMs sintéticas pequenas para isolar: SAT Y/X/tile, terminador D0, 8/9 sprites, 8x16, zoom SMS1, sprite shift, colisão, alteração de SAT/pattern no meio da linha e atualização entre linhas.

### Fase B — separar avaliação e composição de sprites
1. Modelar um buffer de seleção/atributos de sprites para a linha alvo, capturado no boundary de avaliação demonstrado.
2. Separar avaliação SAT, fetch de pattern, overflow e composição visual.
3. Preservar regras específicas de 315-5124 e 315-5246 via profile, sem compartilhar suposições entre revisões.
4. Garantir que writes tardios não alterem retroativamente a seleção já latched.
5. Determinar por testes o timestamp de atualização de status overflow/collision; não atualizar flags somente porque o renderer terminou a linha, se o hardware as disponibiliza antes.

### Fase C — scroll e fetch de background
1. Validar R8 em T14/HPos 21 e sequência de writes por linha.
2. Validar R9 e scroll lock nas colunas 24–31.
3. Introduzir fronteiras de composição parcial apenas para eventos observáveis demonstrados.
4. Validar alterações de name table/pattern/CRAM na posição em que passam a ser visíveis.

### Fase D — contadores e interrupções
1. V Counter NTSC/PAL e wrap conforme perfil.
2. VBlank: linha e timestamp de status/IRQ.
3. HINT/R10: reload, underflow, acknowledge e relação com boundary de barramento.
4. Verificar instruções Z80 que cruzam a fronteira e a ordem causal no Scheduler.

### Fase E — regressão e aceitação
- Unit tests funcionais e de transição.
- Property tests de chunking invariance e snapshots.
- Raster trace tests com oráculo independente.
- Differential tests Ares/MAME sem tratar consenso como prova física.
- Reexecutar V Counter, Scroll, Sprite/Priority e SMSTestSuite.
- Repetir regressões SMS/SG-1000, áudio, frame pacing, save/restore e build desktop.
- Sem regressão de determinismo, frame pacing ou semântica do Scheduler.

## 7. Critérios de aceite

A correção só será considerada concluída quando:
- o conjunto de sprites exibido por linha corresponder à avaliação temporal esperada;
- alterações de SAT/pattern não retroagirem para períodos já avaliados;
- posição e movimento dos sprites coincidirem com o oráculo para os vetores registrados;
- scroll R8/R9 e V Counter/VBlank passarem os testes direcionados;
- flags overflow/collision e IRQ forem observáveis no boundary correto;
- execução contínua e em blocos, snapshot/restore e integração Scheduler permanecerem determinísticos;
- CI e testes direcionados passarem no commit candidato.

**Não declarar dot-accurate/cycle-accurate globalmente.** O alvo é a menor granularidade temporal que reproduz fielmente os comportamentos observáveis do SMS1/315-5124, com SMS2/315-5246 validado separadamente.

## 8. Estado desta auditoria

- Achados estáticos: documentados.
- Correção funcional inicial: seletor de paleta dos pixels iniciais de fine-scroll corrigido; testes unitários atualizados.
- Correção estrutural de avaliação temporal de sprites: ainda não aplicada.
- ROMs de usuário executadas nesta auditoria: nenhuma; as capturas comparativas foram fornecidas pelo usuário em conversa separada.
- VDP temporal PASS: não alcançado.
