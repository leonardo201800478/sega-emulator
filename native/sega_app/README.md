# sega-app — M1

Primeiro executável nativo Rust do Sega Emulator, usando egui/eframe 0.37.0.

## Build (Windows x64)

```powershell
cargo fmt --all --manifest-path native/sega_app/Cargo.toml -- --check
cargo check --locked --manifest-path native/sega_app/Cargo.toml --target x86_64-pc-windows-msvc
cargo test --locked --manifest-path native/sega_app/Cargo.toml --target x86_64-pc-windows-msvc
cargo run --release --locked --manifest-path native/sega_app/Cargo.toml --target x86_64-pc-windows-msvc
```

## Limites desta fatia

- A janela comprova somente a inicialização do eframe.
- Nenhuma ROM, CPU, Scheduler, áudio ou input é iniciado.
- O viewport D3D12 ainda não foi acoplado ao shell.
- O frontend Python permanece disponível como baseline/rollback.
- O crate tem workspace e lockfile próprios nesta etapa; a consolidação com o workspace raiz fica para a fase de integração de crates.

A janela não declara paridade funcional nem substitui o frontend legado.
