# Migração do runtime SG-1000 para Rust — estado vigente

> Atualizado em 2026-10-27. A migração do hardware SG-1000 para Rust está implementada no candidato da PR #139, ainda aberto/em revisão. A documentação anterior de fases e benchmarks foi condensada aqui para não apresentar o runtime Python histórico como estado atual.

## Decisão arquitetural

- O único runtime ativo de hardware SG-1000 é `native/sg1000_core`.
- `native/z80_core` fornece a fronteira PyO3/Maturin e o adaptador fino `NativeSG1000Machine`. O harness Rust `RustZ80Cpu` separado permanece isolado e fora da máquina integrada. Os testes Python diferenciais que o exercitavam foram arquivados; removê-lo é uma limpeza futura separada.
- `src/sega_emulator/machine/sg1000.py` resolve o perfil de cartucho e encaminha chamadas para a extensão. Não contém CPU, memória, VDP, PSG ou scheduler em Python, nem oferece seleção/fallback de backend.
- Python ativo na aplicação fica restrito à interface Qt e backends de host, carregamento de ROMs/perfis, configuração/logging e integração fina. O frontend Pygame, runner headless, implementação Python de hardware, testes que dependiam dela e ferramentas relacionadas estão em `legacy/`.
- Testes Python ativos são testes de aplicação/integração Qt/PyO3. A validação de hardware é feita pelos testes Rust e por oráculos independentes; o legado Python não é importado como runtime nem como oráculo ativo.

## Componentes ativos em Rust

1. CPU Z80 e ALU.
2. Memória, ROM espelhada, RAM interna e RAM de cartucho conforme perfil.
3. Barramento e I/O, incluindo leitura de controles e writes temporizados ao PSG.
4. VDP TMS9918A, VRAM, registradores, raster/VBlank, status e renderização.
5. PSG SN76489AN, geração digital de amostras e fila de eventos.
6. Máquina SG-1000 e coordenação do avanço de instruções/quadros.

## Validação obrigatória

No mesmo SHA candidato, executar os workflows ativos: Rust SG-1000 Workspace, SG-1000 Three Agent Sequential Validation, Qt Frontend Rebuild Validation, Qt Rust SG-1000 runtime integration, UI and Controller Input Validation, Agent 1 Functional Validation, Agent 2 Integrity and Determinism e CI. Checks pulados não contam como aprovados.

Comandos locais de referência:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m ruff format --check .
python -m ruff check .
python -m pytest -q
```

Para a extensão, verificar também `native/z80_core/Cargo.toml`, build Release/Maturin e testes de integração após instalar o wheel. Não executar nem reativar arquivos de `legacy/`.

## Limitações e próximo passo

- O avanço temporal atual é baseado em fronteiras de instrução e T-states nominais, não em um modelo de barramento completo ciclo a ciclo.
- A integração deve ser considerada validada somente quando os workflows terminarem com sucesso no SHA candidato.
- A jogabilidade das oito ROMs da issue #136 e a comparação visual/sonora com ARES ainda precisam de evidência registrada. Não declarar compatibilidade universal com base apenas em compilação, testes sintéticos ou hashes.
