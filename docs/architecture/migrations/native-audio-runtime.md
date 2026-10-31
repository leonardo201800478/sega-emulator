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
3. [ ] Criar sessão de áudio Rust que possua resampler/mixer e backend.
4. [ ] Integrar sessão aos runtimes SG-1000 e SMS sem transportar PCM por PyO3.
5. [ ] Remover APIs Python de drenagem, conversão e escrita PCM do caminho ativo.
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

**DOCUMENTATION_UPDATED / IMPLEMENTATION_PENDING**

A atualização documental não significa que a migração de código foi implementada. Os estados IMPLEMENTED, QUICK_PASS, SHORT_PASS, LONG_PASS, INTEGRATION_PENDING e TEMPORAL_PASS devem ser registrados separadamente, conforme evidência.
