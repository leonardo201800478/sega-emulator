# Estado atual dos componentes — Sega Emulator

**Fotografia reconciliada:** `main` em `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c` (2026-11-06; PRs #595, #596 e #597 integradas).  
**Política de fidelidade:** [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).  
**Prioridade:** comprovar o caminho de áudio nativo e recuperar gameplay SMS, preservando o SG-1000.

Esta página é a fotografia do SHA indicado. PRs #595 e #597 estão integradas; #596 reconciliou documentação. A PR #594 foi fechada como SUPERSEDED. PRs abertas ou Draft continuam fora da baseline.

## 1. Plataformas

| Plataforma | Estado | Diretriz |
|---|---|---|
| SG-1000 | Runtime Rust integrado | Preservar funcionalidades e jogos validados; executar regressões em toda alteração compartilhada |
| Master System | Runtime Rust temporal integrado à Qt/PyO3 | Prioridade imediata: comprovar imagem e som, corrigir boot e validar em Windows real |
| Game Gear | Não concluído como plataforma independente | Fase futura; reutilizar SMS onde houver equivalência |
| Mega Drive / Genesis | Não concluído | Fase futura |
| Sega CD / 32X | Não concluídos | Dependem das plataformas anteriores |

## 2. Estado dos componentes na main reconciliada

| Componente | Estado observado | Limite / próxima evidência |
|---|---|---|
| Z80 compartilhado | Implementado em Rust; executor atômico e sequenciador retomável usados pelo SMS temporal | Fechamento semântico E7.7 ainda não comprovado; não reabrir o Z80 sem divergência reproduzível |
| Z80 E5 / E6 | Integrados na main conforme scorecard e matriz DD/FD | Evidências continuam vinculadas aos testes/SHAs registrados nesses documentos |
| Z80 E7 | Parcial; HALT T4, reset e testes de interrupção SMS possuem cobertura direcionada | Auditoria semântica global ainda aberta |
| Memória / mapper SMS | Integrados ao `SmsTemporalBus` | Compatibilidade de cartuchos deve ser validada com ROMs legais/homebrew e metadados locais |
| VDP SMS | `SmsVdp` compartilhado em `sega_video::sms`, conectado ao barramento temporal; portas, VRAM/CRAM, Mode 4 e composição por scanline | Não é pipeline dot/fetch; pixels de jogos reais ainda precisam de comprovação ponta a ponta |
| IRQ do VDP → INT do Z80 | Integrado pela PR #595: wired-OR da IRQ de nível do VDP com INT externa; sincronização após dispatch e antes de instrução; vetor de acknowledge coerente | Testes de VBlank/IM1/HALT, fonte externa, acknowledge e restore PASS no CI #5178; Line IRQ ainda exige validação de host/jogo |
| PSG SN76489 | Integrado ao `SmsTemporalBus` via `ScheduledPsg`; escritas de porta e geração de amostras seguem avanço do Scheduler | Geração de amostras não prova saída física audível |
| Scheduler SMS | `SmsTemporalMachine` despacha eventos e o barramento aplica efeitos nas fronteiras programadas | VDP ainda avança em modelo scanline; WAIT físico, fetch arbitration e precisão intralinha não estão comprovados |
| Vídeo / mailbox | Worker Rust publica frames RGB888 de 256×192 no `FrameMailbox` | Teste sintético exige RGB `[255,255,255]` na linha 32 tanto no core quanto no mailbox: PASS no CI #5178; D3D12 físico ainda NOT RUN |
| D3D12 | Sessão nativa Rust pode anexar ao HWND do viewport Qt e consumir frames do mailbox | CI Windows compila/testa contratos; GPU física, imagem de jogo e device-loss real permanecem NOT RUN |
| Áudio host | Worker drena PCM do PSG e encaminha à `NativeAudioSession`; sessão mistura/resample e entrega a `PcmSink` | Worker → sessão → `RecordingSink` com PCM não silencioso PASS no CI #5185; `NativeAudioBackend` real/WASAPI/XAudio2 e audição física permanecem NOT RUN |
| PyO3 / Qt | API coarse-grained; comandos de ciclo de vida e configurações permanecem no frontend | Não transportar pixels/PCM nem temporizar hardware em Python |

## 3. Caminho efetivo do runtime SMS

No SHA-base, o caminho de produção é:

`MinimalSmsWindow` → `TemporalSMSMachine` / PyO3 → `SmsTemporalWorker` → `SmsTemporalMachine` → `SmsTemporalBus` → `SmsVdp + ScheduledPsg`.

- O worker Rust executa a máquina e publica o framebuffer RGB888 no mailbox de vídeo.
- A sessão D3D12 consome frames completos em thread nativa separada.
- O worker drena amostras do PSG e as encaminha à sessão de áudio quando configurada.
- O frontend configura WASAPI por padrão e permite XAudio2/áudio desativado; falha de endpoint deve ser informada sem interromper a execução do vídeo.
- A máquina temporal não declara que todos os jogos inicializam corretamente; a cobertura de frame existente antes da correção A/V validava dimensão e avanço, não conteúdo não preto nem amostras PCM não silenciosas.

## 4. Gates de funcionalidade A/V

Os seguintes critérios devem ser tratados separadamente:

| Gate | Critério | Estado na main indicada |
|---|---|---|
| A/V-01 | ROM sintética configura VDP/VRAM/CRAM e produz pixel RGB esperado | PASS — teste Rust sintético no CI #5178 |
| A/V-02 | ROM sintética programa o PSG e produz amostras PCM não silenciosas | PASS — teste Rust sintético no CI #5178 |
| A/V-03 | Worker publica frame RGB888 real do VDP no mailbox e sequência avança | PASS — pixel esperado preservado no mailbox no CI #5178 |
| A/V-04a | Worker → `NativeAudioSession` → sink recebe PCM não silencioso antes do flush | PASS — teste `worker_frame_submits_non_silent_pcm_to_native_audio_session_sink` no CI #5185; 0 underruns/blocks dropped no sink determinístico |
| A/V-04b | `NativeAudioBackend` WASAPI/XAudio2 real aceita PCM do worker sem erro/underrun sustentado | NOT RUN — sink em memória não prova aceitação do backend nativo |
| A/V-05 | Janela Windows apresenta imagem de jogo em GPU física e áudio é audível | NOT RUN — sem evidência de GPU/endpoint físicos |

A PR #595 foi integrada no merge SHA `733152f61b706b2243914ae017643cef949d4ae1`; a PR #597 foi integrada no merge SHA `0fb09e2cad46a92aefe6a98ca07549a5ba91d78c`. CI #5178 valida o HEAD #595 `69ca40ab29c486f26852adb7536a04f95f9dd7ae`; CI #5185 valida o HEAD #597 `30554cc6642dd49d06adf495dca3e80f4822b081`. A/V-01/02/03/04a são PASS automatizados; A/V-04b/05/06 permanecem NOT RUN.

## 5. Prioridades de correção

1. Provar PCM não silencioso aceito por `NativeAudioBackend` WASAPI/XAudio2, incluindo contagem de blocos, erros, underruns e dropped blocks; o sink em memória da #597 não fecha esse gate.
2. Validar manualmente no Windows a apresentação de frames D3D12 em GPU física e estabilidade da sequência de frames.
3. Validar áudio audível em endpoint WASAPI/XAudio2 real, com taxa de amostragem, latência, underruns e erros registrados.
4. Executar ROM legal/homebrew e jogo representativo, registrando região, hash local, boot, input e comportamento observável; não versionar ROM comercial.
5. Validar VBlank/Line IRQ, controles, HALT e snapshots no host real, mantendo regressão SG-1000.
6. Provar execução contínua versus em blocos, save/restore e replay determinístico.
7. Medir custo da telemetria antes de habilitá-la em benchmark; observabilidade não pode ser confundida com desempenho de produção.

## 6. Modelagem temporal e precisão

- **Temporal prioritário:** I/O, registradores VDP/PSG, raster, mapper, IRQ/NMI e demais interações cuja ordem/duração altere gameplay.
- **Atômico padrão:** operações sem efeito intermediário observável; devem continuar semanticamente exatas e determinísticas.
- A ausência de fetch arbitration, WAIT físico ou dot-cycle não deve bloquear a primeira prova funcional de imagem e som.
- Não adicionar JIT, frame skipping, hacks de timing ou lógica de hardware em Python.
- Não declarar TEMPORAL_PASS, compatibilidade geral ou desempenho real sem evidência correspondente.

## 7. Validação

Executar no SHA candidato:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Classificar cada suíte como PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED nunca equivale a PASS. Testes Windows/D3D12, Qt/PyO3 e hardware real devem ser reportados separadamente.

## 8. Fontes de autoridade

- [Roadmap mestre](project-roadmap.md)
- [Escopo do projeto](project-scope.md)
- [ADR-0016 — fidelidade observável](architecture/adr/0016-observable-fidelity-and-proportional-timing.md)
- [Matriz de validação temporal VDP/PSG](architecture/sms-vdp-psg-temporal-validation.md)
- [Scorecard Z80](z80-completion-scorecard.md)
- [Matriz semântica DD/FD](z80-dd-fd-semantic-matrix.md)
- [Auditoria da baseline funcional A/V SMS — 2026-11-06](audits/sms-runtime-functional-baseline-2026-11-06.md)

Fotografias e auditorias datadas anteriores continuam como evidência histórica dos respectivos SHAs; não substituem esta fotografia reconciliada.
