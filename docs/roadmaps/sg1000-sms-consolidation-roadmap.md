# Roadmap de consolidação — SG-1000 + Master System

**Estado:** plano normativo de execução — prioridade máxima do projeto  
**Baseline inicial:** `main` em `3a9adad1a9f7aea2ca8be4c8dde4faf41431db29` (2026-10-31)  
**Objetivo:** reestruturar e consolidar os runtimes existentes, preservando o SG-1000 funcional e tornando o SMS progressivamente funcional e validado.  
**Runtime:** Rust único para hardware; Python/Qt para aplicação e ferramentas; PyO3/Maturin como fronteira fina.

> Este roadmap autoriza refatorações profundas e quebras temporárias em branches de trabalho. Não autoriza descartar regressões, sobrescrever trabalho alheio, publicar diretamente na main ou declarar sucesso sem evidência no SHA candidato.

## 1. Prioridade e política de escopo

A prioridade atual é **SG-1000 e Master System (SMS), em conjunto**. Não iniciar implementação de Game Gear, Mega Drive, Sega CD ou 32X enquanto isso desviar recursos da consolidação dos dois primeiros sistemas.

O SG-1000 é baseline de preservação; o SMS é alvo de fechamento funcional e de compatibilidade. Não existe competição entre os dois: toda mudança compartilhada deve manter regressões SG-1000 e SMS.

### 1.1 O que significa “funcional”

Para cada sistema, registrar separadamente:

- `IMPLEMENTED`: código presente e integrado no runtime Rust.
- `QUICK_PASS`: testes direcionados aprovados.
- `SHORT_PASS`: smoke test funcional curto aprovado.
- `LONG_PASS`: regressão prolongada/estresse aprovada.
- `INTEGRATION_PENDING`: integração ainda incompleta.
- `TEMPORAL_PASS`: evidência temporal apropriada, com traces e oráculo independente.

Estados adicionais de compatibilidade por jogo: `BOOT_CONFIRMED`, `GAMEPLAY_SMOKE`, `REGRESSION_PASS`, `VALIDATED`. Boot não equivale a gameplay; gameplay smoke não equivale a compatibilidade universal.

## 2. Arquitetura que não pode divergir

1. **Rust é o único runtime de hardware:** CPU, ALU, memória, barramento, mappers, I/O, VDP, PSG, interrupções, temporização e scheduler.
2. **Python/Qt é aplicação:** UI, biblioteca, configuração, carregamento de mídia, logs, diagnóstico, ferramentas e testes. Não executar hardware em Python.
3. **PyO3/Maturin é fronteira fina:** comandos coarse-grained e snapshots; sem callback Python por instrução, pixel ou amostra PCM.
4. **Master Scheduler Rust é a única autoridade temporal:** Qt/Python não avança instruções, frames ou clocks por QTimer, sleep ou loop de FPS.
5. **Compartilhar mecanismos, especializar hardware:** Z80 e primitivas realmente comuns podem ser compartilhados; VDPs, mapas de memória, I/O, PSGs, mappers e perfis específicos permanecem separados.
6. **Preservar a evidência:** não apagar testes funcionais existentes para acomodar uma refatoração. Portar os testes e adicionar equivalentes antes de remover duplicações.
7. **Sem falsa precisão:** T-states nominais por instrução não representam execução ciclo a ciclo. A campanha completa temporal pode vir depois da integração funcional, mas os contratos temporais precisam existir desde o início.

## 3. Sequência de fases

### Fase 0 — Baseline reproduzível e inventário

**Objetivo:** saber exatamente o que funciona antes de reorganizar.

- Fixar SHA da main, branches e PRs relevantes.
- Inventariar crates, módulos, APIs públicas, dependências e ownership de cada componente.
- Mapear testes Rust/Python, workflows, filtros de paths e artefatos.
- Registrar jogos legais/homebrew que iniciam em cada sistema, com hash e passos de reprodução locais.
- Capturar checkpoints de boot, framebuffer, áudio e entrada sem incluir ROMs comerciais ou conteúdo de ROM no Git.
- Classificar lacunas como código ausente, integração pendente, teste ausente ou documentação desatualizada.

**Saída obrigatória:** matriz componente × SG-1000 × SMS × implementação × teste × evidência × pendência; baseline dos dois sistemas reproduzível.

### Fase 1 — Unificar o motor Z80

**Prioridade técnica máxima após o baseline.**

A auditoria comparativa encontrou 3.257 linhas executáveis normalizadas idênticas entre os executores SG-1000 e SMS, com maturidade de testes superior no SMS. Usar o SMS como origem da extração é uma estratégia de redução de risco, não uma afirmação de que todas as semânticas já estão exaustivamente validadas.

- Criar/estabilizar o crate compartilhado `sega-z80-cpu`.
- Consolidar decoder, execução, flags e integração com ALU/bus.
- Portar a suíte Z80 SMS para o crate compartilhado e criar adaptadores para ambos os consoles.
- Remover implementações duplicadas somente quando SG-1000 e SMS estiverem consumindo o mesmo crate.
- Preservar contratos públicos necessários aos cores e à extensão PyO3.

**Aceitação:** uma única implementação de CPU em produção; testes comuns passam; os dois adaptadores mantêm seus testes de integração; nenhum desvio semântico não explicado.

### Fase 2 — Fechar lacunas semânticas do Z80

- BASE, CB, ED, DD/FD e DDCB/FDCB.
- Flags, prefixos repetidos/alternados e wraparound.
- Registradores I/R, refresh, WZ/MEMPTR e comportamentos não documentados quando sustentados por evidência.
- INT/NMI, IM 0/1/2, EI delay, HALT, RETI/RETN e reset.
- Acessos de memória/I/O, stack, instruções de bloco e efeitos colaterais.
- Vetores fixos, testes de fronteira e diferencial independente; registrar ancestralidade dos oráculos.

**Aceitação:** matriz de cobertura semântica por opcode/família, resultados reproduzíveis e regressão de ambos os consoles. Cobertura de execução não deve ser rotulada como cobertura semântica exaustiva.

### Fase 3 — Memória, barramentos, mappers e I/O

- Consolidar contratos comuns de transação, leitura/escrita e portas.
- Implementar adaptadores de barramento específicos SG-1000 e SMS.
- Validar mapas, espelhamento, ROM/RAM/VRAM, decodificação e limites.
- Reconciliar mappers/perfis já existentes e priorizar apenas os necessários aos jogos de teste.
- Modelar registradores com efeitos colaterais, interrupções, acknowledge, WAIT e arbitragem conforme evidência.
- Não introduzir mapper genérico nem fallback silencioso para perfil desconhecido.

**Aceitação:** testes isolados e integrados de endereços, dados, reset, I/O, interrupções e cartuchos nos dois sistemas.

### Fase 4 — VDPs independentes e compatibilidade visual

**SG-1000 / TMS9918A:** modos, VRAM, registradores, sprites, colisão/overflow, status, VBlank e IRQ.

**SMS / VDP Mode 4:** scroll, sprites e limites, VRAM/CRAM, status, IRQ horizontal/vertical, composição e efeitos raster.

- Não compartilhar semântica de VDP.
- Criar cenas sintéticas determinísticas para limites e prioridades.
- Investigar a primeira divergência de framebuffer, não apenas hash final.
- Integrar o scheduler gradualmente aos eventos observáveis do VDP.

**Aceitação:** testes de modo, registradores, sprites, status e regressões de cenas reais/homebrew identificadas.

### Fase 5 — Áudio digital específico por console

- Preservar PSG SN76489AN do SG-1000 e perfil PSG do SMS separadamente.
- Compartilhar infraestrutura de eventos, mixer e saída somente quando os contratos forem equivalentes.
- Validar divisores, ruído, atenuação, latches, ordem de escrita, reamostragem e determinismo.
- Separar estado digital do chip, filtragem analógica aproximada e saída host.
- Integrar FM/YM2413 do SMS como etapa própria, condicionada à compatibilidade básica.
- Medir underruns, fila, cadência e deriva no backend efetivo.

**Aceitação:** vetores digitais fixos e testes de integração/saída; não declarar fidelidade analógica sem evidência.

### Fase 6 — Integração funcional ponta a ponta

Para cada console, validar power-on/reset, carregamento, CPU, memória, I/O, VDP, PSG, entrada, interrupções, apresentação e encerramento.

- Troca SG-1000 ↔ SMS sem estado residual.
- Pause mantém ROM/sessão; Stop persiste dados nativos e descarrega mídia, sem fechar Qt.
- Reset físico permanece específico do hardware.
- Erros de firmware/mídia/perfil devem ser explícitos.
- Regressões existentes precisam continuar passando.

**Aceitação:** `SHORT_PASS` nos dois sistemas e evidência de gameplay smoke com homebrew/legalmente obtidas; nenhuma declaração universal.

### Fase 7 — Matriz de compatibilidade e robustez

- Ampliar títulos de teste por modos de VDP, mappers, áudio, controles e regiões.
- Testes longos, troca de mídia, reset, pausa/retomada e determinismo.
- Registrar título/dump/hash/perfil/configuração, cenário, resultado e limitações.
- Classificar cada jogo com `BOOT_CONFIRMED`, `GAMEPLAY_SMOKE`, `REGRESSION_PASS` ou `VALIDATED`.

**Aceitação:** matriz reproduzível, `LONG_PASS` onde aplicável e regressões sem falhas abertas não justificadas.

### Fase 8 — Scheduler mestre e precisão temporal

- Reavaliar e aproveitar a fundação temporal existente (incluindo a PR #315 como referência histórica, não como aprovação automática).
- Estabelecer scheduler por máquina, domínios de clock e relações inteiras/racionais determinísticas.
- Introduzir eventos de barramento, M-cycles, T-states, waits, refresh, interrupções e arbitragem.
- Integrar CPU, VDP, PSG e DMA sem schedulers independentes.
- Manter APIs coarse-grained para Qt e stepping de instrução apenas como ferramenta/fachada.

**Aceitação:** traces determinísticos e comparação independente. `TEMPORAL_PASS` somente com evidência apropriada; ciclos nominais agregados não bastam.

### Fase 9 — Integração e estabilização Qt/PyO3

- Reconciliar o modelo funcional Qt com a implementação já integrada.
- Não reescrever biblioteca, scanner, input, áudio ou apresentação se já houver serviços reutilizáveis.
- Centralizar comandos e estado de sessão; manter preferências separadas do estado da máquina.
- Migrar configurações com schema/versionamento, rollback e diagnóstico.
- Confirmar backend solicitado/ativo/fallback e capacidades reais.
- Testar abertura, troca, Pause, Stop, Reset, falhas e fechamento para SG-1000 e SMS.

**Aceitação:** build Release/Maturin, workflows aplicáveis, testes Qt/PyO3 e smoke test real nos dois sistemas no SHA candidato.

## 4. Critérios de engenharia por fatia

Antes de cada fatia: inventário, contrato esperado, casos-limite, testes e critérios de aceitação.

Durante: branch dedicada, alterações pequenas/coesas, sem sobrescrever trabalho concorrente; validar rapidamente após cada etapa.

Antes de concluir: revisão semântica, testes direcionados, suíte ampla, integração, workflows do SHA candidato e evidência de gameplay quando afetar execução.

Classificar workflows como `PASS`, `FAIL`, `BLOCKED` ou `NOT RUN`. Check ausente, ignorado ou executado em SHA anterior não é aprovação.

### Comandos obrigatórios

**Rust**
```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

**Python**
```powershell
python -m pytest
python -m ruff check .
python -m ruff format --check .
python -m compileall -q src tests
python -m pip check
```

Executar build Release/Maturin e integração quando pertinente. Se lint impedir pytest, marcar pytest como `NOT RUN`. Após correção, repetir as verificações afetadas.

## 5. Política de branches e PRs

- `main` é a fonte da verdade; verificar SHA remoto antes de cada ciclo.
- Mudanças não triviais entram por branch e PR.
- Não fazer force-push, reset destrutivo ou merge sem autorização.
- Não encerrar PRs de código por parecerem redundantes sem comparar os diffs e preservar alterações únicas.
- PRs documentais devem declarar claramente que não alteram runtime nem comprovam gameplay.
- Máximo de três agentes simultâneos, com funções distintas e sem edições sobrepostas.

## 6. Frentes explicitamente adiadas

Até o fechamento funcional SG-1000 + SMS, ficam adiados: Game Gear como plataforma de gameplay independente, Mega Drive, Sega CD, 32X, SVP, otimizações especulativas, JIT, paralelismo e campanha completa de precisão ciclo a ciclo. A documentação de arquitetura futura pode permanecer, mas não deve deslocar a prioridade de implementação.

## 7. Estado inicial e primeira sequência executável

No baseline `3a9adad1a9f7aea2ca8be4c8dde4faf41431db29`, as PRs documentais #298, #316, #317, #318, #320 e #321 registram decisões úteis que devem ser consolidadas sem duplicar conteúdo. A auditoria Z80 comparativa reporta 3.257 linhas executáveis normalizadas idênticas nos dois cores e suíte de testes SMS mais madura; confirmar esses números em nova auditoria do SHA atual antes de usá-los como critério de remoção.

Ordem imediata:
1. Congelar baseline e matriz SG/SMS.
2. Revisar o estado real dos crates Z80/ALU/bus e testes.
3. Definir API do crate compartilhado e plano de compatibilidade.
4. Portar os testes antes de remover executores duplicados.
5. Integrar os dois adaptadores e validar os dois sistemas.
6. Atualizar matriz e documentos com os resultados observados.

**Regra de saída do marco:** SG-1000 continua funcional e coberto por regressões; SMS executa jogos de teste com evidência; CPU Z80 compartilhada; arquitetura/documentação coerentes com o código real. Só então reavaliar o próximo sistema.
