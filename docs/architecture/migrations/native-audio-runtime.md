# Migração do runtime de áudio nativo

A arquitetura normativa e a matriz de hardware estão em [Runtime de áudio nativo](../native-audio-runtime.md).

## Decisão aprovada

- WASAPI Shared/event-driven é o backend padrão no Windows.
- XAudio2 é o backend alternativo existente.
- WASAPI Exclusive e IAudioClient3 ficam como experimentos posteriores, condicionados a medições e validação física.
- Rust é proprietário de todo o runtime e do fluxo PCM; Python/PySide6 fica restrito a configuração, controle e telemetria.
- O escopo arquitetural contempla SG-1000, SMS/GG, Mega Drive, Sega CD e 32X. QSound é extensão DSP opcional, não componente obrigatório desses consoles.

## Plano de execução

1. [x] Inventário inicial de código, crates, fachadas PyO3, frontend e workflows.
2. [x] Decisão de backend e contrato arquitetural documentados.
3. [x] Criar sessão de áudio Rust que possua resampler e backend (primeira fatia; mixer multi-fonte ainda pendente).
4. [x] Integrar sessão aos runtimes SG-1000 e SMS; o caminho Qt ativo não transporta PCM por PyO3.
5. [ ] Remover APIs PyO3 antigas de drenagem/conversão e fachadas Python não utilizadas.
6. [ ] Desacoplar a produção de áudio do timer Qt sem criar uma segunda execução da máquina.
7. [ ] Validar Rust, Maturin, CI Windows, integração e hardware físico.

## Critérios de conclusão

- Nenhum PCM/amostra cruza Python/PyO3 durante gameplay.
- Nenhum processamento de áudio ou escrita no backend ocorre em Python.
- WASAPI Shared é o default e XAudio2 continua selecionável.
- Testes cobrem resampling, mixagem, filas, overflow/underrun, encerramento, falha e recuperação.
- SG-1000 e SMS preservam comportamento determinístico e gameplay.
- Latência, underruns e estabilidade são medidos com baseline equivalente; testes físicos são registrados separadamente.

## Estado

**IMPLEMENTED (primeira fatia) / INTEGRATION_PENDING (migração completa)**

A sessão Rust está integrada ao SG-1000/SMS e o Qt não transporta PCM no caminho ativo. As APIs legadas ainda existem. Os workflows no SHA candidato anterior pararam no Ruff format; lint/build/test ainda não foram executados. Registrar IMPLEMENTED, QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING e TEMPORAL_PASS separadamente, conforme evidência.
