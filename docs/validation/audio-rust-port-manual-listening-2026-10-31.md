# Registro de validação auditiva do runtime de áudio Rust

- **Data do relato:** 2026-10-31
- **Repositório/base examinada:** `leonardo201800478/sega-emulator`, main `9c6f2b75c4994f1eb3eb7ccc0cd64c7c4d2b2de2`
- **Integração relacionada:** PRs #299, #302 e #303
- **Escopo relatado:** SG-1000 e Master System; WASAPI Shared e XAudio2.

## Resultado observado

O mantenedor informou que testou ambos os backends nos dois cores e percebeu som mais limpo, menos engasgos/cortes, menos ruídos/artefatos estranhos, timbre muito próximo ao Ares e volume mais alto.

**Classificação:** validação auditiva manual positiva, relatada pelo mantenedor. O registro preserva a observação qualitativa e sua matriz (2 cores × 2 backends).

## O que o port mudou

A sessão DSP e o fluxo PCM do caminho ativo foram movidos para Rust. `NativeAudioSession<S: PcmSink>` está no crate puro `sega-audio-runtime`; os adaptadores SG-1000/SMS conectam a sessão aos sinks nativos WASAPI/XAudio2. Mixer, resampling, filas e entrega ao dispositivo são executados em Rust. A fronteira PyO3 não transporta buffers PCM e as APIs Python legadas de drenagem/conversão/escrita PCM foram removidas. O frontend Qt mantém comandos coarse-grained de configuração/ciclo de vida e telemetria.

WASAPI Shared/event-driven permanece padrão, com tentativa de IAudioClient3 e fallback Shared convencional; XAudio2 é alternativa selecionável.

## Limites

Este relato não contém ROMs/jogos identificados, duração de cada sessão, hardware/driver/formato efetivos, gravações comparativas ou medições de amplitude/loudness. Não comprova equivalência acústica com Ares, ausência quantitativa de underruns, latência ponta a ponta, estabilidade de longa duração ou fidelidade analógica. Volume percebido mais alto deve ser medido separadamente (RMS/peak/loudness com sinal e condições iguais), antes de ajustar ganho.

O pump de áudio ainda é acionado ao final de `run_frame()` pelo fluxo Qt. O scheduler independente do timer Qt permanece pendente. FM/YM2413, PCM/CDDA/PWM, estéreo nativo completo e filtros analógicos calibrados por chip não são declarados implementados nesta fatia. `LONG_PASS` e `TEMPORAL_PASS` não foram declarados.

Consulte também [a arquitetura do runtime de áudio](../architecture/native-audio-runtime.md) e [o plano de migração](../architecture/migrations/native-audio-runtime.md).
