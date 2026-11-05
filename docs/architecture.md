# Arquitetura do Sega Emulator

O Sega Emulator busca fidelidade funcional e temporal próxima ao hardware original, com desempenho adequado e arquitetura modular. O projeto não pretende simular integralmente a engenharia elétrica, todos os pinos ou todos os estados internos dos chips.

## Diretriz normativa

A [ADR-0016 — fidelidade observável e precisão proporcional ao console](architecture/adr/0016-observable-fidelity-and-proportional-timing.md) é a decisão vigente para definir a granularidade da emulação.

- Modelar corretamente o comportamento que jogos, periféricos e componentes do console conseguem observar.
- Preservar clocks, sincronização e ordem causal nos pontos em que alteram a execução.
- Usar a menor granularidade que reproduza o comportamento: instrução, transação, scanline, amostra ou evento.
- Implementar M-cycles, T-states, WAIT, arbitragem e sinais somente quando um console suportado depender deles.
- Não simular todos os pinos, fenômenos analógicos ou detalhes elétricos sem uma necessidade demonstrada.
- Medir o custo de CPU/memória e evitar eventos e estados desnecessários.

## Responsabilidades por linguagem

### Rust

Runtime de CPU, ALU, memória, barramento, mappers, VDP, PSG/FM/PCM, interrupções, sincronização, Scheduler e estado da máquina.

### Python / Qt / PySide6

Interface, launcher, biblioteca, configurações, logs, diagnóstico, telemetria e ferramentas. Não executa nem temporiza hardware.

### PyO3

Fronteira coarse-grained para comandos de ciclo de vida, carregamento de mídia, stepping em nível de frame quando exposto pela API e telemetria. Sem callbacks por instrução/pixel/amostra.

## Componentes e plataformas

- O Z80 é compartilhado entre plataformas compatíveis; não duplicar uma CPU por console.
- SG-1000 e Master System possuem VDPs, mapas, I/O e particularidades próprios.
- Componentes de áudio podem ser compartilhados quando o hardware e os perfis forem compatíveis.
- Diferenças de revisão/ASIC só exigem caminhos específicos quando alterarem comportamento observável.

## Estado e prioridades

Consulte sempre:
- [Roadmap mestre](project-roadmap.md) — fases e sequência.
- [Estado dos componentes](component-status.md) — implementação e pendências.
- [Escopo e princípios](project-scope.md) — limites.
- [ADRs](architecture/adr/README.md) — decisões arquiteturais.
- [Documentação de hardware](hardware/) — comportamento e fontes.

## Validação

Uma funcionalidade não está concluída apenas porque compila. Exigir testes funcionais e regressões apropriados ao comportamento alterado. Testes temporais detalhados são necessários quando a interação com outro componente depender da ordem ou do instante do evento; não são um gate universal para toda instrução ou pino.

Gates de qualidade:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Cada suíte deve ser classificada PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED não equivale a PASS.
