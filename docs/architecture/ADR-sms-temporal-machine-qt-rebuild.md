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
- snapshots completos, incluindo contadores, saídas, LFSR, registrador latched, fase do divisor, fase de amostragem, filtro e filas de amostras, com política explícita de buffers de host;
- separar o perfil digital do SN76489 do circuito de saída da placa: o perfil SMS do chip mantém LFSR/divisor/registradores; uma etapa de saída compartilhada poderá modelar a curva de amplitude/resposta de placa quando houver medição;
- a variante de resistor de saída discutida para algumas placas SMS2 não será tratada como clipping interno do SN76489 nem inferida automaticamente apenas pelo ASIC/console; será um perfil de circuito explícito, com curva medida e estado serializável. A discussão SMS Power é COMMUNITY_DOC e não substitui medição: https://www.smspower.org/forums/17121-HelpSoundDifferenceInMasterSystemVDPRevisions;
- o FIR/resampler atual do core é filtro de saída digital, não um modelo elétrico completo da placa; não duplicar filtros nem adicionar decay/RC sem evidência de impacto e parâmetros medidos.

O PSG é integrado ao VDP/ASIC SMS e ao mapa real de I/O. A escrita de porta deve produzir efeito no instante de conclusão da transação de barramento, não no instante em que Python ou o dispositivo de áudio solicitar uma amostra.

### D4 — VDP SMS em crate compartilhado, por ASIC

O primeiro alvo funcional é o **SMS1 com ASIC 315-5124**. O SMS2/315-5246 será uma etapa comparativa posterior, não uma segunda implementação paralela nesta fase. A revisão será selecionada por identidade ASIC/perfil explícito; não inferir revisão apenas pelo nome comercial do console. NTSC/PAL é dimensão independente.

O engine fica no crate compartilhado `native/video_core`, pacote Rust `sega-video`, módulo `sega_video::sms`. O crate pode hospedar outros chips de vídeo em módulos separados, mas SMS VDP e TMS9918A do SG-1000 são chips diferentes: compartilhar o crate/utilitários comprovadamente comuns não autoriza fundir registradores, estado ou pipeline. `native/video_runtime` permanece como contrato/transportador de frames e camada host de apresentação.

O `sms_core/src/vdp.rs` é um baseline funcional scanline-level, não um oráculo de hardware; `vdp_sms1.rs` é uma fundação parcial. Não manter dois VDPs SMS paralelos. Consolidar semântica funcional validada no novo engine e manter incógnitas temporais explicitamente bloqueadas.

Requisitos funcionais iniciais do 315-5124:
- VRAM de 16 KiB, CRAM de 32 bytes com valores digitais de 6 bits, portas espelhadas e protocolo de controle/data;
- command latch de dois bytes, read-ahead buffer, wrap de endereço, leitura/clear de status e cancelamento de comando parcial;
- Mode 4, modos herdados suportados pelo VDP SMS, scroll, paletas, prioridade por pixel, transparência e backdrop;
- máscaras de endereço SMS1 documentadas em R2–R6, com assertions separadas e ressalvas para efeitos secundários ainda não testados;
- 64 entradas SAT e buffer lógico de até oito sprites por linha; overflow baseado em sprites elegíveis, não apenas pixels finalmente desenhados;
- peculiaridade SMS1 de zoom: zoom vertical aplica-se a todos; zoom horizontal não se aplica aos últimos quatro sprites encontrados na linha;
- `SpriteLimitMode::HardwareLimit` como padrão; modo sem limite apenas como opção explícita de compatibilidade, sem alterar overflow/status de hardware;
- máscara R0 bit 5 e sprite shift R0 bit 3 como controles distintos; status bit 6 overflow e bit 5 collision conforme referência PRIMARY_DOC, mantendo a divergência editorial da fonte comunitária registrada;
- 192 linhas no perfil SMS1. Modos 224/240 são bloqueados no 315-5124 e ficam para o perfil 315-5246 posterior;
- R8/R9/R10, H/V counters, VBlank/HINT e status com semântica funcional documentada, sem inventar fase ou offset sublinha.

Limites temporais obrigatórios:
- A documentação SMS1 descreve avaliação da SAT durante uma linha para selecionar sprites da linha seguinte, mas não fornece offsets de clock/dot de avaliação, fetch, overflow ou colisão. Modelar a regra funcional; manter fases/fetch slots UNKNOWN.
- A hipótese MAME de colisão suprimida na coluna esquerda mascarada e o comportamento especial de fine-scroll são itens de investigação diferencial, não requisitos confirmados sem teste independente.
- H-counter offsets/wrap, fase de reset, R8 intralinha, fronteira exata de R9, fase de HINT/VBlank, fetch slots, arbitragem VRAM/CRAM e prioridade de eventos simultâneos permanecem BLOCKED até evidência SMS1 apropriada.
- A cadência de acesso de 16 T-states em VBlank e 29 T-states em active video é regra de software documentada; não prova WAIT automático no Z80.
- Aplicar ADR-0016: granularidade temporal proporcional ao efeito observável. Não criar eventos por dot/pixel nem pipeline interno sem consumidor/teste que demonstre necessidade.

### D5 — Representação da tela, overscan e coluna esquerda

Não aplicar correção cosmética ao framebuffer do hardware. Separar:
1. **Raster do hardware:** posição do feixe, região ativa, bordas, blanking/sync e VBlank, com dimensões e offsets explicitamente descritos no metadado do frame.
2. **Área ativa do VDP:** 256 pixels horizontais e a altura efetiva do modo (192/224/240 conforme perfil).
3. **Máscara da coluna esquerda:** R0 bit 5 substitui os 8 pixels da coluna zero pela cor de backdrop/overscan; isso é comportamento do VDP, não um crop implícito.
4. **Deslocamento de sprites:** R0 bit 3 desloca os sprites 8 pixels à esquerda; não é a mesma coisa que mascarar a coluna.
5. **Viewport do frontend:** escolha de enquadramento/crop e proporção sem alterar pixels, registradores, clocks ou raster emulado.

A Qt deve oferecer as opções **Não**, **Auto** e **Sempre** para ocultar a barra esquerda de 8 pixels. O padrão será **Auto**: corta os 8 pixels somente quando o jogo habilitar R0 bit 5 e quando o modo de apresentação não estiver exibindo overscan completo. Esse comportamento segue o modelo de opção de apresentação usado pelo Gearsystem, mas o corte ocorrerá exclusivamente no viewport. Os presets de overscan serão **Desativado**, **Superior/Inferior**, **Completo 284 px** e **Completo 320 px**; as alturas e regiões efetivas devem vir dos metadados do raster e do standard (não de padding inventado). Os presets 284/320 e 240/288 são compatibilidade de apresentação a validar contra o raster de cada revisão, não uma afirmação de que toda placa gera a mesma área visível. Em overscan completo, não aplicar o crop automático da barra esquerda. Deve ser possível visualizar o frame ativo sem corte e inspecionar o raster/overscan completo quando disponível. O frame bruto e seus metadados permanecem disponíveis para testes, capturas e diagnóstico. O contrato de frame deve transportar ao menos `active_rect`, `visible_rect`, proporção de exibição, timestamp da máquina e base de tempo; a dica de máscara da coluna esquerda só permite `Auto` quando os 8 pixels estiverem mascarados durante toda a altura ativa. A preferência visual jamais altera o estado do VDP.

A divisão exata dos 342 dots horizontais, os offsets do H counter, a cor de cada intervalo de blanking/borda e as particularidades por revisão serão tratados como matriz de evidências. Valores publicados em pesquisas que alertam para diferenças entre SMS e Genesis não serão promovidos a verdade de silício sem confirmação.

### D6 — Base temporal e clocks

O Scheduler continua sendo a única autoridade temporal. Para SMS1, o manual de serviço registra nominalmente IC5 pino 30 = 10.738635 MHz ±100 Hz e IC5 pino 34 = 3.579545 MHz ±30 Hz; a razão aritmética nominal é 3:1. Essa relação pode ser representada por domínios racionais, mas não determina fase inicial, duty cycle, offsets internos ou amostragem relativa CPU/VDP. Não assumir fase zero.

O perfil NTSC e o perfil PAL devem permanecer separados. A divergência entre 3.546893 MHz e 3.546895 MHz para CPU PAL continua aberta e exige reconciliação documental/medida antes de ser fixada. Não importar para SMS1 os 342 dots/171 H-counts/228 CPU clocks citados para Game Gear; a constante legada de 228 T-states por linha é hipótese de compatibilidade, não fato físico confirmado para o 315-5124.

O VDP recebe avanços do Scheduler em spans/eventos e pode processar blocos até a próxima fronteira observável. O domínio de clock não obriga o dispositivo a executar um evento por dot. A menor granularidade suficiente é escolhida por comportamento documentado/testado, em conformidade com ADR-0016.

A máquina deverá preservar causalidade e equivalência entre:
- execução contínua;
- execução em blocos de tamanhos diferentes;
- pausa/retomada;
- save/restore em safe points;
- replay determinístico.

O áudio produzido e os frames de vídeo devem carregar timestamps derivados da timeline da máquina. A fase temporal só será adicionada ao estado quando houver evidência e consumidor observável que a exijam.

### D7 — Contrato PyO3 e novo frontend

A API Python será coarse-grained. O frontend não recebe callbacks por instrução, M-cycle, T-state, dot, pixel ou amostra. Os contratos de sessão e capacidades devem distinguir backend selecionado, backend efetivo e estado real da máquina. Não expor comandos de VDP/PSG como API de jogador; diagnósticos podem consultar snapshots imutáveis.

O novo frontend poderá começar como shell de sessão, biblioteca, preferências, monitor de estado, visualização de frame e saída de áudio somente quando esses dados forem fornecidos pelo runtime temporal. Recursos ainda não implementados devem permanecer indisponíveis, não simulados.

### D8 — Expansão FM opcional

O PSG SN76489 não substitui o FM expansion. O perfil de máquina já distingue `FmHardware`; quando `YM2413` estiver presente, a compatibilidade completa dessa variante exigirá o core YM2413 compartilhado, seu clock, estado, mixer e integração temporal. Essa expansão é um gate separado do PSG base e não pode ser anunciada como suportada até existir implementação e validação próprias.

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
- perfis FM opcionais só serem declarados suportados quando YM2413 e seu mixer temporal estiverem implementados;
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
