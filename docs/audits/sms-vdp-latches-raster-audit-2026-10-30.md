# Auditoria de latches e raster do VDP SMS — 2026-10-30

## Estado e escopo

- Base remota: `main` SHA `756666ae6216d8466f40259c11c345bbc1921848`.
- Branch: `audit/sms-vdp-latches-raster-2026-10-30`.
- Fonte principal: `native/sms_core/src/vdp.rs` blob `7968aa6a1bbc2f5a56126f618a11ec76c18e1e2c`.
- Documento arquitetural: `docs/architecture/sms-vdp.md` blob `43992ec06d95c0afbfe7e2b3e2101f008e25e93b`.
- Escopo: registradores R0–R10 relevantes ao raster, latch vertical R9, scroll horizontal R8, atualização de VRAM/CRAM, fechamento de scanline, IRQ de linha e limites das evidências.
- Nenhuma alteração de runtime nesta etapa.

## 1. Resumo executivo

O VDP SMS atual é um modelo determinístico por scanline, não dot/T-state exact. `SmsVdp::advance()` acumula T-states até a fronteira de 228 T-states por linha; `complete_scanline()` então renderiza a linha usando cópias dos registradores e os arrays VRAM/CRAM correntes.

O código possui um tratamento explícito de latch para R9: durante a área ativa, a escrita é guardada em `pending_scroll_y` e aplicada ao fim da área visível. Não há tratamento temporal equivalente para R8. R8 é lido pelo renderer a partir do valor corrente dos registradores durante a renderização da linha.

A consequência arquitetural relevante é confirmada: uma escrita CPU/VDP pode ocorrer entre dois avanços do VDP, mas o código não carrega timestamp horizontal da escrita. Se o avanço posterior completar uma linha, o renderer usa o estado atual naquele instante de fechamento. Portanto, o modelo não consegue representar com generalidade a posição horizontal exata em que uma alteração deveria tornar-se visível. Isso é uma limitação estrutural; ainda não prova a causa de um caso específico de parallax ou raster.

## 2. Evidências no código atual

### 2.1 Avanço e renderização

Em `SmsVdp::advance(tstates)`:
- o tempo é consumido até a próxima fronteira de linha;
- cada linha usa `LINE_TSTATES = 228`;
- ao completar a linha, chama `complete_scanline()`.

Em `complete_scanline()`:
- a linha atual é renderizada se estiver dentro da altura visível;
- cria-se `render_registers = self.registers`;
- R9 é substituído por `latched_scroll_y`;
- renderer recebe referências ao VRAM e CRAM atuais;
- após renderizar a linha, atualiza contador/IRQ de linha;
- ao chegar à altura visível, aplica `pending_scroll_y`, sinaliza VBlank e publica o framebuffer.

Isto comprova que a renderização é feita no fechamento da linha e que não há reconstrução de pixels anteriores baseada em timestamps de escritas.

### 2.2 R9 — latch vertical implementado

Em `write_control()`, quando o comando escreve um registrador:
- grava o valor em `registers[9]`;
- se `scanline < visible_height()`, guarda o valor em `pending_scroll_y`;
- caso contrário, atualiza `latched_scroll_y` imediatamente e limpa o pendente.

Em `complete_scanline()`, ao cruzar a fronteira da altura visível:
- aplica o último `pending_scroll_y`, se houver;
- em seguida inicia o estado de VBlank.

O comportamento corresponde, em alto nível, à descrição de R9 como latch em VBlank encontrada na documentação técnica pública SMS Power e no arquivo comunitário de referência SMS VDP. A regra específica do código é mais precisamente “aplicar ao terminar a área ativa”, não um timestamp elétrico de latch demonstrado.

### 2.3 R8 — ausência de latch temporal explícito

- `write_control()` atualiza `registers[8]` no momento funcional do write.
- Não existe `pending_scroll_x`, cópia por linha, ou timestamp horizontal associado ao write.
- `complete_scanline()` copia o conjunto corrente de registradores.
- `render_scanline()` usa R8 para coordenadas de origem do background.

Os testes atuais cobrem cálculo de scroll subtrativo, fine scroll, pixel inicial/backdrop, bloqueio de scroll nas primeiras 16 linhas e lock vertical a partir da coluna 24. Esses testes exercitam a função de renderização com registradores já configurados; não verificam uma escrita R8 durante uma linha em andamento nem sua visibilidade no pixel/coluna correto.

### 2.4 VRAM e CRAM

- `write_data()` altera VRAM ou CRAM imediatamente.
- Não há fila de transações do VDP, timestamp por write, buffer de linha parcial ou avanço do renderer até o instante da escrita.
- A renderização consulta VRAM/CRAM correntes ao completar a linha.

Assim, os testes de acesso e conteúdo de VRAM/CRAM demonstram a semântica funcional dos ports, não a visibilidade temporal intralinha. Não se pode afirmar a partir deles que um write em CRAM altera pixels da linha atual a partir de um ponto horizontal específico.

### 2.5 IRQ de linha

- O contador é avaliado no fechamento da linha visível, com reload de R10.
- A pendência de linha é atualizada no mesmo evento de scanline.
- `interrupt_pending()` combina a pendência de linha com enable de R0 e VBlank com enable de R1.
- A integração da CPU só observa a linha INT nos limites de instrução, conforme a auditoria CPU/barramento anterior.

Não há borda subinstrução nem fase horizontal específica modelada para o HINT neste VDP.

## 3. Matriz de registradores e efeitos temporais

| Elemento | Código atual | Evidência externa consultada | Lacuna/risco |
|---|---|---|---|
| R0 — scroll inhibit / modos / IRQ | Valor corrente copiado ao renderizar a linha | Manual de software Mark III descreve bits de scroll inhibit e modos | Sem momento de latch horizontal documentado no runtime |
| R1 — display / VBlank IRQ / altura | Valor corrente controla altura, display e IRQ | Manual Mark III descreve display e interrupt enables; referência técnica SMS descreve modos 192/224/240 | Mudança no meio da linha não é posicionada horizontalmente |
| R2–R7 — bases, sprite e backdrop | Consultados pelo renderer por linha | Documentação SMS descreve registradores e organização de VRAM | Sem snapshot por fetch/coluna ou evento intralinha |
| R8 — scroll horizontal | Atualizado imediatamente em `registers[8]`; usado na renderização | Manual Mark III define valor de scroll horizontal; MAME tem cópia `m_reg8copy` e condição baseada em H-position | Falta latch/cópia temporal equivalente; regra exata depende de revisão e evidência |
| R9 — scroll vertical | `pending_scroll_y` durante área ativa; aplicado na transição para VBlank | Manual Mark III diz que R9 é latched em VBlank; documento SMS detalha que alterações durante active display ficam temporárias | Sem prova elétrica do instante exato dentro do blanking |
| R10 — line counter | Escreve valor e recarrega contador funcional | Manual Mark III descreve raster line interrupt | Fase de reload/IRQ é somente por scanline, sem H-position |
| VRAM data | Escrita imediata | Port map documentado; emuladores públicos implementam variações de fetch/contenda | Sem contenção, fetch timing ou visibilidade parcial |
| CRAM data | Escrita imediata de 6 bits | Manual Mark III descreve formato de cores; documentação técnica SMS descreve palette | Sem timestamp e sem DAC por pixel |
| H-counter | Aproximação por `tstates_into_line`; latch sintético para Light Phaser | MAME e documentos técnicos modelam fases H-counter específicas | Conversão linear aproximada não prova equivalência ao contador real |
| V-counter | Fórmulas por região/modo | Documentação técnica SMS e emuladores fornecem tabelas/variações | Revisions e limites precisam de vetores independentes |

## 4. Fontes externas e peso da evidência

### 4.1 Documentação oficial de software Sega Mark III, publicada em SMS Power

URL: https://www.smspower.org/Development/SMSOfficialDocs&num=5

O documento se apresenta como *Software Reference Manual for the SEGA Mark III Console*. Na seção VDP:
- recomenda atualizar o VDP durante blanking para evitar ruído visual;
- descreve R0/R1, incluindo scroll inhibit e display/interrupt;
- define R8 como scroll horizontal;
- declara que R9 é latched em vertical blanking;
- descreve R10 como contador de raster line interrupt e explica o uso de HBLANK.

**Peso:** documentação de software da plataforma, fonte primária reproduzida publicamente. Descreve contrato observável/programável, mas não fornece todos os detalhes de latch elétrico em T-state.

### 4.2 Documento técnico comunitário SMS VDP

URL: https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt

O texto detalha:
- R8 como scroll horizontal por pixels e os efeitos das primeiras 8 pixels;
- R9 como starting row/fine scroll;
- atualização de R9 somente após o período de active display;
- wrap de vertical scroll conforme modo 192/224/240;
- lock de scroll em colunas 24–31.

**Peso:** referência técnica secundária; útil para hipóteses e vetores, mas não é manual oficial nem evidência direta de hardware. Alguns trechos explicitam incertezas.

### 4.3 MAME — modelo de referência comparativo, não especificação de hardware

URL: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp

A revisão consultada mostra:
- temporizadores de linha, bordas, interrupções e flags;
- `m_reg8copy` como cópia de R8;
- atualização de `m_reg8copy` em caso de escrita R8 somente se `screen_hpos()` estiver antes de `m_line_timing[XSCROLL_HPOS]`;
- valores de posição e comentários explicitamente marcados como “not verified” para algumas fases.

**Peso:** implementação independente útil para comparar modelagem e localizar hipóteses de fase horizontal. Os próprios comentários do código marcam valores não verificados; não usar números ou condições do MAME como verdade de hardware sem validação adicional.

### 4.4 Ares, GearSystem e MEKA

A investigação anterior da PR #261 já registrou diferenças arquiteturais:
- Ares interleia trabalho do background/sprites/DAC com avanço horizontal e sincronização da CPU;
- GearSystem e MEKA usam caminhos de renderização por linha com seus próprios latches/ordenação;
- tais diferenças não demonstram, isoladamente, pixel-perfect nem identificam a causa de uma imagem específica.

A investigação anterior foi mantida como evidência de contexto, não duplicada nem declarada concluída.

## 5. Confronto com os testes atuais

Testes unitários inline em `native/sms_core/src/vdp.rs` incluem:
- ownership/tamanho VRAM e CRAM;
- acesso a ports, read-ahead, wrap de endereço e escrita CRAM;
- status e VBlank IRQ;
- reload do contador R10;
- scroll horizontal subtrativo e fine scroll;
- pixels iniciais preenchidos por backdrop;
- scroll horizontal fixo nas primeiras 16 linhas;
- limites de wrap vertical 223/224/255;
- lock vertical a partir da coluna 24;
- teste de R9 aplicado somente após completar a área visível e iniciar o próximo frame.

O teste R9 existente é importante: escreve R9 durante active display, termina a área visível, verifica o frame atual e depois o próximo frame para confirmar o latch. Entretanto, não existe equivalente para:
- R8 escrito em posições horizontais diferentes da mesma linha;
- R0/R1/R2–R7 alterados no meio de linha;
- writes de VRAM/CRAM com comparação de pixels antes/depois do ponto de visibilidade;
- HINT com timestamp horizontal e borda INT;
- instrução Z80 que cruza uma fronteira de linha enquanto executa OUT para o VDP.

## 6. Conclusões confirmadas versus hipóteses

### Confirmado pelo código
1. Renderização por scanline no fechamento da linha.
2. R9 possui estado pendente e é aplicado na transição para VBlank.
3. R8 não possui latch horizontal explícito no runtime.
4. VRAM/CRAM e demais registradores são atualizados funcionalmente sem timestamp.
5. IRQ de linha é atualizada no fechamento da linha; não há borda subinstrução.
6. Testes cobrem a semântica de scroll estático e latch de R9, não efeitos intralinha genéricos.

### Sustentado por documentação
1. R9 é latched em VBlank/ao fim da área ativa, em termos de software.
2. R8 é scroll horizontal e o VDP tem regras especiais de scroll nas bordas/colunas.
3. Software deve preferir atualizar registradores durante blanking para evitar artefatos.

### Hipóteses que ainda não podem ser promovidas a fatos
1. O valor de H-position do MAME para latch de R8 é idêntico em todas as revisões SMS VDP.
2. Todo write R8 deve afetar a linha corrente ou a seguinte conforme uma única regra universal.
3. O desalinhamento/parallax de Aladdin decorre exclusivamente do R8 ou de falta de pixel stepping.
4. Writes CRAM/VRAM devem aparecer em determinado pixel sem evidência de fetch/DAC do VDP.
5. A ordem temporal exata entre CPU, VDP e IRQ pode ser inferida somente de um emulador aberto.

## 7. Plano de validação antes de alterar runtime

### Fase A — especificar o caso reproduzível
- Obter homebrew/teste sintético legal que execute writes R8/CRAM/R10 durante active display.
- Identificar região, resolução, revisão do console/VDP quando conhecida, versão dos emuladores e frame exato.
- Capturar frames lossless 256×192 (ou modo efetivamente usado), sem scaling, shader ou filtro.
- Separar efeito intra-frame de variação entre frames.

### Fase B — vetores sintéticos
Criar um gerador de eventos determinístico que:
1. inicialize padrão de fundo com colunas/cores distinguíveis;
2. avance para uma linha e fase horizontal controladas;
3. escreva R8 em diferentes posições relativas à linha;
4. repita para R9, CRAM, VRAM e R10;
5. atravesse a fronteira de linha com e sem instrução OUT que cruza a fronteira;
6. capture a primeira linha/pixel que muda.

Os resultados devem ser tabelados por evento: linha, offset de T-state conhecido, tipo de write, endereço/registrador, valor, primeiro pixel afetado e flags/IRQ. Não atribuir offsets de bus enquanto a CPU não emitir eventos temporais demonstrados.

### Fase C — oráculos e revisão
- Comparar resultados com documentação Sega, MAME, Ares, GearSystem e MEKA, registrando versões/commits.
- Avaliar ancestralidade comum e não tratar concordância entre emuladores como prova de hardware.
- Se a regra permanecer ambígua, classificar como hipótese e manter runtime inalterado.
- Teste físico/hardware ou captura de trace independente é necessário para elevar timings controversos a fato.

## 8. Contrato arquitetural recomendado

Esta auditoria não especifica um scheduler completo, mas determina que uma futura camada temporal deve:
- receber transações CPU/VDP com timestamp e ordem estável;
- avançar o raster até o evento antes de aplicar o write;
- separar estado programado do estado efetivamente usado pelo renderer/latches;
- distinguir R8, R9, CRAM, VRAM, display enable e IRQ, sem um latch genérico para todos;
- definir explicitamente o empate de evento na fronteira de linha;
- preservar o modelo scanline atual como fallback/test oracle durante migração;
- não duplicar contagem de T-states já atribuída ao scheduler;
- manter tracing opcional e sem alocação quando desativado.

Não implementar ainda. A auditoria Z80/barramento identificou que a CPU atual não emite uma sequência geral de transações timestamped; a integração precisa das duas auditorias antes de se escolher a API final.

## 9. Critérios de aceitação da futura implementação

- [ ] Regra R8 comprovada por documentação específica ou oráculo independente, incluindo fase horizontal e variações de VDP.
- [ ] R9 preservado nos casos já testados (write durante active display, latch após active display).
- [ ] Testes intralinha para R8/CRAM/VRAM com first-pixel divergence.
- [ ] Testes de fronteira de linha e instrução CPU que cruza a fronteira.
- [ ] Testes HINT/R10, status read/ack e linha INT em ordem temporal.
- [ ] Regressões de scroll fino, scroll vertical, coluna fixa, sprites, CRAM, PAL/NTSC e modos 192/224/240.
- [ ] CI no SHA candidato exato.
- [ ] Teste manual no Windows com capturas equivalentes de referência e do Sega Emulator.
- [ ] Evidência do caso Aladdin ou outro caso sintético que reproduza a classe de defeito.
- [ ] `TEMPORAL_PASS` somente após cobertura e evidência apropriadas.

## 10. Validação desta etapa documental

- Inventário remoto da base e leitura de VDP/arquitetura/testes: PASS.
- Confronto de R9 com documentação pública Sega Mark III: PASS, dentro do nível de abstração documentado.
- Identificação da ausência de latch explícito de R8 e timestamp de VRAM/CRAM: PASS por inspeção estática.
- Confirmação de timing físico de R8 por hardware: NOT RUN.
- Reprodução de Aladdin e comparação de capturas: NOT RUN.
- Testes Rust locais: NOT RUN.
- Teste visual em hardware/emulador: NOT RUN.
- Alterações no runtime: nenhuma.
- `TEMPORAL_PASS`: não declarado.

## 11. Próximo passo

Consolidar o contrato de eventos CPU↔VDP a partir das duas auditorias, mas antes disso obter ou produzir um teste sintético mínimo de R8/CRAM intralinha. Não corrigir R8 com um limiar horizontal copiado do MAME sem evidência independente. A PR #261 (raster geral) e a PR #265 (Aladdin) continuam sendo investigações separadas e precisam de revalidação de base/head antes de qualquer edição; não foram alteradas nesta etapa.

## Referências

1. Sega Mark III Software Reference Manual, seção VDP, reprodução pública SMS Power: https://www.smspower.org/Development/SMSOfficialDocs&num=5
2. Sega Master System VDP documentation (community technical reference): https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
3. MAME `sega315_5124_device`: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
4. Ares SMS VDP (investigation source list): https://github.com/ares-emulator/ares/tree/master/ares/ms/vdp
5. MEKA SMS VDP (investigation source list): https://github.com/ocornut/meka/tree/master/meka/srcs
6. GearSystem Video (investigation source list): https://github.com/drhelius/Gearsystem/tree/master/src
