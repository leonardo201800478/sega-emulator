# Auditoria de integração — D3D12 presentation runtime e SMS1 VDP contract tests

**Data:** 2026-11-05  
**Repositório:** `leonardo201800478/sega-emulator`  
**HEAD publicado em `main`:** `795ed98724cb82d350d84c93aa23c6be7cd36916`  
**Escopo:** PRs #546, #547, #550, #553, #555, #558 e #560.  
**Método:** merges GitHub sequenciais, preservando cada PR e sua dependência; sem commit direto na `main`.

## 1. Resultado executivo

As sete PRs foram incorporadas à `main` em 2026-11-05. Cada PR teve seu HEAD fixado por SHA, CI consultado e base retargetada para `main` somente depois que a dependência anterior foi integrada. Após a retarget, o GitHub recalculou `mergeable`; o merge só foi executado quando voltou a `true`. Os merges foram feitos pelo método `merge`, sem squash/rebase.

| PR | Entrega | HEAD validado | Merge commit |
|---:|---|---|---|
| #546 | FIFO limitada e lossless para frames concluídos | `6301a3428d7bf54aaabbedeeaf22960859180f87` | `9b08fc5646e9c67e849bd15f304e46083944e763` |
| #547 | Sessão D3D12 Rust-owned em worker dedicado | `06a86223a96e16c590438ad71e74485d3b708764` | `efd5302f8f07b19ab4891f24ddaf3aff3dab09cc` |
| #550 | Retenção FIFO durante minimize/restore e stretch DXGI | `5e28b609f33d6685b2a542bdcda52ce57af75cf8` | `90d44fe8bf0a57726f14eca9c6d36f4f70c64b93` |
| #553 | DPI por janela: PMv2 com fallback PMv1 | `d950d283208e8f585cbb3cb6e83e6ac464084747` | `87436d5b038c9f0897867363263f98c0bdf227ba` |
| #555 | Polling finito e detecção responsiva de device loss | `3b2c2a4dcfc1eb5654192a8be3da4761322f670f` | `07ea68d0964572adc11c70504f4aeb6072a108ed` |
| #558 | Recriação one-shot para DEVICE_REMOVED/RESET | `e39aaa299606e7533777a0bc096beeaebe67742f` | `4fff1c0ea68f85cb95ba6247537184c96b23c57a` |
| #560 | Testes externos de contrato para portas SMS1 VDP | `27481d44a3f2c0940143a70dd53dd0d7540e8485` | `795ed98724cb82d350d84c93aa23c6be7cd36916` |

Todas as PRs estão fechadas como merged. A última merge commit é também o HEAD de `main` verificado ao encerrar esta auditoria.

## 2. Detalhamento das entregas

### #546 — FrameQueue lossless
- Adiciona `FrameQueue`, FIFO limitada, com sequências monotônicas e preservação de todo frame aceito.
- Backpressure ocorre quando a fila está cheia; o produtor aguarda o consumidor em vez de substituir um frame.
- Suporta espera com timeout, fechamento e drenagem dos frames aceitos.
- O fechamento acorda produtores/consumidores.
- Testes cobrem FIFO, backpressure, fechamento, monotonicidade e timeout.
- `FrameMailbox` mantém semântica distinta de latest-frame/coalescing. O contrato lossless é a escolha para o caminho normal de apresentação.

**Fronteira temporal:** a fila recebe apenas frames completos. Espera por capacidade não pode ocorrer dentro de evento/ciclo do Scheduler.

### #547 — D3d12Session Rust-owned
- Cria `D3d12Host` e HWND em thread dedicada.
- A thread possui message pump, host D3D12, consumo da fila e destruição dos recursos.
- Consome `FrameQueue` em FIFO; frames aceitos não são substituídos.
- Present/fence waits ficam na thread de apresentação, não no Scheduler.
- Shutdown fecha a fila, drena os frames aceitos e junta a thread.
- Falha/fechamento da janela fecha a fila e contabiliza frames pendentes cancelados, sem declará-los apresentados.
- Expõe sequência do último frame apresentado e estado do worker.
- Inclui exemplo `d3d12_session_smoke`.

### #550 — Minimize/restore e resize
- Consulta `IsIconic` antes do dequeue e mantém mensagens Win32 sendo processadas enquanto a janela está minimizada.
- Não retira frames da FIFO durante minimize; após restore, apresenta na ordem original.
- Revalida o estado após dequeue para preservar o frame se a minimização ocorrer nessa janela de corrida.
- No shutdown minimizado, frames não apresentados são contabilizados como cancelados (`PendingFramesCancelled`).
- `DXGI_SCALING_STRETCH` adapta a superfície lógica ao client area. Não altera dimensões do framebuffer nem o tempo do Scheduler.
- Não introduz `ResizeBuffers`; a superfície DXGI permanece no tamanho lógico do frame.

### #553 — DPI por monitor
- Cria HWND com contexto DPI por janela: Per-Monitor V2 e fallback Per-Monitor V1.
- Restaura o contexto anterior do thread com guard RAII logo após a criação; não altera a política DPI global do processo.
- Trata `WM_DPICHANGED` aplicando o RECT sugerido via `SetWindowPos(SWP_NOZORDER | SWP_NOACTIVATE)`.
- Habilita non-client DPI scaling em `WM_NCCREATE` no fallback PMv1.
- Mantém dimensões lógicas do frame/swap chain e stretch scaling DXGI.

### #555 — Esperas responsivas e detecção de device loss
- Substitui waits infinitos do frame-latency handle e fence por polling de 16 ms.
- Bombeia mensagens Win32 durante os timeouts.
- Consulta `ID3D12Device::GetDeviceRemovedReason` para detectar perda/reset e preservar o HRESULT original.
- Trata `UINT64_MAX` de `GetCompletedValue` como sentinel de device removal, nunca como conclusão bem-sucedida da fence.
- Diferencia `DeviceLost` de erro comum de apresentação e mantém o erro Windows como source.
- Frames que não podem ser apresentados são contabilizados como cancelados.

### #558 — Recuperação one-shot de device removed/reset
- A recuperação automática é restrita a `DXGI_ERROR_DEVICE_REMOVED` e `DXGI_ERROR_DEVICE_RESET`.
- Descarta o host/recursos COM/HWND antigos antes de recriar janela, device, swap chain e recursos dependentes.
- Reutiliza a classe Win32 registrada, aceitando `ERROR_CLASS_ALREADY_EXISTS`.
- `WM_DESTROY` não publica `WM_QUIT` thread-wide; o message pump verifica `IsWindow(Some(hwnd))` para reconhecer o HWND corrente e permitir substituição no mesmo worker.
- Retenta exatamente o mesmo `VideoFrame` imutável uma única vez, sem mudar sequence/pixels.
- `DEVICE_HUNG` e `DRIVER_INTERNAL_ERROR` permanecem falhas não recuperáveis automaticamente.
- Falha na recriação ou no retry retorna `DeviceRecoveryFailed`, preservando o HRESULT inicial e o erro da recuperação.
- Expõe `device_recovery_count()`.
- Não executa/reinicia Scheduler, CPU cycles ou eventos durante a recuperação.

**Limite:** a recriação cria um HWND novo; posição/estado exato da janela pode mudar. Não há loop infinito de retry.

### #560 — Contrato funcional independente do SMS1 VDP

Adiciona `native/sms_core/tests/sms1_vdp_contract.rs`, usando somente a API pública de `Sms1Vdp` e expected values explícitos:
- protocolo de dois bytes do control port;
- cancelamento de control word incompleta após status read;
- cancelamento após data-port write;
- VRAM read-ahead e wrap `0x3FFF → 0x0000`;
- alias de CRAM para 32 entradas e máscara de 6 bits;
- escrita em R0–R10 e comportamento de R11 ignorado;
- espelhamento de portas `$BE/$BF`;
- leitura de status que retorna flags e as limpa.

Durante a revisão, foi corrigida a expectativa para R11: o `RegisterFile` expõe 16 slots crus; a escrita em R11 é ignorada e `get(11)` permanece `Some(0)`, não `None`. A correção ficou no teste, sem alteração do runtime.

**Limites:** estes testes validam semântica funcional/estado; não validam raster, H/V counter, fase, fetch slots, WAIT, IRQ ou contenção. Não usam o VDP legado como oráculo.

## 3. Validação automatizada e evidência

Todos os runs de CI associados aos HEADs acima terminaram com conclusão `success`:

| PR | Runs de CI |
|---:|---|
| #546 | `42037248669` |
| #547 | `42038013487` |
| #550 | `42040638346`, `42040620924` |
| #553 | `42041350552`, `42041327864` |
| #555 | `42042557025`, `42042532867` |
| #558 | `42043815445`, `42043779932` |
| #560 | `42045204770`, `42045168720` |

Os gates aplicáveis passaram: Rust fmt/Clippy/testes do workspace, regressões Python Ruff/pytest e compilação/testes Windows x64 do runtime D3D12. No CI de #560, o build de extensão PyO3 Python 3.15 também concluiu com sucesso.

**SKIPPED / NOT RUN no pipeline:** Qt smoke, stress determinístico do Scheduler e sintaxe de tooling PowerShell foram pulados conforme a classificação de impacto das mudanças. Os runs de PRs anteriores também tiveram o build PyO3 pulado quando não aplicável. Um job skipped não é PASS.

**NOT RUN fora do CI:**
- execução real dos exemplos de janela em desktop Windows com GPU física;
- remoção/reset forçado do adaptador e validação de recovery real;
- movimentação real entre monitores com DPI misto;
- preservação de posição/estado da janela após recriação;
- replay headless versus D3D12 e jogos comerciais/reais;
- integração da máquina SMS1 ativa com `VideoFrame` e `D3d12Session`;
- hardware oracle 315-5124.

O CI comprova compilação e testes automatizados nos ambientes dos runners; não comprova comportamento físico de GPU/janela.

## 4. Estado arquitetural após os merges

O caminho de apresentação nativo possui agora:
- contrato de frame e fila FIFO lossless;
- sessão Rust-owned em worker dedicado;
- tratamento de minimize/restore e DPI por janela;
- waits finitos e detecção de device loss;
- recuperação one-shot para removed/reset;
- telemetria de sequência, cancelamento e contagem de recovery.

Isso **não significa que a emulação SMS1 esteja conectada ao D3D12**. A cadeia `SmsMachine → MachineScheduler → Sms1Vdp → VideoFrame → D3d12Session` continua pendente. O `SmsMachine` ainda usa o VDP legado; `SmsTemporalMachine` não conecta VDP/PSG ao Scheduler; o novo `Sms1Vdp` permanece funcional por scanline sem raster advance, fetch arbitration, IRQ temporal ou WAIT.

O gate T0/T0.1 do ASIC 315-5124 continua parcial. Fase VDP/Z80, H-counter, linha/frame por região, slots de fetch, contenção e WAIT real não podem ser preenchidos com constantes inferidas de Game Gear, VDP legado ou outro emulador. A PR #560 cobre somente uma parte dos contratos funcionais.

Referências normativas:
- [ADR-0012 — Runtime de vídeo nativo em Rust com Direct3D 12](../architecture/decisions/ADR-0012-native-d3d12-video-runtime.md)
- [ADR-0016 — Fidelidade observável e precisão temporal proporcional](../architecture/adr/0016-observable-fidelity-and-proportional-timing.md)
- [Migração do runtime de vídeo nativo](../architecture/migrations/native-video-runtime.md)
- [Consolidação Scheduler/SMS1/D3D12](../architecture/scheduler-sms1-vdp-d3d12-consolidation.md)
- [Caracterização T0 do VDP SMS1 315-5124](sms1-315-5124-vdp-t0-characterization-2026-11-02.md)
- [Matriz de validação T0.1](sms1-315-5124-vdp-t01-validation-matrix-2026-11-02.md)

## 5. Próximos passos obrigatórios

1. Executar `cargo run --release --locked -p sega-video-runtime --example d3d12_session_smoke` em Windows x64 com GPU D3D12 física.
2. Testar minimize/restore, resize, DPI PMv2/PMv1 e movimento entre monitores com escalas diferentes.
3. Injetar device removed/reset em ambiente controlado e confirmar recriação + retry único do mesmo frame; testar também DEVICE_HUNG/DRIVER_INTERNAL_ERROR como falhas explícitas.
4. Confirmar que fechamento de janela e falha de recovery contabilizam todos os frames não apresentados.
5. Produzir evidência de hardware 315-5124 para linha/frame, fase, H/V counters, fetch/contenda e WAIT antes de integrar o VDP temporal ao Scheduler.
6. Só então ligar a máquina SMS1 ao frame sink/session e provar equivalência headless versus apresentação, sem alterar MachineTime.
7. Atualizar scorecards/roadmap somente com evidência; classificar cada suíte como PASS, FAIL, BLOCKED ou NOT RUN.

**Conclusão:** as sete PRs de infraestrutura de apresentação e testes funcionais foram publicadas na `main`; a integração temporal do SMS1 e a validação em hardware permanecem explicitamente abertas.
