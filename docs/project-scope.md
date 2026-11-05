# Escopo e princípios arquiteturais — Sega Emulator

**Autoridade de escopo e sequência:** [Roadmap mestre](project-roadmap.md).  
**Diretriz de fidelidade:** [ADR-0016 — fidelidade observável e precisão proporcional](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).  
**Plataformas prioritárias:** Master System, preservando SG-1000.  
**Runtime de hardware:** Rust.  
**Aplicação e tooling:** Python / Qt / PySide6.  
**Fronteira:** PyO3 coarse-grained.

Este documento define princípios e limites transversais. A sequência e as prioridades pertencem exclusivamente ao roadmap.

## 1. Objetivo

Construir um emulador Sega modular, determinístico, compatível e eficiente, com comportamento funcional e temporal próximo ao dos consoles originais.

A meta não é reproduzir a engenharia elétrica integral dos chips. A precisão deve ser suficiente para executar jogos e periféricos corretamente, respeitando clocks, sincronização e interações que tenham efeito observável.

**Fidelidade antes de complexidade:** um detalhe interno só deve ser modelado quando houver dependência real do console, jogo ou periférico, sustentada por documentação, medição ou teste reproduzível.

## 2. Limites de escopo

- Priorizar a conclusão do Master System e preservar as funcionalidades já validadas do SG-1000.
- Avançar para outras plataformas somente conforme a sequência de fases do roadmap.
- Pesquisa de hardware futuro não autoriza antecipar sua implementação.
- Não criar roadmaps ou cronogramas paralelos.
- JIT, frame skipping, hacks temporais e otimizações especulativas são proibidos.
- Não versionar ROMs comerciais, BIOS proprietárias, firmware protegido ou dumps de memória protegida.

## 3. Responsabilidade por tecnologia

### 3.1 Rust — runtime único de hardware

Todo estado e comportamento digital de hardware pertence a Rust:
- CPU, ALU, registradores e interrupções;
- RAM, ROM, VRAM, barramentos, mappers e decodificação;
- VDP, raster, sprites, paletas, DMA quando aplicável;
- PSG, FM, PCM, filtragem digital e mixagem;
- I/O, periféricos e protocolos;
- clocks, sincronização e espera quando necessários ao comportamento da plataforma;
- save states, replay e estado da máquina.

Não manter hardware em Python como fallback, implementação paralela ou runtime alternativo. Código em `legacy/` é referência histórica.

### 3.2 Python — aplicação e ferramentas

Python/Qt pode apresentar menus, biblioteca, preferências, logs e diagnósticos; selecionar ROMs e perfis; enviar comandos de alto nível; consultar telemetria; executar ferramentas de auditoria e testes.

Python não executa CPU/máquina, conta ciclos de hardware, controla barramento, decide IRQ, processa PCM de gameplay ou atua como autoridade temporal.

### 3.3 PyO3/Maturin

A fronteira deve ser coarse-grained, com ownership e sincronização explícitos. Não usar callbacks Python por instrução, pixel ou amostra.

## 4. Composição e reutilização

- Compartilhar CPU e componentes quando representarem hardware compatível e a reutilização estiver demonstrada.
- VDPs de arquiteturas distintas permanecem específicos; não criar VDP universal.
- SG-1000 e SMS mantêm mapas, I/O, perfis de cartucho e particularidades próprias.
- PSGs podem compartilhar implementação digital com perfis de plataforma quando necessário.
- Diferenças de revisão só exigem modelos separados quando alterarem comportamento observável.
- Mapper, perfil ou firmware desconhecido deve produzir erro explícito, nunca fallback silencioso.
- O host não determina a velocidade lógica do console.

## 5. Temporização e precisão

- Um único Scheduler coordena a timeline da máquina composta quando necessário.
- CPU, vídeo, áudio, barramento e periféricos devem permanecer sincronizados nos pontos em que interagem.
- O avanço pode ocorrer por instrução, bloco, scanline, amostra ou evento, conforme a menor granularidade que preserve o comportamento observável.
- M-cycles, T-states, WAIT, fases de borda, arbitragem e sinais de barramento não são exigências universais. Modelar somente quando a plataforma realmente depender deles.
- Não representar todos os pinos físicos nem simular fenômenos analógicos/elétricos.
- WAIT ou outra linha de controle deve ser implementada quando houver ligação e efeito real no console suportado.
- A velocidade real do host e o pacing de apresentação não podem alterar a sequência lógica emulada.

## 6. Performance e estado

- Evitar eventos por tick/pixel/pino, alocações no caminho crítico e estados que não contribuam para o comportamento emulado.
- Usar estruturas compactas e processamento em blocos quando não houver interação observável intermediária.
- Medir antes de otimizar; preservar resultados funcionais e temporais.
- Save states devem conter o estado necessário à retomada determinística, não detalhes elétricos sem função no modelo.

## 7. Evidência e qualidade

Cada funcionalidade deve ter critérios proporcionais ao seu impacto:
- testes unitários para semântica e componentes;
- testes de integração para interações entre CPU, barramento, vídeo, áudio e periféricos;
- ROMs de diagnóstico, referências independentes e testes in-game quando disponíveis;
- medições de desempenho para alterações no caminho crítico.

Classificar suítes como PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED não equivale a PASS. Todo resultado deve estar vinculado ao SHA exato.

Gates de qualidade no SHA candidato:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Não declarar precisão absoluta, cycle-accuracy ou equivalência elétrica sem necessidade concreta e evidência específica.

## 8. Hierarquia documental

1. `docs/project-roadmap.md`: sequência e prioridades.
2. `docs/project-scope.md`: princípios e limites.
3. `docs/component-status.md`: estado observado dos componentes.
4. ADRs: decisões arquiteturais e seu estado.
5. Documentação de hardware: comportamento, fontes, incertezas e testes.
6. Auditorias datadas: evidência histórica, não roteiro ativo.
7. Issues/PRs: execução e validação por SHA.

Se houver conflito, a ADR-0016 define a política de fidelidade; o roadmap define a prioridade. Documentos históricos não criam gates nem tarefas por conta própria.
