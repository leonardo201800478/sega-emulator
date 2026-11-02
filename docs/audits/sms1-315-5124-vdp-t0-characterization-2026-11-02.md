# SMS1 VDP 315-5124 — caracterização T0

**Data:** 2026-11-02  
**Escopo:** VDP do SMS1 com ASIC 315-5124.  
**Estado:** T0 parcial — evidência documental inicial registrada; medições físicas, vetores independentes e cronologia sublinha continuam pendentes.  
**Branch de pesquisa:** `research/sms1-vdp-t0-characterization-2026-11-02`  
**Arquitetura-base:** `docs/architecture/sms-vdp-cycle-accurate-design.md` (PR #342, arquitetura-alvo).  
**Runtime:** nenhuma alteração nesta etapa.

## 1. Decisão de escopo

A primeira implementação do VDP novo terá como alvo explícito o ASIC **315-5124**. SMS2/315-5246 será uma etapa comparativa posterior, não uma segunda implementação paralela nesta fase.

A seleção da revisão será baseada em identidade ASIC/perfil explícito, não apenas no nome comercial ou no gabinete do console. A documentação SMS Power informa que 315-5124 é comum no Master System e 315-5246 no Master System II, mas registra exceções em ambos os sentidos. Variantes integradas TecToy não estão caracterizadas o suficiente e ficam fora desta primeira matriz.

NTSC/PAL continua sendo dimensão independente da revisão ASIC. Este documento não escolhe região padrão nem presume fase de reset.

## 2. Evidência documental registrada

### E-SMS1-001 — Identidade e diferenças conhecidas das revisões

- **Classe:** COMMUNITY_DOC.
- **Fonte:** SMS Power, [SMS1/SMS2 VDPs](https://www.smspower.org/Development/SMS1SMS2VDPs).
- **Trecho/seção:** descrição dos chips 315-5124 e 315-5246.
- **Fato publicado:** as duas revisões são distintas; as diferenças principais conhecidas são os bits de máscara de endereços derivados dos registradores legados TMS9918 no SMS1 e suporte a modos 224/240 linhas no SMS2.
- **Limite:** a página resume diferenças primárias conhecidas, não declara lista exaustiva de diferenças elétricas.
- **Uso:** justifica isolar revisão ASIC e não habilitar modos extra-height no perfil 315-5124.

### E-SMS1-002 — Clocks nominais do IC5 no manual de serviço

- **Classe:** PRIMARY_DOC (manual de serviço, transcrição OCR hospedada em SMS Power).
- **Fonte:** [Sega Master System Service Manual](https://www.smspower.org/Development/SegaMasterSystemServiceManual), seção de verificação do oscilador.
- **Trecho:** IC5 pino 30 = 10.738635 MHz ±100 Hz; IC5 pino 34 = 3.579545 MHz ±30 Hz.
- **Fato publicado:** o manual especifica as duas frequências nominais e tolerâncias para o circuito IC5.
- **Inferência permitida:** os valores nominais têm razão aritmética 3:1.
- **Limites:** o próprio site avisa que o conteúdo é OCR e pode conter erros. A tabela não determina duty cycle, fase inicial, atrasos internos, relação de fase observável com cada acesso Z80 ou variações regionais/revisionais.
- **Uso:** referência inicial de frequência; não congelar fase nem offsets a partir destes números.

### E-SMS1-003 — Contrato programável de registradores, status e portas

- **Classe:** PRIMARY_DOC (Software Reference Manual for the SEGA Mark III Console, reproduzido em SMS Power).
- **Fonte:** [SMS Official Docs](https://www.smspower.org/Development/SMSOfficialDocs%26num%3D5).
- **Trechos:** VDP ports/command protocol; registradores R0–R10; Appendix A/B.
- **Fatos publicados:**
  - VDP possui VRAM de 16 KiB e CRAM com 32 valores de 6 bits (duas banks de 16 cores).
  - $BE é data port; $BF é command/status port. O comando é escrito em dois bytes e a leitura de status limpa a requisição de interrupção.
  - A documentação recomenda espaçamento mínimo entre acessos: 16 Z80A T-states durante VBLANK e 29 T-states durante active video.
  - R8 controla scroll horizontal; R9 controla scroll vertical e é declarado como latched em VBlank.
  - R10 controla H-Line IRQ; o manual descreve a geração durante HBLANK, valor carregado um interrupt antes de tomar efeito e intervalo HBLANK aproximado de 10 μs.
  - status $BF expõe origem da interrupção, sprite overflow e sprite collision; o apêndice descreve latch em HBlank e clear na leitura.
- **Limites:** a restrição de 16/29 T-states é uma regra de cadência de acesso descrita para software. Não prova, por si só, que o VDP injeta WAIT automaticamente no Z80 ou qual transação sofre stall. O manual não fornece uma tabela completa de fetches por dot para o 315-5124.
- **Uso:** contrato funcional e restrições documentadas; transformar a cadência de acesso em WAIT só após evidência elétrica/teste.

### E-SMS1-004 — Máscaras de endereço específicas do 315-5124

- **Classe:** COMMUNITY_DOC.
- **Fonte:** SMS Power, [VDP Registers](https://www.smspower.org/Development/VDPRegisters).
- **Trechos:** overview; R2–R6.
- **Fatos publicados para SMS1:**
  - bits de máscara dos endereços são gates AND: bit de registrador 0 força o bit correspondente do endereço a zero;
  - R2 bit 0 mascara o bit alto da coordenada Y no fetch da name table em Mode 4/192, criando tilemap mirroring;
  - R3 aplica máscara a parte dos bits de endereço de pattern generator;
  - R4 aplica máscaras aos bits de endereço de pattern generator em outro par de bitplanes;
  - R5 bit 0 participa do mascaramento dos endereços X/tile da SAT;
  - R6 bits 1–0 restringem o conjunto de tiles de sprite.
- **Limites:** a própria página registra observações não testadas/efeitos incomuns para R2/R3 e R4, com ressalva explícita do autor para R3. Não converter todos os efeitos secundários em comportamento confirmado sem vetor específico.
- **Uso:** mapear a geração de endereço como função de revisão; criar testes individuais para cada máscara antes de integrar fetch temporal.

### E-SMS1-005 — Comportamento de zoom de sprites do SMS1

- **Classe:** COMMUNITY_DOC.
- **Fonte:** SMS Power, [Sprites](https://www.smspower.org/Development/Sprites).
- **Trechos:** Sprite scaling; table parsing.
- **Fatos publicados:**
  - até 64 sprites na SAT e buffer de até oito sprites por linha;
  - avaliação da SAT ocorre durante uma linha para selecionar sprites da próxima linha;
  - o overflow é marcado quando o buffer de oito está cheio e sprites adicionais qualificam para a linha, independentemente de transparência ou X;
  - no SMS1, zoom vertical aplica-se a todos os sprites, mas zoom horizontal não se aplica aos últimos quatro sprites encontrados na linha;
  - colisão é sinalizada quando pixels opacos de dois sprites se sobrepõem; a flag permanece até leitura do status.
- **Limites:** a página descreve o comportamento funcional, mas não entrega offsets de clock/dot para cada leitura SAT, fetch de padrão, overflow ou colisão.
- **Uso:** separar avaliação, buffer, fetch e composição; não calcular overflow somente a partir de sprites visíveis após recorte horizontal.

### E-SMS1-006 — Contador vertical e HBlank

- **Classe:** PRIMARY_DOC para o comportamento programável geral; COMMUNITY_DOC para tabelas de contador.
- **Fontes:** [SMS Official Docs](https://www.smspower.org/Development/SMSOfficialDocs%26num%3D5), [Scanline Counter](https://www.smspower.org/Development/ScanlineCounter).
- **Fatos publicados:** o manual Mark III documenta R10/H-Line IRQ associado a HBLANK, latência de um interrupt para alteração de R10 e R9 latched em VBlank. A página comunitária Scanline Counter apresenta sequências V-counter separadas por NTSC/PAL e modos 192/224/240.
- **Limite importante:** as tabelas comunitárias incluem modos extra-height que são SMS2-only. Não usar sequências 224/240 como evidência do SMS1. A página de H/V counter da documentação Game Gear também não deve ser transplantada automaticamente para o SMS1.
- **Uso:** criar matriz de V-counter 315-5124/192 linhas e perfil regional com fonte/teste próprio. Offset e wrap do H-counter permanecem UNKNOWN.

## 3. Matriz de escopo SMS1

| Subsistema | Estado T0 | Evidência / bloqueio |
|---|---|---|
| Identidade ASIC | DOCUMENTED | 315-5124 como alvo explícito; exceções de console precisam de identificação |
| Clock de referência | DOCUMENTED, fase UNKNOWN | Manual de serviço: 10.738635 MHz e 3.579545 MHz nominais |
| Perfil NTSC/PAL | PARTIAL | tabelas V-counter comunitárias; falta vetor SMS1 independente e fase |
| H-counter | UNKNOWN | sem offsets, wrap/clock sampling SMS1 suficientemente fixados |
| V-counter 192 | PARTIAL | documentação comunitária separa NTSC/PAL; confirmar com oráculo |
| HBlank/VBlank | PARTIAL | contrato de software documenta intervalos e efeitos, sem fase elétrica completa |
| R8 | DOCUMENTED funcionalmente | scroll horizontal; instante de aplicação intralinha UNKNOWN |
| R9 | DOCUMENTED | latch em VBlank; fronteira exata ainda não medida |
| R10/HINT | DOCUMENTED funcionalmente | HBLANK, reload com um interrupt de atraso; fase de borda ainda UNKNOWN |
| VRAM access | PARTIAL | protocolo e cadência 16/29 T-states; arbitragem/WAIT real UNKNOWN |
| Fetch de background | PARTIAL | endereço e máscaras funcionais; schedule por dot UNKNOWN |
| Sprite evaluation | PARTIAL | buffer de 8, regra de overflow e zoom SMS1 documentados; offsets temporais UNKNOWN |
| Sprite collision | PARTIAL | condição funcional documentada; ponto exato de assert UNKNOWN |
| CRAM/DAC | PARTIAL | CRAM digital 6-bit documentada; timing de write→pixel e colorimetria física UNKNOWN |
| ASIC differences | PARTIAL | diferenças principais SMS1/SMS2 conhecidas; não exaustivas |

## 4. Não inferir / não transportar

1. Não usar os 342 dots / 171 H counts / 228 CPU clocks citados no manual Game Gear como temporização confirmada do SMS1.
2. Não habilitar modos 224/240 no perfil 315-5124: SMS Power os identifica como recursos SMS2.
3. Não presumir que o clock nominal de 10.738635 MHz determina a fase de pixel, a fase inicial do Z80 ou a ordem CPU/fetch.
4. Não converter a regra 16/29 T-states em WAIT automático sem medir a transação afetada.
5. Não assumir que o renderer MAME/Ares/MEKA representa uma medição física. Código de emulador continua EMULATOR_SOURCE.
6. Não generalizar resultados 315-5124 para 315-5246 ou variantes TecToy integradas.

## 5. Próximos testes T0 obrigatórios

### T0-A — Identificação e clock
- Registrar placa, ASIC marcado, região, cristal e revisão de console de cada amostra física disponível.
- Medir pino/clock de referência e clock CPU com osciloscópio/analisador lógico; registrar instrumento, sondagem, tolerância e setup.
- Determinar relação de fase após reset sem presumir fase zero.

### T0-B — Raster/counters
- Construir homebrew mínimo que leia $7E em posições conhecidas e grave a sequência com timestamp CPU.
- Obter V-counter NTSC/PAL SMS1, wrap e linhas de transição.
- Caracterizar H-counter via light gun/port $7F apenas após separar o comportamento de entrada periférica da fase raster do VDP.
- Fixar primeira/última linha ativa e fronteiras HBlank/VBlank por perfil.

### T0-C — R8/R9/R10
- Alterar R8 em posições distintas da linha e medir a primeira linha/pixel afetado.
- Alterar R9 durante active display e durante VBlank; verificar fronteira de latch.
- Variar R10 antes/depois do HBlank e confirmar reload/um interrupt de atraso.
- Capturar status/IRQ e instante em que a CPU observa a linha INT.

### T0-D — Sprite engine
- Testes de 7/8/9/10 sprites, sprites transparentes, fora da tela, Y=$D0 e SAT alterada durante avaliação.
- Confirmar o padrão de zoom SMS1: 2, 4, 6 e 8 sprites elegíveis, com padrões assimétricos.
- Separar overflow, collision e prioridade; registrar primeira posição afetada.

### T0-E — VRAM/CRAM/fetch
- Variar writes VRAM antes/depois de possíveis fetch windows e procurar primeira saída afetada.
- Variar CRAM durante active display e HBlank.
- Caracterizar leitura read-ahead e intervalo mínimo de acesso; não assumir WAIT até a captura de barramento provar.
- Construir testes isolados para máscaras R2–R6.

## 6. Gate de saída T0

T0 não está concluído enquanto:
- não houver perfis NTSC e PAL explicitamente separados;
- não houver testes SMS1 para contadores, R8/R9/R10 e sprite zoom;
- não houver evidência de hardware ou oráculo independente para offsets/fases;
- as incógnitas estiverem identificadas e isoladas sem constantes especulativas.

**Resultado atual:** T0_PARTIAL. A evidência já permite iniciar estrutura de dados e testes funcionais SMS1, mas não autoriza implementar offsets de raster, fetch schedule, bus WAIT ou prioridade de eventos simultâneos. Nenhum status de precisão temporal é promovido.

## 7. Baseline do repositório — inventário, não oráculo

### Código legado observado em main

- Arquivo: `native/sms_core/src/vdp.rs`.
- O arquivo declara explicitamente modelo determinístico por scanline, não dot-cycle exact.
- `LINE_TSTATES = 228`; `advance()` acumula T-states e fecha a linha em `complete_scanline()`.
- O renderer lê VRAM/CRAM/registradores no fechamento da linha. As auditorias de 2026-10-29/30 já identificaram a consequência para writes intralinha.
- O perfil `VideoStandard` permite NTSC/PAL/PAL-M/Unknown e a implementação atual disponibiliza 192/224/240 linhas; não há gate de ASIC para 224/240 no modelo legado.
- O modelo atual mantém R9 pending/latch e atualiza o contador/IRQ por scanline. Isso não fixa a fase real de latch ou IRQ.
- O arquivo contém testes unitários de semântica e testes de caracterização do próprio renderer, incluindo asserts que demonstram que um write R8/CRAM no meio da linha é aplicado à linha inteira. Os comentários dos testes dizem explicitamente que são caracterização do modelo, não oráculo de hardware.

### Cobertura externa e golden masters

- A árvore de `main` não contém atualmente uma suíte de integração dedicada ao SMS VDP em `native/sms_core/tests/`; os testes VDP observados ficam no módulo `vdp.rs`.
- `docs/validation/golden-master-capture-contract.md` está marcado PROPOSED. Define a separação entre determinismo, integridade de baseline e fidelidade de hardware, mas não comprova que existam golden captures SMS aprovados.
- Portanto, T0 não pode declarar baseline visual aprovado apenas porque os testes unitários do VDP legado passam.

### Consequência para a reconstrução

O novo VDP deve começar com testes SMS1 sintéticos e oráculos explicitamente classificados. O VDP legado pode fornecer fixtures funcionais e imagens comparativas, mas nunca deve determinar os timestamps/fases do novo pipeline. A constante legada de 228 T-states por linha fica como hipótese de compatibilidade até ser confirmada por fonte/medição específica do SMS1.
### Discrepância documental — bits de status de sprites

A documentação oficial de software Mark III atribui status bit 6 a sprite overflow (nove sprites) e bit 5 a sprite collision (SMS Official Docs, seção INT e Appendix B). A página comunitária SMS Power/Sprites descreve overflow no bit 6 e também menciona bit 6 para collision. Há, portanto, uma inconsistência textual na fonte comunitária. Para a matriz inicial, preservar o mapeamento do manual Mark III como PRIMARY_DOC; não copiar a referência do bit de collision da página Sprites sem confirmação por teste de hardware/oráculo. Registrar um teste específico que gere overflow sem colisão e colisão sem overflow.
### E-SMS1-007 — SmsVdpTest (homebrew test candidate)

- **Classe:** COMMUNITY_TEST / INDEPENDENT_TEST_CANDIDATE; not yet accepted as a hardware oracle.
- **Fonte:** [FluBBaOfWard/SmsVdpTest](https://github.com/FluBBaOfWard/SmsVdpTest), default branch `main`, inspected commit `490c00b38c83012dee5a7a582097f080c210a3e7` (2026-06-17).
- **Proveniência/licença:** public source repository; GitHub reports no declared license and the tree has no LICENSE file. Do not vendor source or ROM binary into this project until permission/license is resolved. No binary or copyrighted media was added.
- **Cobertura declarada no README:** VDP port read/write sequences, read-ahead, address wrap, CRAM/VRAM mode changes, unused registers, NTSC/PAL V-counter, H-counter latch/read, frame/line IRQ scanline and H-counter, sprite collision and overflow conditions, sprite flags and horizontal-scroll/name-table latch visualization.
- **Separação interna:** assembly calls `StartSMSTest` for the SMS group and has separate Mega Drive and H-counter test paths. The H-counter path includes values/notes marked for MD; these must not be applied to SMS1.
- **Limitações explícitas do próprio projeto:** README asks for exactly 228 CPU cycles per scanline as a precondition. The TODO file still lists hardware work for status-bit behavior, SAT latch timing, sprite-pattern latch timing and Y-scroll latch timing. It also leaves some Y-scroll/H-clock positions unknown. Therefore the 228-cycle precondition and any timing notes are not promoted to PRIMARY_DOC or MEASURED_HARDWARE.
- **Uso proposto:** use the SMS-only test group after legal provenance review; build a test manifest with test name, assembly routine, preconditions, expected output, target ASIC/region, and whether expected output was verified on physical 315-5124. Treat timing-dependent tests as candidate oracles until that evidence exists.