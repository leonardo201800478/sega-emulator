# SMS — roteiro de validação integrada no Windows

Data: 2026-11-07

## Build sob teste

Esta branch integra, sem merge na main:
- PR #639: otimização do commit de previews de clock no MachineScheduler;
- PR #643: mapeamento e persistência de cartridge SRAM do Sega Mapper.

A branch não declara resolvidos os problemas registrados nas issues #640 (underruns WASAPI) e #641 (corrupção visual no menu de save de Phantasy Star).

## Comandos PowerShell

Execute na raiz do repositório, com Rust toolchain do projeto instalada.

```powershell
git fetch origin
git switch integration/sms-gameplay-validation-20261107
git pull --ff-only
rustc --version
cargo --version

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { throw "cargo fmt falhou" }

cargo clippy --workspace --all-targets --locked -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "cargo clippy falhou" }

cargo test --workspace --locked
if ($LASTEXITCODE -ne 0) { throw "cargo test falhou" }

cargo build --manifest-path native/sega_app/Cargo.toml --release --locked
if ($LASTEXITCODE -ne 0) { throw "build nativo falhou" }
```

## Teste manual in-game

Use ROMs locais e não as adicione ao repositório. Registre SHA-256 da ROM, região, backend de áudio, dispositivo de saída, resolução e duração.

1. Inicie um jogo sem SRAM conhecida e confirme boot, imagem, controles e áudio.
2. Teste Phantasy Star: abra o menu de save, observe a região direita da tela e registre se a corrupção aparece.
3. Teste um título com SRAM persistente verificada (por exemplo, Phantasy Star ou Golden Axe Warrior): grave, encerre de forma controlada, reinicie e confirme a restauração.
4. Teste um título sem SRAM persistente e confirme que não surge arquivo de save indevido.
5. Mantenha WASAPI Shared em execução contínua por pelo menos 10 minutos; registre underruns, blocos descartados e qualquer interrupção audível.
6. Repita o teste de áudio com XAudio2, mantendo ROM, região, volume e duração iguais, se disponível.
7. Repita uma sessão curta após pausar/retomar e após reset.

## Evidências a devolver

- Resultado de cada gate: PASS, FAIL, BLOCKED ou NOT RUN.
- Jogo/ROM (nome e SHA-256 local), região e sequência exata para reproduzir.
- Backend de áudio, endpoint, taxa/formato se visível, underruns e blocos descartados antes/depois.
- No problema visual: screenshot sem redimensionamento e momento exato; informe se o defeito aparece no menu e se persiste após fechar/reabrir.
- Logs completos do terminal e da aplicação.

## Limites atuais da observabilidade

Os contadores de underrun/descarte existentes ajudam a confirmar a ocorrência, mas ainda não expõem profundidade/idade da fila nem frames de silêncio inseridos. A telemetria atual do D3D12 expõe sequências publicadas/apresentadas, mas ainda não fornece uma captura lossless do framebuffer RGB888 anterior ao mailbox. Portanto, este roteiro coleta evidências iniciais; não substitui a instrumentação adicional necessária para localizar definitivamente a origem das issues #640 e #641.

## Integridade temporal

Não altere clocks, pacing, tamanho de lote PCM, frame skipping ou temporização do VDP durante a coleta. Não compare resultados obtidos com configurações diferentes como se fossem uma única reprodução.
