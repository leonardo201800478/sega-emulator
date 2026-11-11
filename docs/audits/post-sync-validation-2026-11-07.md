# Relatório de validação pós-sincronização — 2026-11-07

## 1. Identificação e escopo

- Projeto: Sega Emulator — Emulação de Alta Precisão.
- Ambiente: Windows x64, PowerShell, toolchain MSVC; Rust reportado como 1.99.1.
- Branch auditada: `main`.
- HEAD local e remoto confirmado durante a auditoria: `ae90f82b991dc6c85d7386ef7d4b11594473af51`.
- Repositório: `leonardo201800478/sega-emulator`.
- Agente de validação: Antigravity CLI (Agy).
- Escopo: auditoria somente leitura após `git pull --ff-only origin main`; sem alterações de código, commits, branches, merges ou uso de ROM comercial/BIOS proprietária durante os testes.

Este documento registra evidências da sessão executada em 2026-11-07. Os resultados não devem ser extrapolados para SHAs posteriores.

## 2. Integridade Git

| Verificação | Resultado | Evidência |
|---|---|---|
| Branch ativa | PASS | `main` |
| `git status` | PASS | `Your branch is up to date with 'origin/main'; nothing to commit, working tree clean` |
| `git rev-parse HEAD` | PASS | `ae90f82b991dc6c85d7386ef7d4b11594473af51` |
| `git rev-parse origin/main` | PASS | Mesmo SHA do HEAD local |
| Divergência local/remota | PASS | Nenhuma na sessão auditada |

A atualização local ocorreu por fast-forward de 66 commits, sem merge commit.

## 3. Componentes identificados

### Workspace Rust principal

Foram identificados os componentes compartilhados: SMS Core/Runtime, SG-1000 Core, Z80 CPU/ALU/Bus, SN76489, Machine Scheduler, Temporal Core, Video Core/Runtime, Audio Runtime e Input Runtime.

Executáveis identificados:
- `native/sega_app/src/main.rs`: aplicação desktop nativa `sega-app`.
- `native/z80_alu/src/main.rs`: ferramenta de vetores da ALU Z80.
- `native/sms_core/src/bin/z80test_runner.rs`: runner de testes Z80.

Workspaces standalone identificados:
- `native/sega_app`: frontend Rust/egui/eframe.
- `native/audio_output`: saída nativa WASAPI/XAudio2.

## 4. Gates de qualidade e compilação

Os comandos foram executados individualmente. A sandbox restrita inicialmente impediu a criação de arquivos de lock do Cargo (Windows `os error 5`); a repetição com permissão de execução fora da sandbox concluiu.

| Comando / verificação | Resultado | Evidência / observação |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | Código de saída 0; sem violações |
| `cargo fmt --manifest-path native/sega_app/Cargo.toml -- --check` | PASS | Código de saída 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | Repetição concluída; sem warnings |
| `cargo test --workspace --locked` | PASS | Repetição concluída; todas as suítes ativas passaram |
| `cargo build --manifest-path native/sega_app/Cargo.toml --package sega-app --bin sega-app --locked` | PASS | Build concluído; executável de 49.424.384 bytes |
| `cargo test -p sega-input-runtime` | PASS | 6 passed, 0 failed |
| `cargo test --manifest-path native/audio_output/Cargo.toml --locked` | PASS | 5 passed, 0 failed |
| `cargo test --manifest-path native/sega_app/Cargo.toml --locked` | PASS | 10 passed, 0 failed, conforme relatório consolidado |

### Suítes e evidências relatadas

- MachineScheduler: 39 testes aprovados.
- Scheduler stress curto, com `SCHEDULER_STRESS_PROFILE="short"`: 2 aprovados.
- SG-1000 Core: 58 aprovados.
- SMS Core: 130 aprovados.
- Video Runtime: 35 aprovados.
- SMS Runtime: 10 aprovados.
- Input Runtime: 6 aprovados.
- Audio Output: 5 aprovados.
- Z80, ALU, Bus, prefixos e serialização: suítes ativas aprovadas.

A contagem acima reproduz os números do relatório Agy. Não é uma nova execução independente nesta documentação.

### Testes não considerados aprovados

- Scheduler stress replay: dois testes marcados `#[ignore]` não foram executados pelo comando padrão; classificação `NOT RUN`.
- Perfil de stress longo: o relatório menciona evidência histórica em outro SHA; não é promovida como execução nesta sessão.
- ZEXDOC/ZEXALL: `BLOCKED`, pois E7 semântico do Z80 ainda não foi formalmente encerrado.

## 5. Aplicação desktop

### Build e dependências

| Verificação | Resultado | Evidência |
|---|---|---|
| Build do `sega-app` | PASS | `native/sega_app/target/debug/sega-app.exe`, 49.424.384 bytes |
| SDL3 runtime | PASS | `SDL3.dll` encontrado na raiz, tamanho reportado 2.848.256 bytes, versão informada 3.4.20 |
| Testes lógicos SDL3/input | PASS | 6 testes em Input Runtime; mapeamento lógico coberto |
| Testes de configuração WASAPI/XAudio2 | PASS | 5 testes em Audio Output |
| Testes Video Runtime/D3D12 | PASS | 35 testes, incluindo gerenciamento de sessão, conversão RGB e janela filha |

Os testes de configuração e integração não comprovam, por si só, saída física de áudio, apresentação efetiva no monitor ou funcionamento de um gamepad conectado.

### Inicialização e encerramento

- Processo iniciado: PASS.
- Janela nativa criada: PASS; Windows retornou `MainWindowHandle = 68346`.
- Shell egui em teste headless: PASS.
- Encerramento gracioso: **NOT RUN / NÃO COMPROVADO**. O registro PowerShell informa que `CloseMainWindow()` não encerrou o processo dentro da janela de espera (`ClosedGracefully = False`); o script então executou `Stop-Process -Force`. Portanto, a afirmação narrativa de encerramento limpo não é sustentada por essa evidência e não deve ser usada como PASS.

## 6. Testes funcionais do SMS

### Testes sintéticos automatizados

O relatório registra execução de ROM sintética gerada em memória pelo teste `av_test_rom()` no SMS Runtime:

- Renderização Mode 4 com pixels RGB888 não pretos, resolução 256×192: PASS.
- Emissão de pacotes PCM não silenciosos do SN76489 ao sink de áudio: PASS.
- Ciclo de vida da máquina (`start`, `pause`, `step_frame`, `reset`, `stop`): PASS.

Isso comprova os caminhos exercitados pelos testes automatizados, não a experiência completa de gameplay com uma ROM externa.

### ROM externa

- Foram encontrados arquivos de firmware/BIOS proprietários no diretório local de assets e ROMs comerciais de SG-1000.
- Não foi encontrada ROM homebrew de Master System ou ROM sintética externa adequada no disco.
- Por respeito à restrição de não usar ROM comercial ou BIOS proprietária, o teste de gameplay com ROM externa foi classificado `NOT RUN`.

## 7. Matriz consolidada de validação

| Área | Teste | Estado |
|---|---|---|
| Git | HEAD local igual a origin/main no SHA auditado | PASS |
| Formatação | Cargo fmt workspace raiz e sega-app | PASS |
| Lint | Cargo Clippy workspace raiz sem warnings | PASS |
| Testes | Cargo test workspace raiz | PASS |
| Build | Aplicação desktop nativa | PASS |
| Runtime | Presença de SDL3.dll | PASS |
| Automatizado | SMS Core, SG-1000, Z80, Scheduler, vídeo, áudio e input conforme suítes listadas | PASS |
| Integração | Scheduler stress curto explícito | PASS |
| Integração | Dois stress replay ignorados pelo padrão | NOT RUN |
| Aplicação | Processo inicializa e janela nativa é criada | PASS |
| Aplicação | Encerramento gracioso via CloseMainWindow | NOT RUN / NÃO COMPROVADO |
| SMS | ROM sintética em memória nos testes do worker | PASS |
| SMS | Gameplay com ROM homebrew externa | NOT RUN |
| Hardware físico | Apresentação observada em monitor/GPU D3D12 | NOT RUN |
| Hardware físico | Áudio audível em endpoint WASAPI/XAudio2 | NOT RUN |
| Hardware físico | Gamepad USB/Bluetooth físico | NOT RUN |
| Z80 | ZEXDOC/ZEXALL | BLOCKED — aguardar fechamento E7 |

## 8. Limitações e interpretação

1. A falha inicial de acesso ao `.cargo-build-lock` foi atribuída à sandbox restrita do Windows; a repetição autorizada concluiu. Não foi identificada como falha do código.
2. Testes unitários, headless e de integração não substituem validação de saída física de vídeo, áudio ou controles.
3. Criar uma janela nativa não comprova renderização correta de frames do jogo.
4. Gerar PCM não silencioso no sink de teste não comprova áudio audível no dispositivo real.
5. A execução com ROM sintética em memória não equivale a boot e gameplay de uma ROM homebrew externa.
6. A evidência de encerramento gracioso é insuficiente; o processo foi encerrado à força no script de teste.
7. Nenhuma conclusão desta auditoria comprova precisão temporal sistêmica, compatibilidade ampla ou fidelidade total do SMS/Z80.

## 9. Testes manuais recomendados ao usuário

Para completar a validação de produto, executar no Windows com a aplicação compilada e sem ROM comercial/BIOS proprietária:

1. Abrir `native/sega_app/target/debug/sega-app.exe` e verificar visualmente a janela, menus, redimensionamento e ausência de artefatos.
2. Carregar uma ROM homebrew de Master System obtida de fonte autorizada; confirmar boot, imagem e resposta do jogo.
3. Testar teclado e, se disponível, gamepad físico USB/Bluetooth: direções, botões, mapeamento e desconexão/reconexão.
4. Verificar áudio audível no dispositivo selecionado, incluindo início, continuidade, pausa e retomada.
5. Fechar a aplicação pelo mecanismo normal da janela e confirmar no Gerenciador de Tarefas que o processo terminou sem encerramento forçado.
6. Registrar sistema operacional, GPU/driver, dispositivo de áudio, controle utilizado, ROM homebrew e resultado observado. Não incluir ROM/BIOS protegida no repositório.

Esses testes devem ser registrados separadamente como validação manual de produto; não alteram a classificação dos gates automatizados já executados.

## 10. Estado de trabalho e próxima sequência

A auditoria Agy foi somente leitura. Nenhum código, configuração permanente, branch ou PR foi alterado pela Agy.

Na reconciliação do GitHub consultada após a auditoria, a main ainda estava em `ae90f82b991dc6c85d7386ef7d4b11594473af51`, e foram identificadas PRs abertas:
- #636 — teste Z80 ED de wraparound; investigar CI reportado como falho antes de qualquer merge.
- #637 — CI focada PSG/VDP/Input; revisar o HEAD e CI exatos antes de decidir sobre merge.
- #619 — reconciliação documental da auditoria de cartuchos SMS; revisar necessidade e baseline.
- #533 — contrato permanente de conclusão; deve permanecer aberto/Draft durante o desenvolvimento.

Próxima tarefa técnica recomendada: SMS P0-A — contrato de imagem de cartucho e detecção fail-closed de mapper, antes da implementação de SRAM. Critérios iniciais: CRC32/SHA-256 determinísticos sobre os bytes brutos, erro tipado para mapper desconhecido/ambíguo e perfil explícito para ROMs sintéticas de teste. Esta recomendação não é uma implementação nem autorização para iniciar E8 do Z80.

## 11. Referência

Relatório original da sessão Agy: auditoria pós-sincronização de 2026-11-07, executada no SHA `ae90f82b991dc6c85d7386ef7d4b11594473af51`.

**Conclusão:** os gates automatizados e a compilação do frontend passaram na sessão auditada. A aplicação abriu uma janela nativa e os testes sintéticos do SMS passaram. A validação manual de gameplay com ROM homebrew e de periféricos físicos ainda é necessária para elevar a confiança de produto; o encerramento gracioso também precisa ser retestado.
