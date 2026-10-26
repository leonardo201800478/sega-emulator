# Plano de migração do runtime para Rust

## Decisão vigente

O núcleo de emulação seguirá **Rust-first**: Rust é a implementação oficial de CPU, barramento/memória, I/O, VDP, PSG, sincronização e runtime da máquina. Python continuará no frontend Qt/PySide6 e nas ferramentas. A migração é incremental; não é uma reescrita completa nem uma obrigação de portar tudo duas vezes.

O código Python de referência será mantido seletivamente enquanto facilitar a pesquisa de hardware ou permitir comparação de resultados. Após validar um componente, Rust passa a ser a implementação oficial. Não manter dois runtimes completos como estado final.

A extensão PyO3/Maturin é a fronteira entre o núcleo Rust e a UI, não o mecanismo de comunicação entre componentes. O caminho de emulação não deve depender de callbacks Python por instrução ou acesso a byte. Qualquer estratégia para liberar o GIL ou usar threads precisa de um modelo explícito de ownership, estado e sincronização.

## Estado e evidência

O repositório inclui:
- `native/sg1000_core`: crate standalone em evolução, sem dependência de Python/PyO3.
- `native/z80_alu`: ALU reutilizável em Rust.
- `native/z80_core`: núcleo/extensão compatível com PyO3.
- Frontend Qt em Python e testes/harnesses de referência em Python.

A documentação histórica registra uma validação do workflow Windows no commit `7bb95c0f55dfeb0332b9a5ea6b3da54d1bdee43c`, incluindo fmt, Clippy, build Release, testes direcionados e suíte pytest. Esses números pertencem àquele commit e não são afirmação automática sobre HEAD atual. Confira o status do workflow para a revisão candidata antes de usar os resultados como evidência atual.

O crate `native/sg1000_core` continua parcial: ainda é necessário integrar ou reimplementar em tipos Rust puros os dispositivos e o scheduler necessários para uma máquina SG-1000 standalone completa. Não considerar o marco concluído por compilar ou rodar ROM sintética.

## Destino arquitetural

### Rust
- CPU e estado de execução;
- memória, barramento, mapeamento de cartucho e I/O;
- máquina SG-1000 e scheduler/eventos temporais;
- VDP TMS9918A, incluindo portas, registradores, VRAM, raster, sprites, status e framebuffer;
- PSG SN76489AN, incluindo registradores, divisores, tons, ruído, atenuação, amostragem e sincronização;
- entrada e demais componentes internos do hardware;
- APIs de estado/reset/step/frame para o frontend.

### Python
- interface Qt/PySide6, menus, widgets, configuração e ciclo de vida da UI;
- pytest, oráculos, harnesses diferenciais, scripts de auditoria e benchmark;
- protótipos pequenos apenas quando diminuírem o custo de pesquisar comportamento desconhecido.

O frontend Pygame/SDL e caminhos de aplicação Python podem ser consolidados ou removidos apenas depois que Qt consumir com segurança a API Rust e a saída/entrada tiverem substitutos validados.

## Fases e critérios

### 0 — Baseline e contratos
Inventariar módulos e consumidores; registrar estado dos testes; fixar vetores para CPU, memória, I/O, frame, VDP, PSG e entrada conforme suportados. Separar testes funcionais, diferenciais, temporais e desempenho.

**Saída:** mapa de dependências, contratos observáveis e critérios de aceite.

### 1 — CPU e barramento Rust puro
Reutilizar o Z80 existente sem duplicar implementação; fazer o núcleo possuir estruturas Rust para estado de CPU, memória e I/O, sem acesso a objeto Python no caminho de instrução. Expor à UI apenas operações de alto nível.

**Aceitação:** testes de CPU/barramento e vetores de referência passam; estado e T-states observáveis comparáveis; nenhuma chamada Python por byte/instrução no caminho movido.

### 2 — Máquina e scheduler
Conectar CPU, memória e dispositivos por contratos Rust. Definir ordem de eventos, clocks, reset, interrupções e avanço do tempo explicitamente. Não confundir ciclos nominais com ciclos de barramento.

**Aceitação:** execução repetível, regressões e checkpoints determinísticos; diferenças temporais documentadas.

### 3 — VDP
Migrar registradores, VRAM, portas, modos gráficos, status, interrupções e sprites. Preservar semântica durante extração; corrigir fidelidade em mudanças isoladas sempre que possível.

**Aceitação:** testes de portas, incremento/wrap, tabelas, modos, sprites e snapshots de framebuffer reproduzíveis.

### 4 — PSG
Migrar registradores, períodos, divisor, canais de tom, ruído/LFSR, atenuação, geração de amostras e sincronização com CPU.

**Aceitação:** vetores digitais, frequências, formas de onda, ruído, amostras e determinismo. Separar efeitos analógicos da geração digital.

### 5 — Entrada, ROM e serviços
Mover para Rust somente a lógica que pertence ao runtime; a seleção de arquivos fica na UI. Preservar formatos suportados, erros tipados, hashes e diagnóstico.

**Aceitação:** teste de formatos, erros, entradas e integridade com arquivos sintéticos/homebrew.

### 6 — Frontend Qt fino
Trocar dependência direta da máquina Python por API Rust de alto nível. Preservar pause, reset, configuração, input, apresentação, áudio e logs. Planejar separadamente responsividade e sincronização; não alterar clocks lógicos para compensar VSync/VRR.

**Aceitação:** build/instalação Windows, teste de seleção de backend, abrir/executar/pausar/resetar, controle, vídeo, áudio e logs.

### 7 — Remoção seletiva da referência
Só remover runtime Python quando os consumidores tiverem migrado e as regressões necessárias existirem. Manter pequenas referências de teste quando comprovadamente úteis.

**Aceitação:** instalação limpa, workspace Rust, extensão se usada, CI Python/Rust e integração Qt aprovados; nenhuma dependência de runtime Python dentro do núcleo oficial.

### 8 — Sistemas adicionais
Reutilizar Z80 no Master System/Game Gear somente onde compatível. Desenvolver núcleo 68000 para Mega Drive e núcleos necessários para Sega CD/32X em projetos de hardware próprios, com testes e marcos separados.

## Regras de segurança

1. Uma fase por PR, pequena e reversível.
2. Evitar combinar mudança de linguagem e mudança de comportamento temporal na mesma alteração.
3. Não aceitar ganho de velocidade com divergência funcional.
4. Nenhum fallback silencioso para Python quando Rust não suporta operação.
5. Benchmarks informam workload, build, aquecimento, repetições e mediana; não extrapolar instruções/s para FPS.
6. Não versionar ROMs comerciais, BIOS, logs locais sensíveis ou build artifacts.
7. Registrar incertezas e não afirmar precisão temporal não validada.

## Validação pelo GitHub Actions

Manter validadores complementares, sem workflows duplicados:
- funcional: testes unitários e regressões;
- integridade: imports, pacote e execução repetida;
- cobertura/temporal: matriz de instruções, flags, ciclos e interrupções;
- integração: máquina, VDP, PSG, entrada e Qt/PyO3 conforme escopo;
- desempenho: benchmark controlado com baseline.

Todos os resultados devem pertencer ao mesmo SHA candidato. Checks ignorados por path filters, falhas que impedem pytest e artefatos gerados apesar da falha não podem ser relatados como aprovação. Depois de corrigir uma falha, repetir checks afetados e regressões. Workflows são validadores de comandos reproduzíveis; chamá-los de “agentes” não implica que sejam agentes de IA.

## Próxima etapa

Consolidar o crate standalone SG-1000 com CPU, memória, I/O, VDP, PSG e scheduler em estruturas Rust puras. Começar pelo inventário e pelos testes que faltam, mantendo a API existente e sem duplicar arquivos ou implementar sistemas futuros antes do marco atual.
