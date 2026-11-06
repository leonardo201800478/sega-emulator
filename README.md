# Sega Emulator — Rust-only

Emulador Sega em desenvolvimento, com runtime e aplicação nativos em Rust. A prioridade atual é o Master System temporal, com um único Z80 compartilhado e o MachineScheduler como autoridade temporal.

## Estado real

- rust/runtime/sms-runtime contém o worker SMS temporal extraído para Rust puro.
- O runtime usa sega-sms-core, sega-audio-runtime e sega-video-runtime; não depende de Python, PyO3, Qt ou Maturin.
- A sessão de áudio física é criada por uma factory fornecida pelo host e executada na thread proprietária da máquina, preservando a afinidade dos objetos WASAPI/XAudio2.
- native/sega_app ainda é somente um shell egui/eframe. A integração operacional entre GUI, worker SMS, viewport D3D12, áudio e input permanece pendente.
- A migração dos crates para a raiz canônica rust/ e a separação dos módulos SMS temporais/atômicos ainda estão em andamento.
- O legado Python/PyO3 foi removido desta branch em vez de portado; não haverá fallback Python nem runtime alternativo.

A existência do worker e dos crates temporais não comprova ainda que o emulador esteja pronto para gameplay real. Não declarar FPS, compatibilidade ou fidelidade final sem benchmarks e validação reproduzível.

## Arquitetura normativa

- Backend integralmente Rust: CPU Z80 compartilhada, barramento, memória, mapper, VDP, PSG, Scheduler, áudio, input e lifecycle.
- GUI egui/eframe: apresentação e comandos coarse-grained; não executa instruções, clocks ou eventos de hardware.
- SmsTemporalWorker possui a máquina em uma thread dedicada. O host envia comandos de sessão, não callbacks por instrução.
- Sem JIT, frame skipping, hacks temporais ou otimizações especulativas.
- SG-1000 atômico e SMS atômico não são fallback. O runtime ativo inicial é SMS temporal.

Consulte a ADR 0010 (docs/adr/0010-rust-only-sms-temporal-first.md), ADR-0016 e o roadmap mestre (docs/project-roadmap.md).

## Ambiente de desenvolvimento

- Windows x64 para validar D3D12, WASAPI/XAudio2 e input físico.
- Rust toolchain definida em rust-toolchain.toml (atualmente 1.99.1).
- Git e PowerShell.
- Visual Studio Build Tools com MSVC para targets Windows.
- Não é necessário instalar Python, Qt ou Maturin.

## Compilar e validar

Na raiz do repositório, no PowerShell:

```powershell
.\scripts\compile-fast.ps1 -Release -AllWorkspaces
.\scripts\project.ps1 audit
.\scripts\project.ps1 locks
.\scripts\project.ps1 validate
```

Gates Rust obrigatórios:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Workspaces standalone de áudio e aplicação ainda recebem validação separada até a consolidação final. Testes externos ZEXDOC/ZEXALL estão NOT RUN até a migração do runner para Rust; isso não equivale a PASS.

## Segurança e licença

Use somente ROMs e firmware que você tenha direito de executar. ROMs comerciais, BIOS proprietárias e dumps protegidos não devem ser versionados.

O projeto é distribuído sob a licença MIT. Consulte LICENSE. Sega e nomes de consoles/chips são marcas de seus respectivos titulares; o projeto é independente e não afiliado à SEGA.
