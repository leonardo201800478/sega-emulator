# Roadmap Mestre Cadenciado — Sega Emulator (SG-1000 até Sega CD 32X)

**Status:** Única rota normativamente ativa de implementação e arquitetura do projeto.  
**Diretriz de Engenharia:** Arquitetura 100% modular sobre crates compartilhados Rust (`sega-core`, `sega-cpus`, `sega-video`, `sega-audio`, `sega-scheduler`, `sega-hosts`, `sega-media`, `sega-pyo3`). Proibida a criação de lógicas de hardware isoladas ou duplicadas por console.  
**Baseline Auditada:** `main` em `04ee09f9839bfe29a595abbed831d055cc8d177a` (2026-11-03, PRs #471/E4A e #472/E4B integradas).  
**Candidato validado aguardando integração:** PR #473 / E4C, SHA `bac497462a3fc13a1f47892d25d435beefddd6d8`; CI `41742682599` 4/4 PASS.  
**Runtime & Host Target:** 100% Rust para hardware, tempo, barramento, render DirectX 12, áudio WASAPI/XAudio2 + DSP e input SDL3. Python/Qt (PySide6) atua estritamente como launcher, biblioteca, preferências e telemetria.

---

## 1. Diretriz de Variantes de Hardware e Regiões

Cada sistema principal é representado pelo seu modelo original/base. As revisões posteriores e variantes de hardware (SMS I vs SMS II, SG-1000 I vs SG-1000 II, Mega Drive I vs Mega Drive II, Mega-CD Model 1 vs Model 2) operam sob o mesmo pipeline executável Rust, alternando dinamicamente registradores, encoders e ASICs através do perfil de hardware acionado pelo cabeçalho da ROM ou banco de DATs.

### Matriz de Perfis de Frequência, Região e Vídeo

| Plataforma / Modelo | Regiões Suportadas | Padrão / Frequência Vertical | Resoluções Ativas |
| :--- | :--- | :--- | :--- |
| **SG-1000 I & II** | Japão, Nova Zelândia (PAL), Taiwan | NTSC (~59,92 Hz), PAL (~49,70 Hz) | 256 × 192 |
| **Master System I & II / Mark III** | Japão, EUA, Europa, Brasil (PAL-M), Ásia | NTSC (~59,92 Hz), PAL (~49,70 Hz), PAL-M (~59,92 Hz) | 256×192, 256×224, 256×240 (PAL) |
| **Game Gear** | Japão, NTSC-U, PAL Export | NTSC (~59,92 Hz), PAL (~49,70 Hz) | 160 × 144 (Crop LCD de 256×192) |
| **Mega Drive I & II / Genesis** | Japão, EUA (Genesis), Europa, Brasil (PAL-M), Ásia | NTSC (~59,92 Hz), PAL (~49,70 Hz), PAL-M (~59,92 Hz) | H32 (256×224/240), H40 (320×224/240), Interlaced (448i/480i) |
| **Sega CD / Mega-CD (Model 1 & 2)** | Japão, EUA, Europa | Herda do hospedeiro Mega Drive | H32 (256×224/240), H40 (320×224/240), Interlaced |
| **Sega 32X / Super 32X** | Japão, EUA, Europa | NTSC (~59,92 Hz), PAL (~49,70 Hz) | 320×224, 320×240 (PAL), 320×448i, 320×480i |
| **Sega CD 32X** | Japão, EUA, Europa | Herda do hospedeiro Mega Drive | 320×224, 320×240, 320×448i, 320×480i |

---

## 2. Estrutura do Workspace Rust (`crates/`)

```text
sega-emulator/
├── crates/
│   ├── sega-core/         # Abstração de máquina, barramento transacional suspensível, mappers compartilhados
│   ├── sega-cpus/         # Z80 (microsequenciado), Motorola 68000, Hitachi SH-2 (Master/Slave)
│   ├── sega-video/        # TMS9918A/TMS9929, SMS VDP 315-5124, Genesis VDP 315-5313, 32X VDP
│   ├── sega-audio/        # PSG SN76489, YM2413, YM2612, Ricoh RF5C164, PWM 32X Audio
│   ├── sega-scheduler/    # MachineScheduler, clock domains, timekeeper e filas de eventos
│   ├── sega-hosts/        # DX12 Render Engine, WASAPI/XAudio2 + DSP, SDL3 Input Endpoint
│   ├── sega-media/        # Leitor de DATs, Header Parser, BIOS Manager e Auto-Boot
│   └── sega-pyo3/         # Extension module PyO3 (fronteira coarse-grained de controle/telemetria)
```

---

## 3. Cadenciamento de Fases e Sprints (Fases P0 a P9)

### FASE P0 — Infraestrutura Base, DATs e Hosts Rust
**Foco:** Estabelecer a fundação de mídias, auditoria e endpoints físicos de host em Rust.
- **Task P0.1 (`sega-media`):** Integrar o parser de arquivos `.dat` na pasta `dats/` e o identificador de ROMs via Hash SHA-256 e análise de cabeçalho.
- **Task P0.2 (`sega-media`):** Implementar o gerenciador de BIOS com suporte a boot automático (quando permitido/necessário) e interrupção controlada de boot em caso de inconsistência de região.
- **Task P0.3 (`sega-hosts`):** Finalizar o pipeline de vídeo DX12 em Rust com suporte a modos Windowed, Borderless e Fullscreen, seleção de monitores, VSync, VRR, G-Sync, proporções (1:1, 4:3, 16:9, Mailbox) e pipeline de shaders em até 3 passadas combináveis (Bilinear, Scanlines, NTSC CRT).
- **Task P0.4 (`sega-hosts`):** Finalizar o backend de áudio WASAPI/XAudio2 com controle avançado de DSP (amostragem, latência, volume master, estéreo/mono automático).
- **Task P0.5 (`sega-hosts`):** Implementar o subsistema de entrada SDL3 com Hotplug em tempo real, tratamento de zona morta analógica pré-mapeamento, e perfis hierárquicos (*Jogo > Sistema > Periférico*). Mapeamento isolado dos botões físicos **PAUSE** e **RESET/SW**.

---

### FASE P1 — Master System (SMS I & SMS II / Mark III) — *Plataforma Base*
**Foco:** Construção da primeira fatia vertical temporizada ponta a ponta sobre os crates compartilhados Rust, estabelecendo o padrão para os demais consoles.

#### Sprint SMS-1: Integração Z80 & Barramento Transacional
- **Task SMS-1.1 (`sega-cpus` + `sega-core`):** Conectar os M-cycles/T-states do microsequenciador Z80 em `sega-cpus` ao `SmsTemporalBus` via `BusTransaction`/`BusOperationResult`.
- **Task SMS-1.2 (`sega-core`):** Consolidar a janela fixa de RAM (8 KiB com espelhamento), RAM de cartucho e tratamento de escritas nos registradores do Sega Mapper SMS ($FFFD, $FFFE, $FFFF).

#### Sprint SMS-2: VDP, PSG e Scheduler Integration
- **Task SMS-2.1 (`sega-video` + `sega-scheduler`):** Conectar o SMS VDP (315-5124) ao `MachineScheduler` (fatias de rendering por scanline, VBlank IRQ, Line IRQ, sprites e CRAM).
- **Task SMS-2.2 (`sega-video`):** Adicionar a opção de desabilitar a trava física de limite de sprites por linha (Default: limite mantido conforme hardware real).
- **Task SMS-2.3 (`sega-audio` + `sega-scheduler`):** Conectar o gerador de som PSG SN76489 ao `MachineScheduler` com amostragem temporizada em tempo real.
- **Task SMS-2.4 (`sega-core`):** Implementar o codec de Save State de 6 slots com verificação CRC.
- **Gate SMS:** Conclusão funcional de SMS em *host-null* e janela Rust com 100% de testes verdes.

---


### Sub-rota transversal — fechamento semântico do Z80 compartilhado

A CPU Z80 permanece única em `sega-cpus` e compartilhada por SMS, SG-1000, Game Gear e subsistema de áudio do Mega Drive. O fechamento semântico segue a sequência E1–E9, sem duplicar o core por console.

| Fatia | Escopo | Estado em 2026-11-03 |
|---|---|---|
| E1–E2 | Matriz semântica e FSM DD/FD, last-prefix-wins, snapshots | Integradas anteriormente |
| E3A1 | Operações 16-bit IX/IY | Integrada na main |
| E3A2 | Transferências absolutas, PUSH/POP e EX (SP),IX/IY | Integrada na main pela PR #470 |
| E4A | LD r,(IX/IY+d), LD (IX/IY+d),r e LD (IX/IY+d),n | IMPLEMENTED / SHORT_PASS; integrada na main pela PR #471 (`e9a3eff6`) |
| E4B | ADD/ADC/SUB/SBC/AND/XOR/OR/CP (IX/IY+d) | IMPLEMENTED / SHORT_PASS no candidato PR #472; CI run 41738004725, 4/4 PASS |

| E4C | INC/DEC (IX/IY+d) | IMPLEMENTED / QUICK_PASS / SHORT_PASS no SHA `bac497462a3fc13a1f47892d25d435beefddd6d8`; CI `41742682599` 4/4 PASS; 23T/6 M-cycles conforme ZiLOG |
| E5 | DDCB/FDCB | Pendente |
| E6–E9 | Prefixos ignorados, fechamento semântico, validação formal e integração temporal | Pendente |

**Limite de aceitação:** E4A/E4B estão integradas; E4C passou QUICK_PASS/SHORT_PASS no escopo dirigido e aguarda integração pela PR #473. Nenhuma das três etapas tem LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS. ZEXDOC/ZEXALL e a integração temporal sistêmica do Z80 continuam pendentes.

### FASE P2 — SG-1000 (I & II) — *Reaproveitamento Integrado*
**Foco:** Ativação do SG-1000 utilizando os mesmos crates de CPU Z80 (`sega-cpus`), PSG (`sega-audio`), Scheduler (`sega-scheduler`) e Hosts (`sega-hosts`).

#### Sprint SG-1: Microsequenciador e VDP TMS9918A
- **Task SG-1.1 (`sega-core`):** Ligar o microsequenciador Z80 compartilhado ao `Sg1000TemporalBus` (integrado no SHA `9c9b6dcad052193b81de33845b6bf94bd5f927c0`).
- **Task SG-1.2 (`sega-video` + `sega-scheduler`):** Implementar os VDPs TMS9918A (NTSC ~59,92 Hz) e TMS9929 (PAL ~49,70 Hz) dentro de `sega-video` sob governança do `MachineScheduler`.
- **Task SG-1.3 (`sega-core`):** Mapear as expansões de RAM (1 KiB, 2 KiB e 8 KiB) e mídias SG-1000.
- **Task SG-1.4 (`sega-hosts`):** Conectar o botão físico **RESET/SW** e joysticks SG-1000 no crate de entrada SDL3.
- **Gate SG-1000:** SG-1000 I e II operacionais via crates compartilhados sem regressões no Master System.

---

### FASE P3 — Game Gear (GG) — *Extensão da Stack SMS*
**Foco:** Habilitar o Game Gear como extensão direta da engine SMS VDP e PSG.

#### Sprint GG-1: Viewport, Paleta e Panning
- **Task GG-1.1 (`sega-video`):** Implementar o modo de exibição Game Gear em `sega-video` (crop visível 160×144 a partir da matriz interna de 256×192).
- **Task GG-1.2 (`sega-video`):** Implementar o suporte à paleta expandida de 4096 cores (RAM de CRAM expandida).
- **Task GG-1.3 (`sega-audio`):** Habilitar registradores de panning estéreo e mixagem de canais L/R para o PSG SN76489 em `sega-audio`.
- **Task GG-1.4 (`sega-core`):** Suporte ao registrador de pino regional de hardware (Japão / Exportação).
- **Gate Game Gear:** Jogos de Game Gear executando com os mesmos crates centrais do SMS.

---

### FASE P4 — Mega Drive / Genesis (Model 1 & Model 2)
**Foco:** Expansão da arquitetura para suportar arquiteturas de múltiplos processadores (M68000 + Z80).

#### Sprint MD-1: Motorola 68000 e Barramento de Memória
- **Task MD-1.1 (`sega-cpus`):** Implementar o núcleo transacional da CPU Motorola 68000 microsequenciada em `sega-cpus`.
- **Task MD-1.2 (`sega-core` + `sega-scheduler`):** Construir o barramento de memória transacional do Mega Drive coordenado pelo `MachineScheduler` com múltiplos domínios de relógio (M68000 a ~7,67 MHz NTSC / ~7,60 MHz PAL e Z80 a ~3,58 MHz).
- **Task MD-1.3 (`sega-cpus`):** Reconectar o Z80 compartilhado de `sega-cpus` para atuar como co-processador de áudio do Mega Drive.

#### Sprint MD-2: Genesis VDP 315-5313 e YM2612
- **Task MD-2.1 (`sega-video` + `sega-scheduler`):** Implementar o VDP Genesis 315-5313 em `sega-video` com suporte aos modos H32 (256 px), H40 (320 px), interlaced (448i/480i) e interrupções sob o `MachineScheduler`.
- **Task MD-2.2 (`sega-audio` + `sega-scheduler`):** Implementar o gerador FM YM2612 em `sega-audio` temporizado pelo Scheduler, operando em conjunto com o PSG SN76489 existente.
- **Task MD-2.3 (`sega-hosts`):** Suporte aos periféricos do Mega Drive (Controles de 3 e 6 botões) via SDL3.
- **Gate Mega Drive:** Execução fluida de ROMs comercial/homebrew de Mega Drive I e II em Rust.

---

### FASE P5 — Sega CD / Mega-CD (Model 1 & Model 2)
**Foco:** Acoplamento da unidade complementar de CD-ROM ao ecossistema Mega Drive.

#### Sprint CD-1: Sub-CPU 68000, ASIC Gráfico e RF5C164
- **Task CD-1.1 (`sega-cpus`):** Instanciar a segunda CPU Motorola 68000 em `sega-cpus` para atuar como Sub-CPU do Sega CD a ~12,5 MHz.
- **Task CD-1.2 (`sega-core` + `sega-scheduler`):** Implementar o barramento de comunicação e compartilhamento de RAM (PRG-RAM 512 KB, Word-RAM 256 KB) entre Main-CPU e Sub-CPU sob o `MachineScheduler`.
- **Task CD-1.3 (`sega-core` + `sega-video`):** Implementar o ASIC de rotação, escala e gravação de vídeo do Sega CD.
- **Task CD-1.4 (`sega-audio`):** Implementar o chip de som PCM Ricoh RF5C164 em `sega-audio`.
- **Task CD-1.5 (`sega-media`):** Suporte à leitura de mídias de CD-ROM (CUE/BIN, ISO) e verificação regional de BIOS de Sega CD.
- **Gate Sega CD:** Boot e execução de jogos em disco no Sega CD I e II.

---

### FASE P6 — Sega 32X & Sega CD 32X
**Foco:** Integração dos co-processadores de 32 bits Hitachi SH-2 e composição avançada de vídeo.

#### Sprint 32X-1: Dual Hitachi SH-2 e 32X VDP
- **Task 32X-1.1 (`sega-cpus`):** Implementar o núcleo de processamento RISC 32-bit Hitachi SH-2 em `sega-cpus` (instanciado duas vezes: Master SH-2 e Slave SH-2 a ~23 MHz).
- **Task 32X-1.2 (`sega-video` + `sega-scheduler`):** Implementar o Framebuffer VDP do 32X em `sega-video` e o misturador de vídeo com a saída original do Mega Drive VDP.
- **Task 32X-1.3 (`sega-audio`):** Implementar o gerador de som PWM do 32X em `sega-audio`.
- **Task 32X-1.4 (`sega-core`):** Implementar a arquitetura combinada Sega CD 32X (Mega Drive + Sub-CPU Sega CD + Dual SH-2 32X).
- **Gate 32X:** Execução de jogos de 32X e Sega CD 32X.

---

### FASE P7 — Painel Python / Qt (PySide6) e Telemetria
**Foco:** Refatorar a interface gráfica em Python para atuar unicamente como painel de controle e launcher.
- **Task P7.1 (`sega-pyo3`):** Reduzir as exportações PyO3 a métodos de ciclo de vida (`start`, `pause`, `stop`, `reset`, `load_rom`, `get_telemetry`).
- **Task P7.2 (Frontend Qt):** Desenvolver as telas de configuração no Qt para escolha de resoluções, taxas de atualização, seletores de monitor, filtros de vídeo, drivers de áudio/DSP e mapeadores de controle SDL3.
- **Gate PyO3/Qt:** A aplicação Qt atua como painel opcional; o runtime Rust é 100% capaz de executar autonomamente em modo standalone/headless.

---

### FASE P8 — Compatibilidade, Auditoria e Robustez
**Foco:** Auditoria em massa de títulos e estabilização de plataforma.
- **Task P8.1 (`sega-media`):** Executar a ferramenta de auditoria automática de jogos sobre toda a biblioteca de ROMs de todas as plataformas.
- **Task P8.2 (CI / Gates):** Validar os workflows em ambiente Windows x64 com compilação Release dos crates Rust.
- **Gate Compatibilidade:** Matriz de compatibilidade preenchida com hashes SHA-256 e sem erros de regressão.

---

### FASE P9 — Comprovação Sistêmica e Selo TEMPORAL_PASS
**Foco:** Validação formal da precisão temporal.
- **Task P9.1 (`sega-scheduler`):** Validar a cronologia causal e invariância de chunks entre execuções equivalentes sob o `MachineScheduler`.
- **Task P9.2 (`sega-core`):** Validação de determinismo em snapshots e replay em safe points da máquina.
- **Selo TEMPORAL_PASS:** Concessão formal do selo `TEMPORAL_PASS` para todo o ecossistema Sega.

---

## 4. Gates Obrigatórios de Integração (CI Workflow)

Toda e qualquer PR deve cumprir rigorosamente os seguintes comandos de validação antes do merge na `main`:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```