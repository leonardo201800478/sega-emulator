# Estado do projeto — 2026-11-11

**Repositório:** https://github.com/leonardo201800478/sega-emulator  
**Main auditada:** 392a5c67e85a11a5a980d8f7055bcc8a12fe3784  
**Escopo:** estado remoto do GitHub e PRs relevantes; não representa execução local da POC.

## Resumo

O runtime Rust e a integração SMS na main devem ser preservados. A arquitetura desktop alvo definida na ADR 0011 é Slint 1.18.x + WGPU 30.x + CPAL 0.18.2, Vulkan inicial no Windows x64 e SDL3 após auditoria.

A main ainda contém egui/eframe, D3D12 e WASAPI/XAudio2. A nova stack não está implementada. A POC R1 precisa comprovar execução real, device/queue compartilhados, textura externa e composição Slint/WGPU.

## Baseline integrada

- PR #743 MERGED; worker SMS e comandos limitados integram a baseline.
- Runtime SMS Rust com SmsTemporalWorker.
- Scheduler e componentes compartilhados preservados.
- Shell desktop atual egui/eframe.
- Host gráfico atual D3D12.
- Saída física atual WASAPI/XAudio2.
- Compatibilidade e fidelidade sistêmica continuam sujeitas aos gates existentes.

## PRs relevantes

| PR | Estado | Escopo |
|---|---|---|
| [#744](https://github.com/leonardo201800478/sega-emulator/pull/744) | OPEN, Draft; HEAD a357f25b23da7655b421f4e9735c42c2d253c754 | Fallback FPS egui; não integrada |
| [#745](https://github.com/leonardo201800478/sega-emulator/pull/745) | OPEN, Draft; HEAD b3939018d97fae9c061ecce402687135509de3b8 | ADR 0011/documentação. CI #6423 SUCCESS; jobs não aplicáveis SKIPPED/NOT RUN |
| [#746](https://github.com/leonardo201800478/sega-emulator/pull/746) | OPEN, Draft; HEAD 52df9085fc057360095d39c72a87ce2e354eb04a | FrameMailbox portável; não implementa WGPU. CI #6424 e SMS Debugger Runtime #147 SUCCESS no HEAD informado |

PR Draft não é integração. Reconsultar estado e SHA antes de qualquer ação.

## Classificação da migração

| Entrega | Estado |
|---|---|
| Decisão arquitetural | DOCUMENTADA na ADR 0011 |
| Análise documental das APIs | PASS somente como análise |
| POC Slint/WGPU compilada | NOT RUN |
| Device/queue compartilhados em execução | NOT RUN |
| Vulkan real Windows x64 | NOT RUN |
| Textura atualizada e composição | NOT RUN |
| Frames reais SMS | NOT RUN |
| CPAL físico | NOT RUN |
| SDL3 na nova arquitetura | NOT RUN |
| Paridade/corte da baseline | NOT RUN |

CI #6423 não valida a POC gráfica. CI #6424 valida os jobs aplicáveis ao diff do worker, não WGPU/Vulkan.

## Próxima tarefa

Concluir a sprint R1 — POC Slint + WGPU 30 + Vulkan, isolada do runtime. Após R1 aprovada, avançar para R2 e conectar frames SMS sem alterar Scheduler ou hardware.

## Evidências

Classificar cada suíte PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED nunca é PASS. Registrar SHA, ambiente, comando, versões, backend efetivo e limitações. Builds de aplicação e avaliações de desempenho devem ser Release.
