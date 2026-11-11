# Sega Emulator — Rust / SMS temporal

Emulador de consoles Sega em desenvolvimento, construído integralmente em **Rust**. O runtime temporal e o shell atualmente integrados na `main` são preservados como baseline. A arquitetura aprovada para a próxima aplicação desktop é **Slint + WGPU + CPAL**, conforme a [ADR 0011](docs/adr/0011-slint-wgpu-cpal-desktop-platform.md); a POC ainda não foi implementada nem integrada. Não há frontend Python, PySide6/Qt, bindings PyO3 ou runtime alternativo.

A prioridade de hardware continua sendo o **Sega Master System temporal**, com Z80 compartilhado e MachineScheduler como autoridade temporal. A replataforma do host não autoriza reescrita de CPU, VDP, PSG, barramentos ou Scheduler.

## Estado atual

- `rust/runtime/sms-runtime` contém o worker SMS temporal implementado em Rust puro.
- O runtime utiliza `sega-sms-core`, `sega-audio-runtime` e `sega-video-runtime`.
- A máquina SMS temporal é executada pelo worker dedicado; a aplicação envia comandos de sessão, sem callbacks por instrução.
- A sessão de áudio física usa a camada host WASAPI/XAudio2 atualmente integrada.
- `native/sega_app` contém o shell desktop egui/eframe atualmente integrado. As PRs #613 e #614 foram mescladas; o runtime SMS, viewport D3D12, áudio e input nativos fazem parte da baseline `main`.
- O SMS atômico antigo foi removido do core ativo. `native/sms_core` contém a máquina temporal e fachadas para componentes compartilhados.
- A implementação temporal ainda é parcial: o VDP opera por scanline, há WAIT de fixture e ainda não há Save States persistentes completos.
- O SG-1000 atômico antigo não é runtime alternativo nem fallback do SMS temporal.
- A PR #615 integrou melhoria de filtragem de saída SN76489 e fidelidade do reset do ruído.

A existência dos crates e do worker temporal não comprova que o emulador esteja pronto para gameplay real. Compatibilidade, fidelidade e desempenho só devem ser declarados após validação reproduzível.

**Baseline auditada:** `main` SHA `392a5c67e85a11a5a980d8f7055bcc8a12fe3784` (2026-11-10).  
**Migração desktop:** [ADR 0011](docs/adr/0011-slint-wgpu-cpal-desktop-platform.md) e [PR #745](https://github.com/leonardo201800478/sega-emulator/pull/745), ainda Draft e não mesclada.  
**Fotografia histórica:** [status do projeto em 2026-11-07](docs/project-status-2026-11-07.md).  
**Mapa de documentação:** [docs/README.md](docs/README.md).  
**Evidência do stress longo do Scheduler:** [registro de 2026-11-07](docs/evidence/scheduler-stress-long-2026-11-07.md).

## Arquitetura

O produto é integralmente Rust:

- **Hardware e runtime:** CPU Z80 compartilhada, barramento, memória, mapper, I/O, VDP, PSG, Scheduler, áudio, vídeo e input.
- **Aplicação atualmente integrada na main:** egui/eframe, responsável pela apresentação, menus e comandos de alto nível; permanece como baseline durante a migração.
- **Aplicação desktop alvo:** Slint 1.18.x.
- **Apresentação alvo:** WGPU 30.x como API gráfica única, Vulkan como backend preferencial inicial no Windows x64.
- **Saída de áudio alvo:** CPAL 0.18.x; a migração ocorrerá após o gate gráfico R1.
- **Hosts atuais:** D3D12 e WASAPI/XAudio2 permanecem na baseline até paridade e validação da nova stack.
- **Execução:** o worker SMS é proprietário da máquina e do Scheduler. A GUI não executa instruções, avança clocks nem controla eventos de hardware.
- **Comunicação:** comandos coarse-grained de ciclo de vida e sessão; nunca callbacks por instrução.

Não fazem parte da arquitetura: Python, PySide6, PyQt, Qt, PyO3 ou Maturin. egui/eframe é a baseline temporária, não o destino da nova aplicação. Não há fallback para os cores atômicos antigos.

São proibidos JIT, frame skipping, hacks temporais e otimizações especulativas.

Consulte as decisões normativas:
- [ADR 0010 — Produto 100% Rust e SMS temporal](docs/adr/0010-rust-only-sms-temporal-first.md)
- [ADR-0016 — Fidelidade observável e temporização proporcional](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md)
- [Roadmap mestre](docs/project-roadmap.md)

## Ambiente de desenvolvimento

- Rust toolchain definida em `rust-toolchain.toml` (atualmente 1.99.1).
- Git.
- Windows x64 e Visual Studio Build Tools com MSVC para validar a baseline nativa e, durante a migração, o caminho Slint/WGPU/Vulkan.
- PowerShell para os scripts de automação e auditoria do repositório.

**Não é necessário instalar Python, Qt, PyO3, Maturin ou ferramentas de testes Python.**

## Compilar e validar

Na raiz do repositório, em PowerShell:

```powershell
.\scripts\compile-fast.ps1 -Release -AllWorkspaces
.\scripts\project.ps1 audit
.\scripts\project.ps1 locks
.\scripts\project.ps1 validate
```

Gates Rust obrigatórios:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Enquanto existirem workspaces standalone e targets específicos, eles também devem ser validados separadamente. A baseline atual ainda exige regressão dos hosts D3D12/WASAPI/XAudio2; a nova stack exigirá build Windows x64 Release e testes Vulkan/WGPU e CPAL nas fases correspondentes. Testes físicos não executados devem ser classificados como **NOT RUN** ou **BLOCKED**, nunca como PASS.

Os testes externos ZEXDOC/ZEXALL estão **NOT RUN**: o executor Rust de baixo nível existe, mas a aquisição, montagem e orquestração da suíte ainda precisam ser portadas para Rust. A ausência dessas suítes não equivale a aprovação.

## Segurança e licença

Use somente ROMs e firmware que você tenha direito de executar. ROMs comerciais, BIOS proprietárias e dumps protegidos não devem ser versionados.

O projeto é distribuído sob a licença MIT. Consulte [LICENSE](LICENSE). Sega e nomes de consoles/chips são marcas de seus respectivos titulares; o projeto é independente e não afiliado à SEGA.
