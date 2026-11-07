# Sega Emulator — Rust + egui

Emulador de consoles Sega em desenvolvimento, construído integralmente em **Rust**. A interface desktop é nativa, utilizando **egui/eframe**. Não há frontend Python, PySide6/Qt, bindings PyO3 ou runtime alternativo.

A prioridade atual é tornar o **Sega Master System temporal** executável por meio de um único runtime, usando o Z80 compartilhado e o MachineScheduler como autoridade temporal.

## Estado atual

- `rust/runtime/sms-runtime` contém o worker SMS temporal implementado em Rust puro.
- O runtime utiliza `sega-sms-core`, `sega-audio-runtime` e `sega-video-runtime`.
- A máquina SMS temporal é executada pelo worker dedicado; a aplicação envia comandos de sessão, sem callbacks por instrução.
- A sessão de áudio física é criada por uma factory do host e executada na thread proprietária da máquina, preservando a afinidade dos objetos WASAPI/XAudio2.
- `native/sega_app` contém o shell desktop egui/eframe. A integração operacional entre a GUI, o worker SMS, o viewport D3D12, o áudio e o input nativos ainda está em andamento.
- O SMS atômico antigo foi removido do core ativo. `native/sms_core` contém a máquina temporal e fachadas para componentes compartilhados.
- A implementação temporal ainda é parcial: o VDP opera por scanline, há WAIT de fixture e ainda não há Save States persistentes completos.
- A consolidação dos crates na árvore canônica `rust/` e a integração do runtime ao shell egui continuam pendentes.
- O SG-1000 atômico antigo não é runtime alternativo nem fallback do SMS temporal.

A existência dos crates e do worker temporal não comprova que o emulador esteja pronto para gameplay real. Compatibilidade, fidelidade e desempenho só devem ser declarados após validação reproduzível.

## Arquitetura

O produto é integralmente Rust:

- **Hardware e runtime:** CPU Z80 compartilhada, barramento, memória, mapper, I/O, VDP, PSG, Scheduler, áudio, vídeo e input.
- **Aplicação desktop:** Rust com egui/eframe, responsável pela apresentação, menus e comandos de alto nível.
- **Hosts nativos:** integração com D3D12, WASAPI/XAudio2 e dispositivos de entrada no ambiente Windows.
- **Execução:** o worker SMS é proprietário da máquina e do Scheduler. A GUI não executa instruções, avança clocks nem controla eventos de hardware.
- **Comunicação:** comandos coarse-grained de ciclo de vida e sessão; nunca callbacks por instrução.

Não fazem parte da arquitetura: Python, PySide6, PyQt, Qt, PyO3, Maturin ou um frontend legado. Não há fallback para os cores atômicos antigos.

São proibidos JIT, frame skipping, hacks temporais e otimizações especulativas.

Consulte as decisões normativas:
- [ADR 0010 — Produto 100% Rust e SMS temporal](docs/adr/0010-rust-only-sms-temporal-first.md)
- [ADR-0016 — Fidelidade observável e temporização proporcional](docs/architecture/adr/0016-observable-fidelity-and-proportional-timing.md)
- [Roadmap mestre](docs/project-roadmap.md)

## Ambiente de desenvolvimento

- Rust toolchain definida em `rust-toolchain.toml` (atualmente 1.99.1).
- Git.
- Windows x64 e Visual Studio Build Tools com MSVC para validar os targets e hosts nativos Windows, incluindo D3D12, WASAPI e XAudio2.
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

Enquanto existirem workspaces standalone e targets específicos, eles também devem ser validados separadamente, incluindo compilação Windows/D3D12 e hosts WASAPI/XAudio2. Testes físicos não executados devem ser classificados como **NOT RUN** ou **BLOCKED**, nunca como PASS.

Os testes externos ZEXDOC/ZEXALL estão **NOT RUN**: o executor Rust de baixo nível existe, mas a aquisição, montagem e orquestração da suíte ainda precisam ser portadas para Rust. A ausência dessas suítes não equivale a aprovação.

## Segurança e licença

Use somente ROMs e firmware que você tenha direito de executar. ROMs comerciais, BIOS proprietárias e dumps protegidos não devem ser versionados.

O projeto é distribuído sob a licença MIT. Consulte [LICENSE](LICENSE). Sega e nomes de consoles/chips são marcas de seus respectivos titulares; o projeto é independente e não afiliado à SEGA.
