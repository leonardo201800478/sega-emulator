# ADR — Reconstrução Qt Temporal e fechamento vertical do Master System

- **Status:** ACCEPTED — decisão de arquitetura e produto
- **Data:** 2026-11-05
- **Branch de trabalho:** `feat/qt-temporal-clean-room-20261105`
- **Baseline auditada:** `b0bda60e3fbfa56f820fe77d3098f8b82b5af69d`
- **Escopo:** novo frontend Qt, runtime SMS temporal, VDP, PSG e isolamento do runtime legado.
- **Decisão do produto:** aceita-se a interrupção temporária da execução de jogos pelo runtime SMS antigo enquanto o novo runtime temporal é concluído. Não se declara paridade funcional até os gates desta ADR passarem.

## 1. Contexto e estado observado

A `main` contém duas linhas SMS distintas:

1. **Runtime anterior:** Qt/PySide6 → adaptador Python `SMSMachine` → PyO3 `NativeSMSMachine` → `sega_sms_core::SmsMachine`. Esse caminho contém a execução SMS funcional anteriormente reportada.
2. **Fatia temporal:** `SmsTemporalMachine` → `SmsTemporalBus` → microsequenciador Z80 + scheduler/coordenador. O barramento temporal ainda registra escritas de portas em trace limitado, devolve open bus para dispositivos não conectados e não integra VDP/PSG.

O workspace já possui o crate compartilhado `native/sn76489`, com perfil SMS (divisor /16 e LFSR de 16 bits com taps 0 e 3), além de:
- `native/sms_core/src/vdp.rs`: renderer SMS/TMS por scanline, com 256 px de largura e altura configurável, mas não conectado à fatia temporal;
- `native/sms_core/src/vdp_sms1.rs`: fundação independente para o VDP SMS1 315-5124, com VRAM/CRAM, registradores, portas e composição funcional de scanline Mode 4; fetch slots, raster, interrupções e temporização ainda não integrados;
- `native/machine_scheduler` e `native/temporal_core`: infraestrutura compartilhada de tempo, domínios de clock e eventos.

Logo, não se deve criar um segundo PSG nem promover o renderer antigo a VDP temporal por simples renomeação. A integração deve reaproveitar o chip PSG compartilhado e evoluir o VDP SMS em módulos com identidade de revisão explícita.

## 2. Decisões

### D1 — Novo Qt, sem modernização incremental do frontend antigo

O frontend atual será arquivado em `legacy/qt/`. O novo frontend será escrito em módulos novos e terá como backend SMS exclusivo o contrato temporal. A aplicação permanece Python/PySide6 apenas como apresentação, launcher, biblioteca, preferências, logs, telemetria e orquestração coarse-grained.

Camadas:
- Presentation: janela, menus, toolbar, viewport, diálogos e acessibilidade.
- Application: controller e serviços de sessão, mídia, preferências, capabilities e diagnóstico.
- PyO3: API de alto nível para criar/carregar, start, pause, stop, reset, step-frame e consultar snapshots/filas de saída.
- Rust: único proprietário de CPU, barramento, memória, VDP, PSG, scheduler, raster, interrupções, áudio e vídeo.

A interface não terá timers que avancem o hardware, loops de FPS que controlem a máquina, lógica de chip, callbacks por instrução/T-state/pixel/amostra ou fallback Python.

### D2 — Arquivamento e isolamento do legado

- Qt antigo: mover para `legacy/qt/`, fora dos entrypoints e do pacote operacional.
- Adaptadores Python SMS que chamam `NativeSMSMachine`: arquivar em `legacy/`; não serão importados pelo novo frontend.
- `legacy/python_reference/` permanece referência histórica, sem instalação/importação operacional.
- O runtime Rust `SmsMachine` anterior deve deixar de compilar no build padrão. A implementação poderá ser preservada temporariamente atrás de feature explícita não-default ou em crate arquivado excluído do workspace, conforme a auditoria de dependências determinar.
- O caminho PyO3 antigo `NativeSMSMachine` deve ser removido do build operacional; a ponte temporal SMS será exposta sem depender da feature antiga.
- Não desabilitar nem duplicar Z80 compartilhado, scheduler, mapper/memória, PSG SN76489 ou módulos que o caminho temporal/SG-1000 consuma. `cargo` não compila Python; a retirada do Qt/Python antigo exige também ajustar entrypoints, packaging, imports, testes e workflows.

A decisão sobre feature gating versus crate arquivado será tomada após a matriz de dependências. Nenhum módulo será movido/desativado por nome sem verificar seus consumidores.

### D3 — PSG SMS completo e temporal, reutilizando `sega-sn76489`

O SMS utiliza o PSG integrado à família SN76489. O núcleo compartilhado `Sn76489Core` será a única implementação digital. A máquina SMS fornece o perfil e o relógio corretos e reutiliza `sega-audio-runtime::ScheduledPsg` para consumir os avanços de clock do Scheduler e aplicar escritas no boundary de dispatch; não será criada uma segunda ponte temporal local.

Requisitos:
- quatro canais: três tons e um ruído;
- registradores de tom de 10 bits, quatro atenuadores de 4 bits e controle de ruído de 3 bits;
- protocolo latch/data com registrador latched persistente; toda escrita data deve atualizar o registrador selecionado, inclusive volume/ruído;
- atualização imediata de tom após cada byte, sem esperar uma escrita de 10 bits;
- períodos de ruído 0x10, 0x20, 0x40 ou tom 2;
- perfil SMS de LFSR 16-bit, seed e taps SMS (bits 0 e 3, feedback em bit 15); não reutilizar o perfil SG-1000;
- divisor interno /16, fase de divisor preservada e bordas determinísticas;
- saída digital/mixagem e resampling separados da evolução temporal do chip;
- clock do PSG derivado do mesmo domínio físico da máquina, sem relógio de parede e sem avanço provocado pela demanda do dispositivo de áudio;
- snapshots completos, incluindo contadores, saídas, LFSR, registrador latched, fase do divisor, fase de amostragem, filtro e filas de amostras, com política explícita de buffers de host.

O PSG é integrado ao VDP/ASIC SMS e ao mapa real de I/O. A escrita de porta deve produzir efeito no instante de conclusão da transação de barramento, não no instante em que Python ou o dispositivo de áudio solicitar uma amostra.

### D4 — VDP SMS de alta fidelidade com revisão explícita

Modelar separadamente a identidade do VDP (315-5124 SMS1, 315-5246 SMS2 e revisões adicionais somente com evidência) da região/standard de vídeo. Não inferir a revisão do VDP apenas do nome do console.

Requisitos funcionais e temporais:
- 16 KiB VRAM, CRAM de 32 bytes com formato e máscaras por revisão, 16 registradores e portas espelhadas;
- protocolo de duas escritas no control port, data port, read-ahead buffer, wrap de endereço, leitura de status e cancelamento de comando parcial;
- Mode 4 com tilemap 32 colunas, paletas de background/sprites, prioridade por pixel, transparência e scroll horizontal/vertical;
- compatibilidade com modos TMS9918 herdados quando suportados pelo perfil de máquina;
- modos de 192 linhas e modos estendidos 224/240 somente nos perfis/revisões que os suportam;
- tabela de atributos de sprites, 64 sprites, 8 sprites por scanline no perfil com limite físico, overflow e collision flags, prioridade, transparência, tamanho 8x8/8x16, deslocamento horizontal e padrões;
- R0 bit 7/6 scroll locks, bit 5 left-column blank, bit 4 line IRQ enable, bit 3 sprite shift-left e bits de modo;
- R1 screen enable, VBlank IRQ, seleção de altura/modo e tamanho de sprite;
- R7 backdrop/overscan color, R8 horizontal scroll, R9 vertical scroll e R10 line counter;
- status/IRQ e seus clears conforme leitura de status;
- H/V counters, TH latch para lightgun, frame/scanline/dot e efeitos de escrita em registradores/VRAM/CRAM no instante em que o barramento conclui a operação;
- snapshots completos de VRAM, CRAM, registradores, status, latches, fetch pipeline, sprite evaluation, beam/raster, interrupções, framebuffer parcial e eventos pendentes.

A granularidade temporal será a menor que reproduza os efeitos observáveis: scanline/frame não substituem dot/fetch quando uma escrita raster, scroll latch, sprite evaluation, counter ou IRQ puder ser observada dentro da linha. Não simular transistor/pino sem requisito demonstrável.

### D5 — Representação da tela, overscan e coluna esquerda

Não aplicar correção cosmética ao framebuffer do hardware. Separar:
1. **Raster do hardware:** posição do feixe, região ativa, bordas, blanking/sync e VBlank, com dimensões e offsets explicitamente descritos no metadado do frame.
2. **Área ativa do VDP:** 256 pixels horizontais e a altura efetiva do modo (192/224/240 conforme perfil).
3. **Máscara da coluna esquerda:** R0 bit 5 substitui os 8 pixels da coluna zero pela cor de backdrop/overscan; isso é comportamento do VDP, não um crop implícito.
4. **Deslocamento de sprites:** R0 bit 3 desloca os sprites 8 pixels à esquerda; não é a mesma coisa que mascarar a coluna.
5. **Viewport do frontend:** escolha de enquadramento/crop e proporção sem alterar pixels, registradores, clocks ou raster emulado.

A Qt deve oferecer as opções **Não**, **Auto** e **Sempre** para ocultar a barra esquerda de 8 pixels. O padrão será **Auto**: corta os 8 pixels somente quando o jogo habilitar R0 bit 5 e quando o modo de apresentação não estiver exibindo overscan completo. Esse comportamento segue o modelo de opção de apresentação usado pelo Gearsystem, mas o corte ocorrerá exclusivamente no viewport. Os presets de overscan serão **Desativado**, **Superior/Inferior**, **Completo 284 px** e **Completo 320 px**; as alturas e regiões efetivas devem vir dos metadados do raster e do standard (não de padding inventado). Os presets 284/320 e 240/288 são compatibilidade de apresentação a validar contra o raster de cada revisão, não uma afirmação de que toda placa gera a mesma área visível. Em overscan completo, não aplicar o crop automático da barra esquerda. Deve ser possível visualizar o frame ativo sem corte e inspecionar o raster/overscan completo quando disponível. O frame bruto e seus metadados permanecem disponíveis para testes, capturas e diagnóstico. A preferência visual jamais altera o estado do VDP.

A divisão exata dos 342 dots horizontais, os offsets do H counter, a cor de cada intervalo de blanking/borda e as particularidades por revisão serão tratados como matriz de evidências. Valores publicados em pesquisas que alertam para diferenças entre SMS e Genesis não serão promovidos a verdade de silício sem confirmação.

### D6 — Base temporal e clocks

O scheduler é o único proprietário do tempo. A máquina deve declarar domínios de clock explícitos e relações racionais, evitando frequências arredondadas como fonte de deriva. O perfil NTSC de serviço identifica 10.738635 MHz no clock de vídeo e 3.579545 MHz na CPU. A documentação técnica consultada reporta 3.546893 MHz para CPU PAL; o código legado atual contém 3.546895 MHz, divergência de 2 Hz que deve ser investigada e corrigida por evidência antes de reutilização.

A máquina deverá preservar causalidade e equivalência entre:
- execução contínua;
- execução em blocos de tamanhos diferentes;
- pausa/retomada;
- save/restore em safe points;
- replay determinístico.

O áudio produzido e os frames de vídeo devem carregar timestamps derivados da timeline da máquina.

### D7 — Contrato PyO3 e novo frontend

A API Python será coarse-grained. O frontend não recebe callbacks por instrução, M-cycle, T-state, dot, pixel ou amostra. Os contratos de sessão e capacidades devem distinguir backend selecionado, backend efetivo e estado real da máquina. Não expor comandos de VDP/PSG como API de jogador; diagnósticos podem consultar snapshots imutáveis.

O novo frontend poderá começar como shell de sessão, biblioteca, preferências, monitor de estado, visualização de frame e saída de áudio somente quando esses dados forem fornecidos pelo runtime temporal. Recursos ainda não implementados devem permanecer indisponíveis, não simulados.

## 3. Sequência de implementação

1. **S0 — Arquitetura e isolamento:** inventário de imports/consumidores, ADR, contratos de sessão e definição do build operacional sem Legacy.
2. **S1 — PSG temporal:** auditar `sega-sn76489`, corrigir perfil/clock/estado se necessário, conectar escrita de I/O no timestamp de commit, scheduler, blocos de áudio e snapshots.
3. **S2 — VDP funcional:** consolidar `vdp_sms1` e o código comprovado em engine única parametrizada por revisão; fechar ports, memória, modos, scroll, paleta, sprites e status.
4. **S3 — VDP temporal:** clock/raster/counters/fetch/evaluation/IRQ e efeitos de escritas em pontos de barramento; integrar à mesma timeline do PSG e Z80.
5. **S4 — Máquina vertical:** mapper, memória, I/O, Z80, VDP, PSG, interrupções, input e outputs de frame/áudio numa única máquina temporal.
6. **S5 — PyO3:** API de sessão, execução contínua controlada por Rust, comandos coarse-grained, snapshots e filas de frame/áudio.
7. **S6 — Novo Qt:** shell modular e apresentação de vídeo/áudio reais; configuração de overscan/viewport exclusivamente visual.
8. **S7 — Remoção operacional do legado:** mover Qt e adaptadores antigos para `legacy/`, retirar entrypoints/packaging/imports, desativar feature/código Rust antigo no build padrão e revisar workflows.
9. **S8 — Validação sistêmica:** suites funcionais, temporais, diferenciais, raster, áudio, determinismo, save states, regressões SG-1000 e smoke tests Windows.

As etapas podem ser divididas em PRs pequenos; não serão condensadas numa migração monolítica.

## 4. Critérios de aceitação — gate SMS temporal

O Master System só poderá ser declarado operacional quando:
- ROMs de referência/homebrew executarem até gameplay com Z80, memória, mapper, VDP, PSG, I/O e interrupções integrados;
- VDP e PSG responderem às escritas no instante de commit do barramento;
- raster, contadores, IRQ, scroll e sprite status passarem testes temporais e diferenciais;
- coluna esquerda mascarada, sprite shift, backdrop e overscan forem testados isoladamente e em combinações;
- 192/224/240 e perfis SMS1/SMS2 tiverem suporte/limites documentados;
- PSG passar vetores de latch/data, tons, volumes, ruído periódico/white, LFSR SMS, clock divider e continuidade de amostras;
- execução em blocos e contínua forem equivalentes; save/restore/replay preservarem 100% do estado temporal;
- saída de áudio e vídeo tiver timestamps e não controlar a timeline;
- novo Qt executar somente pelo contrato temporal;
- SG-1000 continuar passando sua suíte sem regressão;
- Rust fmt, Clippy sem warnings, testes, Python pytest/Ruff/format, build Maturin/PyO3 e testes de host aplicáveis forem classificados individualmente PASS/FAIL/BLOCKED/NOT RUN;
- nenhuma suíte SKIPPED seja contabilizada como PASS.

## 5. Fontes técnicas iniciais

- SMS Power — VDP Registers: https://www.smspower.org/Development/VDPRegisters
- SMS Power — Modes: https://www.smspower.org/Development/Modes
- SMS Power — SN76489: https://www.smspower.org/Development/SN76489
- SMS Power — Software Reference Manual for the SEGA Mark III: https://www.smspower.org/Development/SMSOfficialDocs
- SMS Power — Master System Service Manual: https://www.smspower.org/Development/SegaMasterSystemServiceManual
- MAME — VDP 315-5124/315-5246 (referência comparativa; declara TODO para mid-scanline changes e restrições de acesso VRAM/CRAM): https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- Gearsystem — implementação e opção Hide Left Bar (referência comparativa, não oráculo de silício): https://github.com/drhelius/Gearsystem/blob/master/src/Video.cpp
- Gearsystem — documentação VDP derivada do trabalho de Charles MacDonald: https://github.com/drhelius/Gearsystem/blob/master/skills/gearsystem-debugging/references/msvdp.md
- Referência independente VDP (deve ser confrontada com SMS Power e manual): https://github.com/franckverrot/EmulationResources/blob/master/consoles/sms-gg/Sega%20Master%20System%20VDP%20documentation.txt
- Cargo Features: https://doc.rust-lang.org/cargo/reference/features.html
- Qt for Python 6.12: https://doc.qt.io/qtforpython-6/
- PyO3 0.30.1: https://pyo3.rs/v0.30.1/
- Maturin 1.16.0: https://www.maturin.rs/

## 6. Limites de evidência

Esta ADR define decisões e critérios; não declara PSG/VDP temporal concluídos, validação em hardware físico ou paridade de jogos. Cada comportamento não documentado deve ser rotulado por evidência (manual, pesquisa reversa, teste em hardware, comparação independente ou inferência). A precisão temporal deve corresponder a um efeito observável e possuir teste reprodutível.
