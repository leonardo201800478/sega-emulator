# Arquitetura de emulação multi-sistema

## Objetivo e diretriz oficial

Construir um emulador determinístico, testável e evolutivo, começando pelo SG-1000. **Rust é a implementação oficial do núcleo de emulação**: CPU, barramento/memória, I/O, VDP, PSG, agenda de eventos, sincronização e composição da máquina. **Python é a camada de aplicação e desenvolvimento**: frontend Qt/PySide6, configuração, ferramentas, automação, benchmarks e testes.

A migração do runtime existente em Python para Rust é incremental; não implica reescrita integral imediata. O código Python de referência será mantido apenas enquanto melhorar a pesquisa ou fornecer um oráculo diferencial útil. Não manteremos dois runtimes completos como objetivo final. Após atingir os critérios de aceitação, Rust passa a ser a fonte oficial de cada componente.

## Princípios

1. **Fidelidade antes de atalhos:** instruções, flags, efeitos colaterais, interrupções e temporização devem ser sustentados por documentação e testes.
2. **Separar CPU de plataforma:** núcleo de CPU depende de contratos de barramento/sinais, não de um console concreto.
3. **Separar estado lógico de temporização:** ciclos nominais de instrução não equivalem a cada ciclo de barramento.
4. **Dispositivos explícitos:** memória, VDP, PSG, controladores e chips de som têm estados e regras de acesso definidos.
5. **Composição por plataforma:** cada console conecta componentes concretos e define mapas, clocks, reset, interrupções e arbitragem.
6. **Determinismo:** a mesma configuração, estado inicial e sequência de eventos deve produzir o mesmo resultado.
7. **Sem abstração prematura:** introduzir interfaces a partir de necessidades reais e testes.
8. **Precisão declarada por evidência:** documentar o implementado, validado e aproximado separadamente.
9. **Fronteira Python/Rust grossa:** PyO3/Maturin conectam o runtime Rust ao frontend; não devem causar callbacks Python por instrução, por acesso a byte ou para coordenar dispositivos internos.
10. **Otimização baseada em medição:** profiling e benchmark antes de otimizar; não adotar JIT ou paralelismo sem necessidade comprovada.

## Camadas

### 1. Núcleos de processadores

Cada arquitetura de CPU terá implementação, testes e documentação próprios em Rust. O Z80 deve ser reutilizado nos sistemas que realmente contêm um Z80 compatível, respeitando sinais e configurações específicos. O Mega Drive exige um núcleo 68000 separado; a plataforma 32X acrescenta SH-2. Não compartilhar código entre arquiteturas distintas.

Cobrir instruções oficiais e prefixos do modelo escolhido, modos de endereçamento, flags, limites, interrupções e estados especiais. Comportamentos não documentados devem ser identificados e implementados apenas com evidências experimentais ou fontes confiáveis.

### 2. Barramento e contratos de acesso

O contrato atual de leitura/escrita de bytes é válido para a etapa inicial, mas não representa um barramento temporal completo. Evoluir em etapas testadas para:
- memória e portas de I/O;
- ciclos de leitura/escrita, espera e contenção;
- interrupções, reset, HALT e linhas de controle;
- avanço temporal e sincronização de dispositivos.

Não introduzir todos os contratos simultaneamente sem necessidade demonstrada.

### 3. Dispositivos de hardware

Implementar dispositivos em tipos Rust independentes sempre que isso represente o hardware e facilite testes. Reutilizar componentes apenas quando as características lógicas e temporais forem realmente compatíveis; preservar diferenças de revisão, mapa, portas e clocks.

### 4. Frontend e ferramentas

Qt/PySide6 permanece em Python, consumindo uma API Rust de alto nível por PyO3. A UI não deve conhecer detalhes internos dos dispositivos. Testes, auditorias, oráculos diferenciais e ferramentas continuam em Python quando isso melhorar a produtividade. O runtime Python de emulação deve ser reduzido gradualmente e não receber novas funcionalidades críticas por padrão.

## Sistemas e dependências arquiteturais

| Sistema | Processadores relevantes | Consequência |
|---|---|---|
| SG-1000 | Z80 | Primeiro alvo para validar CPU, memória, I/O, VDP, PSG e entrada. |
| Master System / Game Gear | Z80 | Reutilizar o núcleo Z80 compatível; tratar VDP, som, I/O e diferenças da plataforma separadamente. |
| Mega Drive / Genesis | Motorola 68000 principal e Z80 auxiliar | Núcleo 68000 próprio; modelar comunicação, memória e arbitragem com o Z80. |
| Sega CD | Base Mega Drive, 68000 adicional e subsistemas próprios | Adicionar CPU, CD, memória e dispositivos em marco separado. |
| 32X | Base Mega Drive e dois SH-2 | Núcleos SH-2 próprios e composição gradual da plataforma. |

A tabela é direção arquitetural, não declaração de implementação concluída.

## Estratégia de testes

- Testes unitários independentes para CPU, memória/bus, I/O, VDP, PSG e cada periférico.
- Testes de integração para máquina e interação temporal entre componentes.
- Vetores fixos e oráculos independentes sempre que viável.
- Testes diferenciais Python/Rust durante a migração, usando estado inicial, ROM sintética/homebrew, entradas e checkpoints equivalentes.
- Comparar registradores/flags, PC/SP, ciclos, portas, estado VDP/PSG, framebuffer e amostras. Investigar a primeira divergência.
- Testes de determinismo com execuções repetidas; hashes são sinais úteis, não prova absoluta da correção.
- Testes de desempenho separados dos funcionais e sem extrapolar uma ROM sintética para jogos em geral.

Os workflows GitHub Actions devem separar, sem duplicar desnecessariamente, validação funcional, integridade/determinismo, cobertura/temporização, integração e desempenho. Jobs automatizados são validadores reproduzíveis, não agentes de IA por definição. Um job pulado ou bloqueado não é um teste aprovado.

## Migração incremental

1. **Baseline e contratos:** inventariar módulos, dependências, APIs e testes existentes.
2. **CPU/barramento Rust:** reutilizar os crates presentes e retirar callbacks Python do caminho de execução.
3. **Máquina SG-1000:** integrar CPU, memória, I/O e scheduler nativos com avanço temporal determinístico.
4. **VDP TMS9918A:** migrar registradores, VRAM, portas, rasterização, status e sprites, mantendo regressões de framebuffer.
5. **PSG SN76489AN:** migrar registradores, divisores, tons, ruído, atenuação, amostragem e sincronização.
6. **Entrada/carregamento/runtime:** mover lógica que pertença à máquina para Rust; manter a seleção de arquivos e UI em Python.
7. **Qt fino:** consumir snapshots e operações de alto nível sem callbacks Python no caminho crítico.
8. **Retirada seletiva do runtime Python:** remover código redundante somente depois de equivalência suficiente, migração dos consumidores e CI confirmada.
9. **Master System/Game Gear:** reutilizar apenas componentes compatíveis e registrar diferenças de hardware.
10. **Mega Drive, Sega CD e 32X:** começar cada sistema com inventário técnico próprio e testes antes da implementação.

Cada fase deve constituir PR pequena, testável e reversível. Evitar, na mesma PR, migração de linguagem e mudança funcional de hardware.

## Critérios mínimos de conclusão de uma CPU

- Cobertura documentada dos opcodes oficiais e prefixos do modelo escolhido.
- Testes de resultados, flags conhecidas, PC/SP, wraparound e efeitos colaterais.
- Ciclos nominais validados em caminhos tomados/não tomados e modos de endereçamento.
- Interrupções, reset, HALT e I/O conforme a arquitetura.
- Suítes independentes/diferenciais e regressões automatizadas.
- Lacunas e incertezas identificadas; nenhuma alegação de ciclo exato sem evidência temporal correspondente.

## Referência externa: Gearsystem

[Gearsystem](https://github.com/drhelius/Gearsystem) é referência de estudo para SG-1000, Master System e Game Gear. Compare organização, validação, compatibilidade, áudio, vídeo e práticas de teste; não presuma que a arquitetura C++ possa ser transposta literalmente. O projeto declara GPL-3.0: antes de reutilizar qualquer código/teste/asset, faça revisão explícita da licença e compatibilidade. Prefira reimplementação independente baseada em especificações e testes legalmente utilizáveis.

## Estado do repositório (observação em 26/10/2026)

- Já existem os crates Rust `native/sg1000_core`, `native/z80_alu` e a extensão `native/z80_core` com PyO3.
- O crate standalone ainda é parcial: CPU, memória e máquina headless não equivalem ao sistema completo. VDP, PSG, I/O e scheduler exigem integração e testes adicionais.
- A extensão existente já integra vários dispositivos, mas a documentação técnica registra limitações: objetos PyO3 no caminho interno, GIL durante a execução de quadro e temporização por T-states nominais. Isso não comprova runtime completamente independente de Python nem precisão ciclo a ciclo.
- Próxima tarefa: evoluir o crate standalone com componentes Rust puros, preservar contratos, expandir testes e sincronizar cada avanço com CI. A interface Qt permanece em Python.
- Consultar [plano de migração](rust-runtime-migration-plan.md), [ADR 0006](adr/0006-standalone-rust-sg1000-core.md), [auditoria do código](codebase-audit.md) e [handoff de desenvolvimento](development-handoff-2026-10-22.md) como registros de estado, confirmando o código mais recente antes de atuar.
