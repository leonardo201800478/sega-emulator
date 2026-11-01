# Plano de implantação — Rust Temporal Core

**Branch inicial:** architecture/rust-temporal-core  
**Estado:** T0 implementado no código candidato; integração ainda pendente.  
**Regra:** não alterar o avanço das máquinas existentes até haver contratos, testes e equivalência demonstrada.

## Inventário confirmado no HEAD de origem

- SG-1000 mantém cpu.total_tstates e sincroniza VDP/PSG em fronteiras de instrução.
- A máquina SG-1000 converte T-states em clocks do PSG com numerador/resto local e filas específicas de escrita.
- SMS também mantém contagem T-state/PSG e fila própria de escrita.
- VDPs SG-1000 e SMS avançam por contadores agregados de ciclos/scanline; os próprios módulos declaram que ainda não são dot-cycle exact.
- O contrato Z80 possui registros de timing para famílias instrumentadas, mas a documentação do barramento deixa claro que isso não é trace pin-level completo nem callback do scheduler.
- Não existe ainda um crate temporal compartilhado no workspace.

Esses caminhos serão preservados até que o novo mecanismo prove equivalência e cubra suas lacunas. Não duplicar nem remover as conversões atuais nesta fase.

## T0 — infraestrutura independente

- [x] Criar sega-temporal-core sem dependências de host ou console.
- [x] Implementar MachineTime, ClockRatio, ClockDomain e snapshot.
- [x] Implementar fila determinística EventQueue<T>.
- [x] Criar testes unitários para razão fracionária, invariância por chunks, fase, monotonicidade, overflow e ordenação.
- [ ] Executar fmt, Clippy e testes no SHA candidato.
- [ ] Observar workflows aplicáveis no SHA candidato.

## T1 — scheduler headless

1. Definir API MachineScheduler e política de delta-cycle/eventos simultâneos.
2. Definir perfis de tempo SG-1000/SMS a partir de documentação técnica primária; não codificar clocks presumidos.
3. Adicionar clock virtual, replay de eventos e checkpoints.
4. Validar causalidade, overflow, wraparound, pause/reset/load/stop e determinismo.
5. Não integrar chamadas de host nem depender de PR #300/#301 para esta fase.

## T2 — adaptação dos dispositivos

1. Criar adaptadores temporais sem reescrever os chips.
2. Integrar CPU/barramento primeiro com pontos de acesso e waits.
3. Integrar VDP com eventos raster/IRQ e efeitos temporais de registradores.
4. Integrar PSG com divisor/fase e writes timestamped.
5. Comparar traces e resultados contra a implementação anterior e oráculos independentes.
6. Remover contadores/conversões locais apenas após equivalência comprovada.

## T3 — micro-operações e ciclo a ciclo

- Refatorar CPU por famílias para emitir operações temporais explícitas.
- Modelar M-cycles/T-states, refresh, I/O, ciclos internos, acknowledge, waits e interrupções.
- Introduzir contenção e arbitragem específica de cada plataforma.
- Não confundir a soma dos T-states com a ordem e o instante dos acessos ao barramento.
- Promover TEMPORAL_PASS somente com evidências por T-state/evento.

## T4 — integração de host e expansão

- PR #300 input: necessária para captura nativa e publicação de eventos de gameplay.
- PR #301 vídeo: necessária para apresentação nativa e pacing do host.
- Runtime de áudio: reutilizar a sessão Rust existente; o worker de saída não avança a máquina.
- Essas integrações não bloqueiam o Temporal Core headless, mas são gates para declarar a fronteira completa do host.
- Expandir para Mega Drive, Sega CD e 32X apenas quando cada plataforma tiver perfil e evidência temporal próprios.

## Critérios de aceitação do marco atual

- [ ] Workspace formata e compila com lockfile.
- [ ] Clippy sem warnings.
- [ ] Testes do novo crate passam.
- [ ] Testes existentes SG-1000/SMS continuam passando.
- [ ] CI aplicável passa no SHA candidato.
- [ ] Nenhum runtime de hardware Python é introduzido.
- [ ] Nenhuma alegação de precisão ciclo a ciclo é feita nesta fase.
