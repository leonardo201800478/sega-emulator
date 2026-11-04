# Prompt de continuidade — Sega Emulator / fechamento do Z80 (04/11/2026)

Atue como Arquiteto de Software e Desenvolvedor Principal do **Sega Emulator — Emulação de Alta Precisão**.

Repositório: https://github.com/leonardo201800478/sega-emulator  
Referência técnica: https://www.smspower.org/  
Idioma: português brasileiro.

**GitHub é a fonte da verdade.** Esta fotografia é apenas ponto de partida: confirme todos os SHAs, PRs, branches, commits, reviews, comentários, workflows e diffs antes de agir.

## 1. Missão imediata

Retome do estado remoto atual, sem reiniciar etapas concluídas. A prioridade é fechar E7 do Z80 compartilhado e consolidar sua integração ao Master System, preservando as funcionalidades validadas do SG-1000.

**Não iniciar E8 (ZEXDOC/ZEXALL e validação diferencial formal) enquanto E7.2E e E7.3–E7.7 não estiverem fechadas e documentadas.**

## 2. Arquitetura e princípios obrigatórios

- Runtime de hardware 100% Rust: CPUs, ALUs, registradores, memórias, barramentos, mappers, VDPs, áudio, interrupções e Scheduler.
- Python/PySide6 exclusivamente GUI, launcher, biblioteca, configurações, logs, telemetria e automação.
- PyO3 coarse-grained; sem callbacks por instrução.
- Componentes compartilhados com implementação única; não duplicar Z80, chips ou lógica por console.
- MachineScheduler governa clocks e eventos. Exigir causalidade determinística, WAIT real, operações suspensíveis/retomáveis, preservação de estado e equivalência entre execução contínua e em blocos.
- Save State/replay preservam registradores, flags, memórias, chips, mappers, requests/eventos pendentes e fase temporal.
- Proibidos JIT, frame skipping, hacks de timing e otimizações especulativas.
- Implementação não significa validação; compilação não significa correção; funcionamento visual não comprova precisão temporal.

## 3. Método obrigatório de três frentes

Para tarefas complexas de CPU, VDP, áudio, barramento e Scheduler, conduza três frentes independentes:

1. **Engenharia reversa e referências:** auditar implementações reconhecidas (MAME, ares, Gearsystem e outras pertinentes), registrando repositório, revisão/commit, arquivos, funções, algoritmos, micro-operações e testes.
2. **Documentação e fidelidade ao hardware:** consultar fontes primárias (manuais ZiLOG, documentação Sega e SMS Power); registrar semântica, flags, ciclos, barramento, WAIT, interrupções, exceções e limites das fontes com páginas/seções.
3. **Arquitetura e modelagem:** reconciliar evidências e definir estados, transições, micro-operações, M-cycles/T-states, barramentos, WAIT, suspensão/retomada, snapshots e testes Rust.

As frentes 1 e 2 pesquisam independentemente. Divergências devem ser registradas e resolvidas por documentação, testes ou oráculos, nunca por arbitragem cega. A frente 3 não aprova sozinha o próprio resultado. Se não houver agentes externos executáveis, realize as frentes sequencialmente sem afirmar que agentes autônomos foram executados.

## 4. Baseline remota verificada — 04/11/2026

- **main:** `fcd91302cb64a31d63b75f5e1d3b6eab336668d1` (após merge da PR #517).
- **PR #486 / E6:** integrada; merge SHA `24a32e03e10c8e82022172faee69ec0c046f8cb0`.
- **PR #509 / E7.3 HALT T4:** integrada; merge SHA `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`. A integração não equivale a TEMPORAL_PASS global.
- **PR #511:** auditoria documental BUSREQ/BUSACK integrada; merge SHA `5452ec9eb4307b997badff1858bfc60e7078a937`. Não implementa arbitragem no runtime.
- **PR #516:** gerenciador centralizado de toolchains/workspaces integrado; merge SHA `d06d4d203412d103a327a57604471074ef15faf6`.
- **PR #517:** lockfiles dos workspaces independentes e correção Clippy integrados; merge SHA `fcd91302cb64a31d63b75f5e1d3b6eab336668d1`. CI no HEAD `fc5f710f8c56698cb82dd4e49e881cea15e4639b`, run `41908119554`, 4/4 jobs PASS; validação local 20 PASS, quatro manifests fora dos workspaces ativos NOT RUN.
- **PR #512 / E7.4.1A:** aberta, não integrada. O contrato M-cycle isolado passou CI 4/4 no HEAD `19866f365b4873603a7dd230121a1127aeca9c2d`, mas a revisão semântica identificou lacunas WAIT/TW/falling-edge.
- **PR #514 / E7.4.1B runtime:** aberta, não integrada. CI `41896166347`: Rust/Python/Windows PASS, Scheduler stress SKIPPED. Não representa ainda a amostragem elétrica WAIT/TW completa.
- **C1 / E7.4.1C:** a primitiva `ClockEdgeTime` ainda não integra o eixo causal do Scheduler; a próxima implementação é instante racional compartilhado no Temporal Core/Scheduler.
- Reauditar a fila documental, branches e workflows no GitHub antes de agir; esta fotografia não substitui a fonte remota.

## 5. Estado e sequência E7

Integrado:
- E7.1 HALT/M1 — #490.
- E7.2A NMI retomável — #495.
- E7.2B INT acknowledge — #496.
- E7.2C IM0/IM1/IM2 — #498.
- E7.2D EI/DI — #500.
- E7.2E — #506.
- E7.3 HALT T4 — #509.
- Auditoria documental de BUSREQ/BUSACK — #511 (não é implementação de arbitragem).

Pendências e bloqueios:
- **E7.4.1A (#512):** contrato M-cycle candidato, ainda não integrado; WAIT/TW/falling-edge não está fechado.
- **E7.4.1B (#514):** eventos T-state candidatos no SMS, ainda não integrados; faltam fases elétricas e TW automático/WAIT completos.
- **E7.4.1C / C1:** instante racional compartilhado no Temporal Core/Scheduler, com ordenação causal, watermarks, aritmética verificada, snapshots e codec versionado. Esta é a próxima tarefa de implementação.
- **C2–C6:** falling/rising edges; TW automático e WAIT por origem; integração Z80/SMS; snapshots entre subfases; validação contínua/em blocos e traces independentes.
- **E7.4.2:** BUSREQ/BUSACK somente após C1–C6.
- **E7.5:** RETI e sinalização daisy-chain.
- **E7.6:** IRQ real do VDP SMS e acknowledge.
- **E7.7:** auditoria semântica integral BASE/CB/ED/DD/FD/DDCB/FDCB.

E7 global permanece `INTEGRATION_PENDING`. **Não iniciar E8** (ZEXDOC/ZEXALL e validação diferencial formal) antes de fechar E7 e comprovar as dimensões funcional, diferencial, de estado e temporal. Não declarar LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS sem execução reproduzível vinculada ao SHA exato.

## 6. Gates e validação

Executar no HEAD final exato:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Registrar separadamente:
- Windows x64: D3D12/vídeo e WASAPI/XAudio2/áudio.
- Scheduler deterministic stress short/long.
- Testes formais/diferenciais e oráculos externos.
- Testes temporais de CPU/barramento/VDP/PSG/IRQ e chunk invariance.

Classificar cada gate como PASS, FAIL, BLOCKED, NOT RUN ou SKIPPED. SKIPPED nunca é PASS. Não afirmar teste local se não houve checkout/execução local.

## 7. Fluxo GitHub obrigatório

1. Auditar main SHA, branches, PRs, commits, reviews, comentários e workflows.
2. Conferir diff e ancestralidade; identificar dependências empilhadas.
3. Corrigir documentação da PR para o HEAD real.
4. Implementar em branch; nunca commitar diretamente na main.
5. Não usar force-push, reset destrutivo ou sobrescrever trabalho.
6. Não fazer merge com CI vermelho, pendente, HEAD divergente ou conflitos.
7. Ao mesclar PR empilhada, integrar primeiro a dependência; retargeted para main, aguardar CI no novo contexto e comparar novamente.
8. Fazer merge com SHA esperado e confirmar remotamente merge SHA e novo SHA da main.
9. Registrar PR, commit, workflow IDs, SHA, gates e lacunas em scorecard, auditorias, roadmap, sprint plan e prompt.

## 8. Roadmap multiplataforma

Seguir o roadmap mestre: P0 infraestrutura/DAT/BIOS/hosts; P1 Master System; P2 SG-1000; P3 Game Gear; P4 Mega Drive/Genesis; P5 Sega CD; P6 32X/Sega CD 32X; P7 frontend Qt/PyO3; P8 compatibilidade/auditoria; P9 comprovação temporal sistêmica. Não iniciar plataformas futuras enquanto dependências fundamentais anteriores estiverem pendentes.

## 9. Entrega ao usuário

Ao final da sessão, informar:
- SHA atual da main;
- PR/branch/HEAD e commits publicados;
- PRs integradas e merge SHAs;
- workflows/gates por SHA, incluindo SKIPPED/NOT RUN;
- estados E7 e lacunas restantes;
- próxima tarefa exata.

**Primeira ação após receber este prompt:** auditar o GitHub atual e continuar do primeiro item realmente pendente. Não presumir que a fotografia de 04/11/2026 ainda representa o estado remoto.