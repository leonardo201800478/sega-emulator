# Escopo e princípios arquiteturais — Sega Emulator

**Autoridade de escopo:** [Roadmap mestre](project-roadmap.md).
**Status ativo:** SG-1000 + Master System até o marco M4; integração temporal até P9.
**Runtime de hardware:** Rust.
**Aplicação e tooling:** Python / Qt / PySide6.
**Fronteira:** PyO3/Maturin coarse-grained.
**Plataforma prioritária:** Windows x64.

Este documento define princípios e limites arquiteturais transversais. Não define fases, prioridades, cronogramas, plataformas ativas ou sequência de implementação. Esses elementos existem somente em `docs/project-roadmap.md`.

## 1. Objetivo

Construir um emulador Sega determinístico, modular, testável e de alta fidelidade demonstrável. Preservar os runtimes SG-1000 e SMS existentes, fechar a compatibilidade funcional e evoluir para integração temporal demonstrável sem introduzir regressões não explicadas.

A prioridade é correção, evidência e integridade arquitetural. Desempenho só pode ser otimizado após baseline funcional e perfilamento.

## 2. Limites de escopo

- A única frente de implementação ativa é SG-1000 + Master System.
- Game Gear, Mega Drive/Genesis, Sega CD, 32X, SVP, Neptune e sistemas integrados de terceiros estão adiados até revisão formal após M4.
- Não criar roadmap de subsistema, cronograma paralelo ou plano de migração independente.
- Documentação de hardware futuro, quando preservada como referência, não autoriza implementação nem altera prioridade.
- JIT, frame-skip, hacks temporais e paralelismo especulativo são proibidos.
- Não versionar ROMs comerciais, BIOS proprietárias, firmware protegido ou dumps de memória protegida.

## 3. Responsabilidade por tecnologia

### 3.1 Rust — runtime único de hardware

Todo estado e comportamento digital de hardware pertence a Rust:
- CPU, ALU, registradores e interrupções;
- RAM, ROM, VRAM, barramentos, mappers e decodificação;
- VDP, raster, sprites, paletas, DMA e composição;
- PSG, FM, PCM, filtragem digital e mixagem;
- I/O, periféricos e protocolos;
- clocks emulados, T-states, WAIT, arbitragem e Scheduler;
- save states, replay e estado da máquina.

Nenhuma lógica de hardware pode existir em Python como fallback, implementação paralela ou oráculo permanente. Código em `legacy/` é referência histórica, não runtime ativo.

### 3.2 Python — aplicação e ferramentas

Python/Qt pode:
- apresentar menus, biblioteca, preferências, logs e diagnósticos;
- selecionar ROMs, perfis e recursos externos;
- enviar comandos de alto nível e consultar telemetria;
- executar ferramentas de auditoria, testes e automação.

Python não pode executar ou avançar CPU/máquina, contar ciclos, controlar barramento, processar PCM do gameplay, decidir IRQ, controlar raster ou atuar como autoridade de pacing.

### 3.3 PyO3/Maturin

A fronteira deve ser coarse-grained, com ownership e sincronização explícitos. Não usar callbacks Python por instrução, pixel ou amostra. O GIL, cópias de buffers, threads e lifecycle precisam de testes e documentação. A meta é que o runtime Rust funcione sem Python instalado.

## 4. Composição e isolamento de componentes

- Compartilhar CPU e mecanismos somente quando equivalência funcional estiver demonstrada.
- VDPs de arquiteturas distintas permanecem implementações específicas; não criar VDP universal.
- SG-1000 e SMS mantêm mapas de memória, I/O, perfis de cartucho e particularidades de VDP próprios.
- PSG SN76489 pode compartilhar implementação digital com perfis explícitos por máquina/revisão; não assumir equivalência elétrica sem evidência.
- Mappers desconhecidos, perfis ambíguos ou firmware incompatível devem produzir erro explícito, nunca fallback genérico.
- O host (Windows, GPU, dispositivo de áudio, controle) não determina o tempo emulado.

## 5. Autoridade temporal e sincronização

- Um único Master Scheduler é a autoridade temporal de cada máquina composta.
- CPUs, VDP, PSG, I/O e barramentos avançam por eventos/transações coordenados; chips não possuem loops temporais independentes.
- Clock domains e divisores devem ser derivados de fontes técnicas e manter fase/ordenação causal demonstráveis.
- T-states nominais e avanço por instrução são aproximações transitórias, não cycle accuracy.
- WAIT, arbitragem, acknowledge, interrupções e efeitos de borda só podem ser modelados com semântica sustentada por documentação ou medição.
- O pacer de host controla apenas a relação entre tempo emulado e tempo real; nunca altera a sequência lógica de eventos.

## 6. Host e apresentação

A arquitetura-alvo mantém os adaptadores de áudio, vídeo, input e storage no lado Rust, atrás de portas explícitas. A janela de jogo é responsabilidade do host Rust; Qt permanece launcher/painel de controle. Essa é uma meta arquitetural: cada backend só pode ser marcado como integrado quando o código, testes e evidências no SHA confirmarem a ligação.

Host-null/headless deve permitir testes determinísticos sem dispositivos físicos. Áudio e vídeo não podem bloquear a thread de emulação; filas e buffers precisam de políticas explícitas de overflow/underrun.

## 7. Mídia, perfil e firmware

- Identificar a mídia e resolver o perfil da máquina antes do início da execução.
- Distinguir identidade da ROM, revisão da mídia, modelo/revisão do console e região física.
- Não inferir hardware somente pelo nome do arquivo ou por um byte de cabeçalho.
- Preservar a imagem original; hashes e transformações devem ser rastreáveis.
- Firmware/BIOS deve ser externo, validado por identidade, versão, região e compatibilidade. Ausência de firmware obrigatório bloqueia o boot; firmware opcional não.
- Nunca selecionar mapper, perfil ou BIOS por fallback silencioso.
- Diagnósticos registram hashes e metadados, nunca conteúdo de ROM comercial ou dumps protegidos.

## 8. Evidência e qualidade

Cada componente usa estados separados:
- `IMPLEMENTED`
- `QUICK_PASS`
- `SHORT_PASS`
- `LONG_PASS`
- `INTEGRATION_PENDING`
- `TEMPORAL_PASS`

Cada suíte é `PASS`, `FAIL`, `BLOCKED`, `NOT RUN` ou `SKIPPED`, vinculada ao SHA candidato. Teste ignorado, CI de SHA anterior ou documentação sem código não é aprovação.

Golden master demonstra preservação do comportamento observado, não correção. Correção semântica exige oráculo independente com origem/licença/versão registradas. Compatibilidade deve distinguir boot, gameplay smoke, regressão e validação ampliada.

Gates obrigatórios no SHA candidato:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

## 9. Hierarquia documental

1. `docs/project-roadmap.md`: única fonte de escopo ativo, fases e prioridade.
2. `docs/project-scope.md`: princípios e limites arquiteturais, sem cronograma.
3. `docs/component-status.md`: estado observado dos componentes e integrações.
4. ADRs: decisões arquiteturais e seu estado.
5. Especificações/auditorias: comportamento, fontes, incertezas e evidências.
6. Issues/PRs: execução, dependências, critérios de aceitação e validação por SHA.

Documentos de planejamento antigos foram removidos. Documentos técnicos e componentes permanecem, desde que não sejam usados como roteiro concorrente. Se houver conflito, interromper a execução e corrigir o roadmap por PR antes de continuar.
