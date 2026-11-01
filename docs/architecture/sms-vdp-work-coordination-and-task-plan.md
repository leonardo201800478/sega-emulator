# Plano operacional — auditoria VDP SMS e migração gráfica nativa

**Escopo:** PR #338 — `feat(video): plan full SMS VDP audit and native Vulkan/D3D backends`  
**Branch:** `feat/sms-vdp-audit-native-graphics-apis`  
**Natureza:** plano de execução e coordenação; não representa implementação concluída nem criação de agentes autônomos.  
**Regra de capacidade:** até três frentes/agentes de IA em paralelo, conforme a política geral do repositório. CI é um verificador independente, não um agente.

## 1. Objetivo operacional

Fechar primeiro a correção e a previsibilidade do VDP SMS — com prioridade máxima para sprites, SAT, scroll e raster — e somente depois avançar para renderer Rust e backends Vulkan, D3D11 e D3D12. A apresentação gráfica nunca poderá controlar ou modificar o tempo emulado.

A PR permanece Draft até os critérios de aceite e evidências descritos aqui serem cumpridos no mesmo SHA candidato.

## 2. Organização das frentes

As frentes abaixo são papéis de trabalho. Podem ser executadas por agentes independentes quando disponíveis ou pelo executor principal em sessões separadas. Não presumir que uma frente está executando em segundo plano sem confirmação observável.

| Frente | Responsabilidade exclusiva | Entregáveis principais |
|---|---|---|
| A — Hardware e modelo temporal | Modelar comportamento VDP/SAT/raster com base em fontes primárias e referências de emuladores | Especificação de eventos, tabela de evidências, decisões por revisão SMS1/SMS2/GG |
| B — Implementação e integração Rust | Implementar correções isoladas em Rust, APIs e integração com scheduler/GUI quando autorizada | Commits pequenos, testes unitários, integração sem Python no caminho gráfico |
| C — Validação independente | Criar vetores, executar testes, comparar ARES/MAME e analisar divergências | Matriz de testes, traces, screenshots, hashes e relatórios PASS/FAIL/BLOCKED |

**Separação de responsabilidades:** quem implementa não é o único responsável por declarar o resultado validado. A frente C revisa os critérios e as evidências. O CI continua independente.

## 3. Política de despacho, bloqueio e redistribuição

1. Cada tarefa deve ter um único responsável primário e, quando necessário, um revisor de outra frente.
2. Manter no máximo três tarefas ativas em paralelo. Tarefas dependentes ficam em fila, não iniciadas prematuramente.
3. Uma tarefa só é considerada iniciada quando existe responsável, objetivo, arquivos/área delimitados e critério de conclusão.
4. Marcar **BLOCKED** quando faltar fonte técnica, acesso a ferramenta, ambiente, oráculo ou decisão arquitetural.
5. Ao bloquear, registrar: motivo verificável, evidência tentada, dependência necessária, próximo passo e condição de desbloqueio.
6. Se não houver progresso verificável após uma rodada de execução/revisão, ou se a dependência não puder ser resolvida pela frente atual, suspender a tarefa e redistribuir o trabalho independente para outra frente disponível.
7. Não transferir uma tarefa com alterações não publicadas sem primeiro preservar o estado e registrar o SHA/arquivos. Nunca sobrescrever trabalho local do usuário.
8. A redistribuição não apaga o histórico: registrar responsável anterior, motivo, novo responsável e estado.
9. Não simular execução paralela. Se só houver um executor disponível, executar as frentes em sequência e manter o mesmo rigor de revisão cruzada.
10. CI não deve ser usado como substituto de pesquisa de hardware, inspeção de imagens ou validação temporal.

## 4. Backlog priorizado

### P0 — Fechar o contrato de sprites e scroll

**A0.1 — Inventariar implementação atual**
- [ ] Inspecionar `native/sms_core/src/vdp.rs`, configuração SMS e todos os call sites do modo de limite de sprites.
- [ ] Registrar o comportamento efetivo por perfil/modo: seleção de sprites, SAT, Y+1/wrap, terminador, X shift, zoom, 8x16, ordem, prioridade, overflow e colisão.
- [ ] Confirmar a ligação da preferência Qt `sprite_limit_enabled` até a criação da máquina; a API core/PyO3/Python já exposta não prova integração ponta a ponta.
- **Saída:** inventário com símbolos/arquivos e gaps reproduzíveis.

**A0.2 — Formalizar pipeline de avaliação**
- [ ] Modelar avaliação da linha N para apresentação N+1.
- [ ] Definir quando os oito índices são amostrados e quando atributos/padrões são lidos.
- [ ] Especificar efeitos de escritas SAT/VRAM durante a linha, incluindo quais alterações podem afetar a linha atual ou seguinte.
- [ ] Separar diferenças SMS1, SMS2 e Game Gear; não generalizar zoom/overflow sem evidência.
- **Saída:** diagrama temporal e tabela de regras com fonte e nível de confiança.

**A0.3 — Formalizar coordenadas e scroll**
- [ ] Provar por vetores que R8/R9 afetam background e não deslocam sprites diretamente.
- [ ] Cobrir movimento de objetos pelo jogo: atualização da SAT, scroll fino/grosso e transição de borda.
- [ ] Cobrir wrap X/Y, R0 bit 3, valores extremos e mudanças de registradores em meio à linha.
- **Saída:** vetores de referência com coordenadas esperadas por frame/linha/pixel.

**A0.4 — Política de limite de sprites**
- [ ] Preservar o padrão hardware-faithful: limite habilitado.
- [ ] Validar o modo desbloqueado como extensão de compatibilidade, sem alterar o status OVERFLOW de hardware.
- [ ] Testar 8/9/64 sprites por linha, modos Mode 4 e legacy, prioridade e colisão.
- [ ] Ligar a preferência persistida Qt ao construtor/configuração da máquina e validar persistência, default e alternância em runtime.
- **Saída:** teste core + adapter + GUI; documentação explícita de que unlocked não é hardware real.

**Critério de saída P0:** matriz de sprites/scroll passa em testes sintéticos e em pelo menos um oráculo independente; divergências restantes têm causa/escopo documentados. Não declarar TEMPORAL_PASS nesta fase apenas por testes de coordenadas.

### P1 — Auditoria funcional completa do VDP

**A1.1 — Registradores e portas**
- [ ] R0–R10, reset, valores reservados, latch, read-ahead, endereçamento e wrap VRAM/CRAM.
- [ ] Status, VBlank, HINT, IRQ, acknowledge e H/V counters por modo/região.
- [ ] Testes de escritas/leitura durante display ativo.

**A1.2 — Background e composição**
- [ ] Name table, pattern generator/attributes, planos de bits, flips, paleta, prioridade e backdrop.
- [ ] Scroll fino/grosso, locks, left-column mask, wrap e mudanças mid-frame.
- [ ] Modos legados TMS e diferenças de comportamento.

**A1.3 — Raster e temporização**
- [ ] Linhas/clocks por região, display ativo, HBlank/VBlank e sequência de fetch.
- [ ] Latches de registradores, VRAM e CRAM com timestamp definido.
- [ ] Acessos CPU/barramento sincronizados antes de aplicar efeitos colaterais.
- [ ] IRQ e eventos simultâneos com ordenação determinística.
- **Saída P1:** relatório de lacunas fechado/aberto, testes de regressão e trace de primeira divergência.

### P2 — Renderer Rust headless

- [ ] Mapear crates e caminhos gráficos existentes antes de criar novos crates.
- [ ] Definir frame snapshot imutável, formato, ownership e contrato de transferência.
- [ ] Implementar renderer/headless Rust sem API gráfica e sem Python/PyO3.
- [ ] Comparar byte a byte/hash com saída digital de referência.
- [ ] Provar que renderização não altera estado VDP, scheduler, IRQ ou PCM.
- **Saída:** renderer headless determinístico, testável em CI sem GPU.

### P3 — Backends gráficos nativos (sequenciais)

**P3.1 Vulkan**
- [ ] Spike de dependências Rust, surface, device, swapchain, queues e sincronização.
- [ ] Lifecycle: resize, DPI, fullscreen, alt-tab, minimize/restore, shutdown e device loss.
- [ ] Comparação do frame apresentado com o snapshot de origem.

**P3.2 Direct3D 11**
- [ ] Implementar adapter e lifecycle nativo Windows.
- [ ] Validar feature levels, formatos, sincronização, resize e recuperação.
- [ ] Rodar a mesma suíte visual/host do Vulkan.

**P3.3 Direct3D 12**
- [ ] Implementar adapter, command queues/lists, descriptors, barriers e fences.
- [ ] Validar swapchain, resize, device removal e shutdown.
- [ ] Rodar a mesma suíte visual/host dos demais backends.

**Regra:** não iniciar os três ports em paralelo antes de o contrato headless estar estável. Reutilizar abstrações apenas quando equivalência e custo forem demonstrados; documentar decisões em ADR.

### P4 — Integração com scheduler e host

- [ ] Fazer o scheduler Rust avançar VDP/CPU/barramento em uma timeline determinística.
- [ ] Publicar frames concluídos sem esperar a GPU.
- [ ] Definir fila limitada e backpressure sem alterar o tempo lógico.
- [ ] Reproduzir mesmo estado e framebuffer em headless/Vulkan/D3D11/D3D12.
- [ ] Validar SG-1000 como regressão obrigatória e SMS no Windows.
- [ ] Medir separadamente tempo de emulação, renderização, submissão, apresentação, FPS lógico e FPS apresentado.

### P5 — Aceitação e encerramento

- [ ] Todos os gates Rust e Python existentes passam no mesmo SHA candidato.
- [ ] Testes headless e testes de integração Windows executados e registrados.
- [ ] Capturas/traces com ROM homebrew ou conteúdo legal, SHA-256 e configuração reproduzível.
- [ ] ARES/MAME usados como oráculos com versão e limitações registradas.
- [ ] Nenhum warning com `-D warnings`; workflows existentes não alterados sem autorização.
- [ ] Remoção do legado somente após equivalência demonstrada.
- [ ] Atualizar PR, changelog técnico e matriz de maturidade por backend.

## 5. Matriz mínima de validação de sprites

| Dimensão | Casos obrigatórios |
|---|---|
| Coordenadas | X=0,1,7,8,247,248,255; Y=0,1,0xEF,0xF0,0xFF; Y+1 e wrap |
| Deslocamento | R0 bit 3 ligado/desligado; X antes/depois do shift |
| Scroll | R8=0,1,3,7,8,15,127,255; R9 e locks R0.6/R0.7 |
| SAT | base variável, wrap VRAM, terminador $D0 no modo 192 |
| Formato | 8x8, 8x16, zoom 1x/2x, seleção de padrão |
| Limite | 8, 9 e 64 sprites na mesma linha; ordem SAT |
| Status | OVERFLOW, collision, sprites ocultos/fora da tela |
| Temporal | avaliação N/N+1; alteração SAT durante linha; leitura de padrão/atributos |
| Perfil | SMS1, SMS2 e GG somente onde a evidência/documentação sustentar |

Cada caso deve registrar: configuração/perfil, estado inicial, sequência de writes, linha/frame alvo, pixels esperados, resultado real, oráculo e primeira divergência.

## 6. Evidência e estados obrigatórios

Usar os estados de validação do projeto: **PASS**, **FAIL**, **BLOCKED**, **NOT RUN**. Para componentes, usar **IMPLEMENTED**, **QUICK_PASS**, **SHORT_PASS**, **LONG_PASS**, **INTEGRATION_PENDING** e **TEMPORAL_PASS**.

- **PASS** exige comando, ambiente, SHA e log.
- **FAIL** exige reprodução mínima e primeira divergência.
- **BLOCKED** exige dependência explícita e plano de desbloqueio.
- **NOT RUN** significa que não há evidência de execução.
- **TEMPORAL_PASS** exige traces temporais e oráculos independentes; testes unitários de framebuffer não bastam.

Não reutilizar resultados de SHA anterior como validação do candidato atual.

## 7. Ordem de execução e dependências

```text
P0 (sprites/scroll + GUI toggle)
  └── P1 (auditoria funcional/temporal VDP)
        └── P2 (renderer headless Rust)
              ├── P3.1 Vulkan
              ├── P3.2 D3D11
              └── P3.3 D3D12
                    └── P4 integração scheduler/host
                          └── P5 aceitação
```

P3 pode ser executado em sequência após P2; não usar paralelismo para contornar dependências. Pesquisa documental de P1 pode ocorrer em paralelo com a implementação isolada de P0, desde que os contratos ainda não estejam em disputa.

## 8. Estado inicial deste plano

Estado herdado da descrição da PR #338; precisa ser revalidado no SHA atual antes de execução:

| Item | Estado inicial |
|---|---|
| Especificação arquitetural VDP/backends | IMPLEMENTED (documentação) |
| API do limite de sprites core/native/Python | presente na branch; CI/runtime ainda requerem validação no SHA atual |
| Testes sintéticos R8 e nono sprite | adicionados; execução local NOT RUN na sessão anterior |
| Preferência Qt ligada à máquina | INTEGRATION_PENDING |
| Pipeline SAT N/N+1 com amostragem temporal completa | NOT RUN / pendente de implementação |
| Auditoria temporal integral | NOT RUN |
| Renderer headless Rust | NOT RUN |
| Vulkan/D3D11/D3D12 | NOT RUN |
| Integração scheduler + GPU host | NOT RUN |
| Comparação visual ARES/MAME em campanha | NOT RUN |
| TEMPORAL_PASS | NOT CLAIMED |

## 9. Próxima rodada obrigatória

1. Revalidar branch head, CI e diff da PR #338.
2. Executar A0.1 e confirmar o estado real do toggle Qt e dos testes sintéticos.
3. Fechar A0.2/A0.3 com referências e casos reproduzíveis antes de alterar o algoritmo de sprites.
4. Implementar apenas a menor correção sustentada pela evidência.
5. Executar testes direcionados, depois gates completos no mesmo SHA.
6. Atualizar este documento e a descrição da PR com evidências, não estimativas.

## 10. Fontes técnicas de partida

- SMS Power — Sprites: https://www.smspower.org/Development/Sprites
- SMS Power — VDP Registers: https://www.smspower.org/Development/VDPRegisters
- MAME — TMS9918/SMS VDP implementation: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp

As fontes de partida não substituem a inspeção da revisão exata do hardware, traces ou testes independentes. Toda regra com diferenças por ASIC deve manter a proveniência e o grau de confiança explícitos.
