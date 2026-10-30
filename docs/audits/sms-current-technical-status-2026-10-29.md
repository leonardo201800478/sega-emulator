# Estado técnico atual — Sega Master System (SMS)

**Data da fotografia:** 2026-10-29  
**Repositório:** `leonardo201800478/sega-emulator`  
**Branch-base auditada:** `main`  
**SHA-base exato:** `b4a85c82b1d975a126f1dcbd3084dac51249b398`  
**Commit-base:** `fix(sms-vdp): correct scroll and status semantics`  
**Escopo:** leitura do código e da documentação presentes nesse SHA. Não é uma validação de hardware real nem substitui a execução dos testes.

> Esta página substitui, para fins de fotografia do estado atual, os resumos SMS anteriores que apontavam para SHAs anteriores à integração do runtime SMS em Qt. Os documentos históricos continuam preservados, mas seus resultados pertencem às bases e candidatos que cada um identifica.

## Resumo executivo

O projeto já possui um **runtime SMS nativo em Rust integrado à aplicação Qt/PySide6**. A implementação deixou de ser apenas uma fundação isolada: há caminho de máquina que conecta CPU Z80, memória/cartucho, I/O, VDP e PSG, com adaptador Python fino para a extensão PyO3.

Isso representa integração funcional de subsistemas, mas **não** significa que o SMS esteja concluído em fidelidade ou temporalidade. O VDP trabalha com modelo determinístico orientado a scanlines, a CPU reporta T-states nominais por instrução e a arquitetura temporal ainda não representa integralmente fases de barramento, esperas e ordenação de eventos entre todos os dispositivos.

## Estado por subsistema

| Subsistema | Estado observado no SHA-base | Limite conhecido / próximo gate |
|---|---|---|
| Runtime SMS em Rust | **IMPLEMENTED** — crate `native/sms_core` e máquina `SmsMachine` integram os componentes SMS | Ampliar validação de integração e compatibilidade de jogos; não declarar conclusão global |
| CPU Z80 SMS | **IMPLEMENTED** — executor nativo com registradores principais/alternativos, IX/IY, I/R, IFF, modos de interrupção, HALT, NMI/INT, WZ/Q/P e contagem nominal de T-states | Semântica global ainda requer vetores independentes e cobertura de casos-limite; T-states agregados não são precisão de barramento |
| Semântica Z80 | **QUICK_PASS / SHORT_PASS** apenas nas fatias cobertas pelos testes e workflows históricos identificados nos documentos Z80 | Não inferir cobertura integral a partir da matriz de opcodes ou do número de testes; revisar flags, aliases, prefixos, interrupções e estados |
| Trace temporal Z80 | **IMPLEMENTED** para um subconjunto delimitado de instruções ED de bloco, com amostras T-state/M-cycle | Cobertura parcial; não modela todos os ciclos/pinos, waits ou o scheduler completo |
| Cartucho e memória | **IMPLEMENTED** — imagem de cartucho, mapper padrão, memória e RAM de trabalho integrados à máquina | Confirmar compatibilidade de mappers/dumps e metadados por ROM; não presumir suporte a todos os mappers comerciais |
| I/O e controles | **IMPLEMENTED** — portas SMS, dois controles ativos em nível baixo e caminho de light phaser | Validar combinações, bordas e periféricos em hardware/oráculo; botão Pause físico do console não deve ser confundido com pausa do host |
| VDP SMS | **IMPLEMENTED** — Mode 4, VRAM/CRAM, registradores, scroll, sprites, status, contadores H/V, IRQ, light phaser e estado SegaScope | Modelo de renderização por scanline; não é dot/pixel-cycle exact. Efeitos raster intralinha continuam sendo risco de compatibilidade |
| PSG SN76489 | **IMPLEMENTED** — implementação SMS própria, escrita por portas, geração determinística de amostras, buffer e filtro digital | Não equivale a modelagem analógica completa; YM2413/FM não está implementado como sintetizador ativo |
| FM / YM2413 | **INTEGRATION_PENDING** | Configuração distingue presença de FM, mas a síntese FM e sua mixagem ainda são pendências |
| PAL/NTSC | **IMPLEMENTED** no nível de configuração/clock e geometria de raster | Precisão temporal global e comportamento por modelo/revisão ainda precisam de validação |
| Scheduler da máquina | **INTEGRATION_PENDING** para precisão temporal completa | A CPU avança por instrução e o VDP recebe avanço agregado; ainda falta um scheduler mestre com arbitragem/ordem de eventos e waits suficientemente demonstrados |
| Qt/PySide6 | **IMPLEMENTED** — adaptador `SMSMachine` encaminha operações para `NativeSMSMachine`; há seleção de sistema e integração de áudio/vídeo | Validar build Maturin/instalação real e execução no Windows no SHA candidato; manter Python fora do caminho por instrução |
| BIOS externa SMS | **NOT IMPLEMENTED / não comprovada neste SHA** | Não documentar como recurso disponível até existir carregamento, mapeamento, configuração e testes; ROM de cartucho continua sendo o caminho observado |

## Evidências no código

- `native/sms_core/src/lib.rs`: expõe módulos de CPU, barramento, cartucho, configuração, entrada, I/O, máquina, memória, PSG e VDP.
- `native/sms_core/src/machine.rs`: integra Z80, memória, I/O, VDP e PSG; encaminha acessos de portas e avança o raster a partir dos T-states da instrução.
- `native/sms_core/src/cpu.rs`: executor Z80 SMS; documenta explicitamente que os tempos reportados são nominais e não constituem precisão ciclo a ciclo.
- `native/sms_core/src/vdp.rs`: declara modelo determinístico por scanline, Mode 4 e ausência de precisão dot-cycle.
- `native/sms_core/src/psg.rs`: PSG SMS independente, com geração digital de amostras e filtro FIR.
- `native/sms_core/src/config.rs`: separa território, padrão de vídeo, presença de FM e política de limite de sprites. O padrão atual é Export/NTSC, FM ausente e limite físico de sprites ativo.
- `src/sega_emulator/machine/sms.py`: adaptador Python fino que instancia `NativeSMSMachine` e encaminha `step()`; CPU, VDP e áudio permanecem no runtime nativo.
- `src/sega_emulator/frontend/qt_frontend.py` e módulos Qt SMS: integração com a interface e configurações de entrada.

## Testes e validação: como interpretar

O repositório contém testes Rust direcionados para ALU, semântica de opcode, prefixos, fluxo de controle, interrupções, memória, I/O, VDP, PSG e integração. A existência de um teste é evidência de cobertura daquele contrato específico; não é aprovação automática do subsistema inteiro.

Os documentos de progresso Z80 registram resultados de candidatos e workflows anteriores, incluindo as PRs #227–#229. Esses PASS permanecem vinculados aos SHAs e workflows citados na documentação histórica. **Não são transferidos para o SHA-base desta fotografia.**

Para uma mudança futura, executar e registrar no mesmo SHA candidato:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `python -m pytest`
- `python -m ruff check .`
- `python -m ruff format --check .`
- `python -m compileall src tests`
- `python -m pip check`
- build Release da extensão via Maturin e testes de integração Qt/PyO3 pertinentes.

**Resultado desta PR documental:** testes de runtime e build não foram executados por esta alteração; classificar como **NOT RUN**. Checks de CI devem ser consultados no SHA exato da nova PR, sem reutilizar resultados de candidatos anteriores.

## Lacunas prioritárias para fechar o SMS

1. **Z80 semântico:** continuar a matriz de instruções com vetores fixos, cobrindo flags documentadas/não documentadas, prefixos, wraparound, estados especiais, HALT e interrupções. Registrar origem, versão, licença e ancestralidade dos oráculos.
2. **Interrupções e barramento:** completar contratos de NMI/INT, acknowledge, EI delay, refresh, acessos e esperas; distinguir estado arquitetural de sequência elétrica/temporal.
3. **Scheduler mestre:** evoluir de avanço agregado por instrução para coordenação explícita de T-states, acessos ao barramento, VDP, PSG e eventos de interrupção, sem declarar precisão ciclo a ciclo antes dos traces comparativos.
4. **VDP raster:** investigar efeitos que dependem de alterações de registradores/scroll durante a linha; criar repros determinísticos e comparar a mesma cena com oráculo independente.
5. **Áudio:** validar PSG por vetores e capturas de referência; implementar FM/YM2413 apenas quando a arquitetura e os perfis de hardware estiverem definidos.
6. **Compatibilidade de software:** manter uma matriz de ROMs legais/homebrew e dumps fornecidos pelo usuário, registrando tamanho/hash localmente e resultados reproduzíveis. Nunca versionar ROM comercial ou BIOS protegida.
7. **Integração Windows:** provar no SHA candidato a extensão efetivamente importada, build Release/Maturin, troca de sistema, áudio/vídeo e controles em execução manual.

## Classificação consolidada

| Estado | Classificação atual |
|---|---|
| Componentes CPU, memória/cartucho, I/O, VDP e PSG | **IMPLEMENTED** |
| Testes direcionados existentes | **QUICK_PASS / SHORT_PASS** somente nos escopos/candidatos explicitamente registrados |
| Integração funcional Qt/PyO3 | **IMPLEMENTED** no código; validação Windows do candidato deve ser registrada separadamente |
| Scheduler e temporização integral | **INTEGRATION_PENDING** |
| Precisão ciclo a ciclo do SMS | **TEMPORAL_PASS NÃO DECLARADO** |
| Completude semântica global Z80 | **NÃO DECLARADA** |

## Fontes internas e histórico

- [Roadmap SMS](../architecture/master-system-roadmap.md) — decisões arquiteturais e histórico de evolução.
- [Modelo temporal do barramento Z80](../architecture/z80-temporal-bus-model.md) — escopo e limites dos traces.
- [Matriz semântica Z80](../architecture/z80-semantic-matrix.md) e [matriz CSV](../architecture/z80-sms-opcode-matrix.csv).
- [Progresso Z80 até PR #197](../architecture/z80-progress-through-pr-197.md) — histórico vinculado aos SHAs e candidatos daquela fase.
- [Auditoria de cobertura de opcode](../architecture/z80-opcode-space-completion-audit.md).

Esta fotografia deve ser atualizada quando o SHA de `main` mudar de forma relevante para o SMS. Sempre registrar o novo SHA, arquivos inspecionados, evidências, resultados e limites; nunca substituir silenciosamente um snapshot histórico.
