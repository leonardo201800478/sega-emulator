# ADR 0008 — Fronteira rígida: hardware em Rust, aplicação Qt em Python

- **Status:** aprovado para migração incremental.
- **Data:** 2026-10-26.
- **Escopo inicial:** SG-1000; não expandir para outros consoles antes da aceitação deste marco.

## Decisão

1. Todo comportamento de hardware em produção deve residir em Rust puro: CPU Z80/ALU, barramentos, memória, mapeamento de cartuchos, I/O, VDP, PSG, temporização, interrupções, scheduler e runtime da máquina.
2. Python ativo na aplicação fica restrito à interface Qt e backends de host, seleção/carregamento de ROMs e perfis, configuração/logging e uma fachada fina para PyO3. Ferramentas e testes Python são infraestrutura e não podem executar implementações de hardware em Python.
3. PyO3/Maturin formam uma fronteira fina. O caminho crítico não deve executar callbacks Python por instrução/operação de hardware nem copiar grandes buffers repetidamente.
4. Código Python que implementava hardware foi movido para `legacy/python_reference/` como consulta histórica, fora do pacote executável e de imports/testes/workflows ativos. É proibido reativá-lo como runtime ou oráculo.
5. Para futuras mudanças, mapear imports, contratos, consumidores e validação antes de editar. Hardware novo nasce em Rust; regressões e testes ativos exercitam o runtime Rust. Arquivo legado permanece inerte.
6. Código legado arquivado é somente referência: não deve ser importado pelo runtime, empacotado como módulo ativo nem tratado como fonte oficial. Preservar o histórico Git e a proveniência dos arquivos.
7. Não declarar o SG-1000 completo até que CPU, memória/barramento, I/O, VDP, PSG e scheduler estejam integrados e testados. Não declarar precisão ciclo a ciclo sem modelo temporal e evidências correspondentes.

## Inventário inicial confirmado no repositório

- Rust puro inicial: `native/sg1000_core/src/{bus,cpu,machine,memory}.rs` e `native/z80_alu/src/lib.rs`.
- Adaptador/extensão PyO3 ativo: `native/z80_core/src/machine.rs` e a fronteira em `lib.rs`; `memory.rs` e `lib.rs` ainda mantêm um harness Rust Z80 legado para testes diferenciais, não usado pela máquina integrada. O VDP/PSG ativos vêm exclusivamente de `native/sg1000_core`.
- Implementações Python de hardware arquivadas: `legacy/python_reference/src/sega_emulator/{cpu,bus,video,audio,machine}/`. `machine/cartridge_profiles.py` e `machine/sg1000.py` permanecem ativos apenas para resolução de perfil e fachada PyO3, sem lógica de hardware.
- `audio/output_config.py` e `audio/vgm_recorder.py` permanecem ativos como integração de aplicação/Qt. `audio/synchronization.py` foi arquivado com o runtime Python anterior.

Este inventário é inicial, não uma declaração de que cada arquivo está isolado ou pode ser movido sem refatoração. A lista completa de importadores e testes deve ser produzida por fase.

## Ordem de migração e aceitação

1. Auditar dependências, contratos e testes de CPU, ALU, barramento e memória.
2. Completar e validar Z80 em Rust puro com vetores independentes, estados/flags, instruções, prefixos, interrupções e temporização suportada.
3. **Concluído para o SG-1000:** mapa de memória, cartucho e I/O são delegados ao núcleo Rust via fachada PyO3.
4. Integrar VDP TMS9918A, PSG SN76489, temporização e scheduler determinístico.
5. **Concluído para o SG-1000:** aplicação Qt usa a fachada PyO3 e não possui caminho ativo de hardware Python.
6. Arquivar implementações Python substituídas em `legacy/python_reference/` e confirmar que nenhum módulo de produção as importa.
7. Executar `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, build Release/Maturin e testes de integração relevantes; validar workflows no SHA exato.
8. Atualizar README, matriz de cobertura e limitações antes de marcar cada fatia concluída.

## Restrições

- Não remover a interface Qt nem as ferramentas Python.
- Não mover código ativo apenas para satisfazer a organização de diretórios.
- Não aceitar hashes iguais como prova absoluta de correção.
- Não introduzir JIT ou paralelismo especulativo.
- Não fazer merge sem revisão dos diffs e dos checks exigidos.
