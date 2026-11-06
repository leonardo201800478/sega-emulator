# Estado consolidado do projeto — 2026-11-05

**Repositório:** `leonardo201800478/sega-emulator`  
**Base deste inventário:** `43f481b10590253dd2965aa7a9bb38cb414b7f7b`  
**Escopo:** consolidação do estado publicado, runtime SMS, Z80, vídeo/áudio/input e próximo ciclo de arquitetura Qt.  
**Natureza:** inventário técnico; não declara fidelidade integral, validação em hardware físico ou conclusão das integrações temporais.

## 1. Resumo executivo

O projeto possui um frontend Qt/PySide6 capaz de executar a máquina SMS através da extensão PyO3/Rust, com carregamento de ROM e BIOS, apresentação de imagem, saída de áudio e entrada de controles. O usuário confirmou o funcionamento integrado desses fluxos em sua instalação. Essa confirmação é um smoke test funcional do host, não substitui testes automatizados, validação temporal ou comparação com hardware.

O backend de produção SMS utilizado pelo frontend é atualmente `SMSMachine` (Python adapter fino) → `NativeSMSMachine` (PyO3) → `sega_sms_core::SmsMachine`. O crate `native/sms_core` também contém `SmsTemporalMachine` e componentes temporais, mas esse caminho ainda não substituiu o runtime de produção nem está exposto como opção selecionável na Qt.

O código Python histórico de hardware permanece arquivado em `legacy/python_reference/`; não é um backend de produção selecionável.

## 2. Inventário de runtime

### 2.1 Caminho SMS ativo

1. `src/sega_emulator/frontend/qt_frontend.py` seleciona e instancia `SMSMachine`, aplica política de região/BIOS e conecta os comandos da interface.
2. `src/sega_emulator/machine/sms.py` valida ROM/BIOS e encaminha operações para `sega_z80_core.NativeSMSMachine`.
3. `native/z80_core/src/sms_machine.rs` constrói `sega_sms_core::SmsMachine` por `SmsMachine::with_bios_audio`.
4. `native/sms_core/src/machine.rs` é o caminho de máquina SMS legado/atual usado por essa fronteira.

### 2.2 Caminho temporal SMS

O crate `native/sms_core` contém módulos separados para coordenação temporal, barramento temporal, M1, máquina temporal e VDP SMS1 (`temporal.rs`, `temporal_bus.rs`, `temporal_m1.rs`, `temporal_machine.rs`, `vdp_sms1.rs`). A existência desses módulos não significa que o frontend os execute.

A integração completa `SmsTemporalMachine → MachineScheduler → VDP/PSG → VideoFrame → sessão de apresentação` permanece pendente. A máquina temporal não é ainda um backend de produção intercambiável com `SmsMachine`.

### 2.3 Seletor de backend

No estado desta base, não existe seletor funcional entre `SmsMachine` e `SmsTemporalMachine`. A preferência `cpu_core` é forçada para `rust` e valores antigos `python` são migrados; isso escolhe o runtime Rust, não uma variante de máquina SMS.

Qualquer futuro seletor deve ser explícito, restrito a builds/debug quando apropriado, persistido com migração de configuração, exibindo backend efetivo e impedindo fallback silencioso. Não deve confundir seleção de backend de máquina com seleção de driver de vídeo, áudio ou input.

## 3. Código legado e preservação

- `legacy/python_reference/`: referência histórica arquivada, fora do caminho de produção.
- `native/sms_core::SmsMachine`: runtime SMS Rust atual exposto ao frontend.
- `native/sms_core::SmsTemporalMachine`: caminho temporal em desenvolvimento.
- `native/z80_core`: fronteira PyO3/Maturin, não deve ser confundida com o motor Z80 compartilhado.
- `native/z80_cpu`: crate canônico do motor Z80 compartilhado.
- `native/sg1000_core`: core SG-1000 separado; sua compatibilidade/regressões devem ser preservadas durante mudanças SMS.

Não remover `SmsMachine`, o adaptador PyO3 ou referências históricas até que testes de equivalência, consumidores e política de rollback estejam documentados.

## 4. Z80 — estado e evidências

A trilha E4/E5/E6 e várias partes E7 já foram desenvolvidas e integradas em PRs anteriores. A auditoria global E7 ainda não está fechada; E8 (ZEXDOC/ZEXALL) continua bloqueada até o fechamento formal de E7.

Nesta atualização:
- PR #562 foi integrada na main pelo merge commit `43f481b10590253dd2965aa7a9bb38cb414b7f7b`.
- A correção E7.7 para o erratum NMOS `LD A,I` / `LD A,R` foi reaplicada sobre a main atual: INT aceito imediatamente na fronteira relevante limpa P/V; NMI não recebe a exceção; o latch é consumido na fronteira de fetch normal; snapshot/restore é coberto.
- PR #539 foi encerrada como substituída pela #562; sua branch antiga não deve ser integrada.
- Evidências adicionais de flags de I/O em bloco e registradores metade IXH/IXL/IYH/IYL foram reconstituídas em branch baseada no HEAD atual para revalidação, pois as PRs históricas usavam base muito antiga.

Os relatórios de auditoria distinguem explicitamente: manual oficial, experimento em hardware de outra plataforma Z80, comparação com emuladores, modelo matemático de referência, regressão diferencial e execução em hardware SMS. Essas categorias não são equivalentes. Não alegar validação de silício SMS quando o teste não foi executado em hardware Master System.

## 5. Vídeo e host Windows

As entregas de runtime D3D12 registradas nas PRs integradas incluem:
- #546: fila FIFO limitada e lossless de frames;
- #547: sessão D3D12 com ownership Rust e thread dedicada;
- #550: preservação de frames em minimize/restore;
- #553: DPI por janela PMv2 com fallback PMv1;
- #555: polling finito de 16 ms e detecção de device loss;
- #558: recuperação one-shot para DEVICE_REMOVED/DEVICE_RESET;
- #560: testes externos do contrato funcional do VDP SMS1, cobrindo portas, VRAM read-ahead, wrap de endereço, CRAM, registradores, espelhamento e flags de status.

Essas entregas estabelecem infraestrutura e contratos. Não comprovam, por si, a integração temporal completa do VDP SMS1 à máquina, GPU física real, device removal em hardware, replay headless versus D3D12 ou fidelidade integral do VDP 315-5124.

## 6. Áudio e controles

O adaptador SMS oferece configuração/encerramento de saída de áudio nativa Rust e telemetria de underrun/dropped blocks. O frontend configura a sessão de áudio sem transportar PCM por callbacks Python. A entrada de controles usa a ponte Rust e mapeamentos do frontend.

O smoke test funcional reportado pelo usuário cobre som e resposta aos controles. A validação automatizada deve continuar separando disponibilidade do dispositivo host, comportamento do chip emulado, sincronização temporal e compatibilidade de jogos.

## 7. Interface Qt — inventário e direção

A Qt é camada de apresentação, launcher, biblioteca, preferências, logs, telemetria e automação. Não deve possuir CPU, memória, mapper, VDP, PSG, interrupções, clocks ou lógica de scheduler.

A interface atual já possui fluxo de abertura de ROM, seleção de sistema, pausa/retomada, reset, controles, biblioteca/auditoria de ROM SMS, logs, diagnóstico de máquina, configuração de drivers e opções de áudio/vídeo. O próximo trabalho deve auditar todos os menus e diálogos existentes antes de redesenhar, evitando duplicar funcionalidades já implementadas.

### Organização proposta dos menus

1. **Arquivo**
   - Abrir ROM…
   - Biblioteca de jogos…
   - Recentes (submenus separados por sistema quando aplicável)
   - Fechar jogo
   - Sair

2. **Sistema**
   - Selecionar sistema (SG-1000, Master System; sistemas futuros somente quando implementados)
   - Região/território do SMS
   - BIOS/firmware e diagnóstico de firmware
   - Backend de máquina (somente quando a seleção Legacy/Temporal estiver implementada e validada)
   - Informações da máquina ativa

3. **Emulação**
   - Executar / Pausar
   - Reset
   - Reiniciar com BIOS / boot direto, quando aplicável
   - Estado de execução e métricas essenciais
   - Save State / Load State somente após contrato de serialização temporal completo

4. **Controles**
   - Dispositivos conectados
   - Mapear controles
   - Perfis por sistema/jogo
   - Teste de entradas

5. **Vídeo**
   - Driver de apresentação (Auto, Vulkan, D3D11, D3D12, Raster conforme disponibilidade real)
   - Escala e proporção
   - VSync/VRR
   - Filtros e shader/CRT
   - Captura de frame e diagnóstico de vídeo

6. **Áudio**
   - Driver de saída (WASAPI, XAudio2, desativado conforme suporte)
   - Volume e balanço
   - Taxa de amostragem e buffer
   - Sincronização A/V
   - Diagnóstico de áudio

7. **Ferramentas**
   - Auditoria de ROMs
   - Diagnóstico de BIOS
   - Captura de logs/telemetria
   - Snapshot técnico do VDP
   - Ferramentas de desenvolvimento, separadas das opções comuns de jogador

8. **Ajuda**
   - Atalhos
   - Sobre
   - Documentação
   - Relatório de diagnóstico

As opções devem aparecer uma única vez, em menus de responsabilidade inequívoca. Preferências avançadas devem ficar em diálogos agrupados por domínio; não replicar controles em menus, toolbar e abas sem uma razão de fluxo clara.

## 8. Pendências técnicas conhecidas

- Integrar a máquina SMS temporal ao scheduler e aos dispositivos sem duplicar semântica de CPU.
- Decidir e implementar o contrato do seletor `SmsMachine` / `SmsTemporalMachine`, mantendo o caminho atual como padrão seguro até equivalência demonstrada.
- Integrar VDP SMS1 e PSG à timeline com eventos e causalidade documentados.
- Concluir integração do vídeo produzido pela máquina com a sessão de apresentação.
- Validar execução contínua versus blocos, save states/replay e determinismo.
- Realizar testes em GPU física e hardware SMS/VDP quando disponíveis.
- Consolidar mapper/SRAM e sincronização de portas SMS sem integrar branches antigas por merge cego.
- Fechar E7 semanticamente antes de E8.
- Auditar a Qt inteira: arquitetura, hierarquia, acessibilidade, atalhos, responsividade, DPI, consistência visual, fluxo de ROM/BIOS, erros, estado vazio, telemetria e testes headless/Windows.

## 9. Critérios de qualidade e publicação

Classificar cada suíte individualmente como PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED nunca é PASS. Executar no HEAD exato:
- Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`;
- Python: `python -m pytest`, `python -m ruff check .`, `python -m ruff format --check .`;
- build PyO3/Maturin e smoke test Qt em Windows;
- stress determinístico do scheduler e testes de host aplicáveis.

Toda alteração deve entrar por branch e PR. Não fazer commit direto na main, não usar force-push/reset destrutivo e não integrar com warnings ou checks obrigatórios pendentes.

## 10. Próxima sequência recomendada

1. Reconciliar e validar os testes/documentos E7 reconstituídos sobre a main atual.
2. Fechar a auditoria semântica E7 com matriz de evidências e pendências explícitas.
3. Abrir sprint SMS de integração temporal e contrato de backend selecionável.
4. Implementar o seletor de backend como ferramenta de validação controlada, com telemetria de backend efetivo e testes comparativos.
5. Executar auditoria completa da Qt e consolidar navegação/menus antes de redesenho visual.
6. Implementar o redesign Qt em fatias, mantendo a lógica de hardware exclusivamente em Rust.
7. Somente depois de equivalência e validação, decidir se o caminho temporal se torna padrão e quando o legado de produção pode ser removido.

**Estado final deste inventário:** o SMS atual é funcional no fluxo principal reportado pelo usuário, mas a arquitetura temporal e a seleção entre os dois caminhos ainda não estão concluídas. A prioridade é tornar a transição observável, testável e reversível, sem interromper a execução funcional já existente.
