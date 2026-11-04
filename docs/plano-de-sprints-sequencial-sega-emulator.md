# Plano de Sprints Sequencial por Plataforma — Sega Emulator

**Status:** Rota oficial de implementação progressiva.  
**Estratégia de Engenharia (MÓDULOS COMPARTILHADOS):** Toda e qualquer integração deve ser realizada exclusivamente nos **módulos e crates compartilhados** do workspace Rust (`sega-core`, `sega-cpus`, `sega-video`, `sega-audio`, `sega-scheduler`, `sega-hosts`). É estritamente proibido criar implementações individuais, duplicadas ou isoladas por console. Cada nova plataforma adicionada deve reaproveitar, estender ou parametrizar os componentes centrais existentes.  
**Baseline atual:** SHA `f89a52346e91e88e82ab37a4afcfffff2d793407` (`main`, após PR #475 / E4C).  
**Última fatia Z80 integrada:** PR #475 / E4C, HEAD validado `8f04b44e6c6ba85920a7438b74c9876586d5736c`, CI 41747460761 4/4 PASS.

---

## Diretriz Geral de Reutilização em Crates Compartilhados

1. **Processadores (`sega-cpus`):** Os núcleos de CPU (Z80 microsequenciado, Motorola 68000, Hitachi SH-2) são únicos e compartilhados entre todos os consoles. O mesmo Z80 executa SMS, SG-1000, Game Gear e o áudio do Mega Drive.
2. **Vídeo (`sega-video`):** As famílias de VDP (TMS9918A/9929, SMS VDP 315-5124, Genesis VDP 315-5313, 32X VDP) são construídas como engines reutilizáveis com parâmetros de revisão e modo.
3. **Áudio (`sega-audio`):** Os chips digitais de áudio (PSG SN76489, YM2413, YM2612, RF5C164, PWM) residem em `sega-audio` e são instanciados conforme a arquitetura da máquina.
4. **Temporização & Barramentos (`sega-scheduler` e `sega-core`):** O `MachineScheduler` e as traits de `BusTransaction`/`BusOperationResult` são a abstração única para sincronização temporal de todas as plataformas.
5. **Backends de Host (`sega-hosts`):** DirectX 12 Render, WASAPI/XAudio2 + DSP e SDL3 Input são compartilhados universalmente.

---

## Ordem Cronológica de Execução Sequencial

1. **Sprint 1 & 2:** Master System (SMS I / SMS II / Mark III) — *Base temporal dos módulos compartilhados Z80, SMS VDP, PSG e Bus Transacional.*
2. **Sprint 3:** SG-1000 (SG-1000 I / SG-1000 II) — *Reaproveitamento direto dos crates de CPU Z80, PSG, Scheduler e inclusão do VDP TMS9918A em `sega-video`.*
3. **Sprint 4:** Game Gear (GG) — *Reaproveitamento total da stack SMS com adição de viewport/paleta no crate `sega-video`.*
4. **Sprint 5 & 6:** Mega Drive (MD I / MD II / Genesis) — *Inclusão do M68000 em `sega-cpus`, Genesis VDP em `sega-video` e YM2612 em `sega-audio`.*
5. **Sprint 7:** Sega CD / Mega-CD (Model 1 & 2) — *Extensão da stack Mega Drive com segundo M68000, CD Controller e RF5C164.*
6. **Sprint 8:** Sega 32X / Super 32X & Sega CD 32X — *Inclusão do Hitachi SH-2 em `sega-cpus`, VDP 32X e PWM Audio.*

---

## Sprint 1 & 2: Master System (SMS I & II) — Foco Ativo

**Objetivo:** Concluir a integração vertical temporizada ponta a ponta do Master System utilizando os **crates compartilhados**, servindo como fundação para os sistemas subsequentes.

### Tasks Executáveis (SMS em Crates Compartilhados)

* [ ] **SMS-01 (`sega-cpus` + `sega-core`):** Conectar a CPU Z80 microsequenciada compartilhada ao barramento temporal SMS (`SmsTemporalBus`) via `BusTransaction`.
* [ ] **SMS-02 (`sega-video` + `sega-scheduler`):** Conectar a engine SMS VDP (315-5124) ao `MachineScheduler` (fatias de scanline/fetch e interrupções VBlank/Line IRQ).
* [ ] **SMS-03 (`sega-audio` + `sega-scheduler`):** Conectar o gerador PSG SN76489 compartilhado ao relógio e scheduler do SMS com amostragem temporizada.
* [ ] **SMS-04 (`sega-core`):** Finalizar suporte aos mappers compartilhados SMS ($FFFD-$FFFF) e cartuchos Sega Card/50-pin.
* [ ] **SMS-05 (`sega-hosts`):** Validar entradas de periféricos (Joystick SMS e Pause Button) no sistema de input SDL3 compartilhado.
* [ ] **SMS-06 (`sega-core`):** Executar suíte de testes determinísticos e Golden Masters com ROMs comerciais/homebrew de SMS.


### Sub-sprint transversal Z80 — fechamento do core compartilhado

Esta sub-rota evolui o mesmo microsequenciador em `sega-cpus`; não cria implementação Z80 específica do SMS.

- [x] **E3A1:** operações 16-bit IX/IY integradas na main.
- [x] **E3A2:** LD absoluto, PUSH/POP e EX (SP),IX/IY integrados na main pela PR #470.
- [x] **E4A (PR #471 integrada na main):** LD r,(IX/IY+d), LD (IX/IY+d),r e LD (IX/IY+d),n; displacement assinado, wrap de 16 bits, exceção H/L, WAIT, snapshot/restore e prefixos DD/FD repetidos/alternados.
- [x] **CI do candidato E4A:** run 41734982519 — Rust, Python, Scheduler stress short e Windows x64 — 4/4 PASS.
- [x] **E4B (candidato PR #472):** ADD, ADC, SUB, SBC, AND, XOR, OR e CP (IX/IY+d), ALU compartilhada, flags S/Z/H/PV/N/C e X/Y, CP preserva A e copia X/Y do operando, WAIT, snapshot/restore e last-prefix-wins.
- [x] **CI do candidato E4B:** run 41738004725 — Rust, Python, Scheduler stress short e Windows x64 — 4/4 PASS.
- [x] **E4C (PR #475 integrada):** INC/DEC (IX/IY+d), read-modify-write compartilhado, flags S/Z/H/PV/N, C preservado, WAIT, snapshots, wrap, prefixos repetidos/alternados; 5 testes dirigidos.
- [x] **CI E4C:** run 41747460761 — Rust, Python, Scheduler stress e Windows x64 — 4/4 PASS.
- [ ] **E5:** DDCB/FDCB completo no microsequenciador.
- [ ] **E6–E9:** prefixos ignorados, fechamento semântico, ZEXDOC/ZEXALL e integração temporal.

**Estado:** E4A, E4B e E4C integradas na main. E4 está implementada e aprovada nos gates dirigidos, com E4C em QUICK_PASS / SHORT_PASS. Próxima tarefa: E5 DDCB/FDCB. Validação diferencial externa, ZEXDOC/ZEXALL, LONG_PASS e TEMPORAL_PASS permanecem pendentes.

**Critério de Conclusão da Sprint SMS:** Boot e gameplay funcional de SMS via módulos compartilhados, em *host-null* e janela Rust nativa sem dependência de Python.

---

## Sprint 3: SG-1000 (I & II)

**Objetivo:** Conectar a máquina SG-1000 reutilizando integralmente o Z80 (`sega-cpus`), PSG (`sega-audio`), Scheduler (`sega-scheduler`) e Hosts (`sega-hosts`) desenvolvidos na Sprint SMS.

### Tasks Executáveis (SG-1000)

* [ ] **SG-01 (`sega-core`):** Conectar o Z80 microsequenciado compartilhado ao `Sg1000TemporalBus` (integrado na PR #423).
* [ ] **SG-02 (`sega-video`):** Adicionar o VDP TMS9918A / TMS9929 ao crate `sega-video` e conectar ao `MachineScheduler` (NTSC ~59.92 Hz / PAL ~49.70 Hz).
* [ ] **SG-03 (`sega-hosts`):** Mapear o botão **RESET/SW** e joysticks SG-1000 no crate de input SDL3.
* [ ] **SG-04 (`sega-core`):** Validar mídias e tamanhos de RAM no barramento temporário compartilhado (1 KiB, 2 KiB e 8 KiB).

**Critério de Conclusão:** SG-1000 I e II funcionais reaproveitando os crates compartilhados com regressão limpa em relação ao SMS.

---

## Sprint 4: Game Gear (GG)

**Objetivo:** Estender os crates compartilhados de SMS para suportar o Game Gear sem criar um emulador isolado.

### Tasks Executáveis (GG)

* [ ] **GG-01 (`sega-video`):** Adicionar o modo de viewport 160×144 (crop da matriz 256×192) dentro da engine SMS VDP em `sega-video`.
* [ ] **GG-02 (`sega-video` + `sega-core`):** Suporte à paleta estendida de 4096 cores e pino de região de hardware no barramento compartilhado.
* [ ] **GG-03 (`sega-audio`):** Habilitar panning/mixagem estéreo no PSG SN76489 em `sega-audio`.

**Critério de Conclusão:** ROMs de Game Gear executando com os mesmos crates do SMS, alternando apenas o perfil de vídeo/áudio do hardware.

---

## Sprints 5 a 8: Resumo das Etapas Futuras (Congeladas)

### Sprint 5 & 6 — Mega Drive (MD I & II)
* Adição do Motorola 68000 em `sega-cpus`, Genesis VDP em `sega-video`, YM2612 em `sega-audio` e reutilização do Z80 (`sega-cpus`) + PSG (`sega-audio`) para o subsistema de áudio.

### Sprint 7 — Sega CD
* Adição do segundo M68000 em `sega-cpus`, controlador de CD-ROM e ASIC gráfico em `sega-core`, e RF5C164 em `sega-audio`.

### Sprint 8 — Sega 32X & Sega CD 32X
* Adição do núcleo Hitachi SH-2 em `sega-cpus`, VDP 32X em `sega-video` e PWM Audio em `sega-audio`.

---

## Atualização de execução — 2026-11-04 (prevalece sobre o status antigo da sub-sprint Z80)

**Baseline main:** `644b2c22879d63447f63b730ed9415db11e81fab`; CI pós-merge #41831738966 SUCCESS.

### Sub-sprint Z80 — status confirmado

- [x] E3A1/E3A2 e E4A/E4B/E4C — integradas.
- [x] E5 DDCB/FDCB — PR #485 integrada.
- [x] E6 resolução geral de prefixos — PR #486 integrada.
- [x] E7.1 HALT/M1 — PR #490 integrada.
- [x] E7.2A NMI — PR #495 integrada.
- [x] E7.2B INT acknowledge — PR #496 integrada.
- [x] E7.2C IM0/IM1/IM2 — PR #498 integrada.
- [x] E7.2D EI/DI — PR #500 integrada.
- [x] E7.2E incremento de snapshot/trace — PR #504 integrada; restore/continuação das 70 fases geradas pelo corpus — PR #505 integrada.
- [ ] E7.2E matriz ampliada de estados inválidos e WAIT — PR #506 aberta, CI pendente no HEAD consultado.
- [ ] E7.3 HALT T4 e `EI;HALT;INT`.
- [ ] E7.4 BUSREQ/arbitragem real.
- [ ] E7.5 RETI/daisy-chain.
- [ ] E7.6 IRQ real VDP SMS.
- [ ] E7.7 auditoria semântica integral.

**Estado:** E7 global `INTEGRATION_PENDING`. Não iniciar E8 até fechar E7. Scheduler stress SKIPPED não conta como PASS. A rota Z80 é transversal e continua compartilhada; não criar um core por console.
