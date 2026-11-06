# ADR 0010 — Produto 100% Rust, SMS temporal como único runtime inicial

- **Status:** ACCEPTED — decisão definitiva de arquitetura e prioridade.
- **Data:** 2026-11-06.
- **Baseline:** `main` em `edbec9272efe87a6a408bac74283d86a91d87b23`.
- **Substitui:** decisões transitórias da ADR 0009 e trechos conflitantes de planos anteriores.
- **Escopo:** runtime, aplicação, interface, ferramentas, testes, CI e distribuição do produto.

## 1. Decisão definitiva

O Sega Emulator será um produto integralmente Rust. Python deixa de ser linguagem de produção, integração, teste, tooling ou referência versionada do projeto. A interface desktop será egui/eframe. PySide6/Qt, PyO3, Maturin, pytest, Ruff, setuptools, uv e todo o empacotamento Python serão removidos.

Não haverá frontend legado nem frontend de biblioteca nesta etapa. O executável nativo terá somente uma interface operacional mínima em egui: menus do emulador, abertura/carregamento de mídia quando a sessão SMS estiver integrada, comandos de ciclo de vida e apresentação de estado. Não implementar agora biblioteca visual, scraper, catálogo visual ou painéis de configuração extensos.

A migração não é uma opção nem uma direção futura sujeita a paridade do frontend antigo. A arquitetura anterior falhou em entregar um produto realmente Rust-only e a manutenção do caminho Python aumenta o custo e o tempo de validação. Os contratos funcionais úteis serão reimplementados/testados em Rust; não se manterá Python como oráculo permanente.

## 2. Único runtime ativo inicial: Master System temporal

O único console ativo nesta etapa será o Sega Master System. A máquina SMS deverá usar exclusivamente:

- `sega-z80-cpu`: novo Z80 compartilhado e sua execução/microsequenciador;
- `sega-machine-scheduler` e `sega-temporal-core`: autoridade única da linha temporal;
- `sega-sn76489` e `sega-audio-runtime`: PSG e pipeline temporal de áudio;
- `sega-video`: VDP SMS temporal/compartilhado;
- memória, mapper, I/O, barramento e perfil próprios do SMS, sem uma segunda CPU;
- hosts Rust existentes para D3D12, WASAPI/XAudio2 e entrada nativa.

A execução SMS não poderá chamar nem depender do Z80 atômico legado, da máquina SMS atômica, do PSG legado ou de um caminho Python/PyO3. A API da aplicação será coarse-grained (`load_rom`, `start`, `pause`, `stop`, `reset`, `step_frame`) e não fará callbacks por instrução. egui não controla clocks nem executa hardware.

## 3. Isolamento dos cores antigos

### 3.1 SMS atômico antigo

O caminho SMS atômico baseado em CPU/máquina próprios deve ser removido do grafo de dependências ativo e transferido para `legacy/rust_reference/sms_atomic/` somente como referência temporária de código Rust, se ainda for necessário para auditoria. Não pode ser compilado pelo workspace ativo, selecionado em runtime ou usado como fallback.

A separação será por dependência real: preservar no runtime temporal somente os módulos de cartucho/memória/I/O/configuração que o barramento temporal efetivamente consome. Mover a implementação atômica de CPU/máquina/ALU e qualquer PSG/VDP duplicado que não seja dependência do caminho temporal. Não mover cegamente módulos compartilhados antes de provar seus consumidores.

### 3.2 SG-1000 antigo

O crate atômico `native/sg1000_core` será desconectado do workspace e movido para `legacy/rust_reference/sg1000_atomic/` como arquivo Rust temporário. Não haverá seleção de SG-1000 na aplicação nem dependência do crate legado. Depois que o SMS estiver executável e aceito, o core SG-1000 antigo será eliminado. O futuro SG-1000 será um módulo novo, derivado diretamente do Z80 compartilhado e dos componentes temporais que forem tecnicamente compatíveis, sem reaproveitar a CPU local antiga.

Não criar agora um novo runtime SG-1000 nem bloquear a entrega SMS por ele.

## 4. Remoção integral de Python e Qt

Eliminar do repositório e da distribuição, não apenas desativar na CI:

- `src/sega_emulator/**`, incluindo toda a aplicação e frontend Qt/PySide6;
- `tests/**/*.py`, `tools/**/*.py`, scripts e referências Python de produção;
- `legacy/python_reference/**` e quaisquer outros arquivos Python arquivados;
- `native/z80_core` enquanto extensão PyO3, seus manifests/locks e todos os bindings;
- `pyproject.toml`, `uv.lock`, `.python-version`, configurações pytest/Ruff e dependências Python;
- wheels, artefatos binários da extensão e workflows de Python/Qt/PyO3;
- dependências, scripts, documentação de build e tarefas que exijam Python.

Antes de apagar um componente compartilhado, extrair seu comportamento necessário para Rust puro. No baseline, o worker SMS temporal estava localizado em `native/z80_core/src/sms_temporal_worker.rs`; na branch de migração ele foi extraído para `rust/runtime/sms-runtime`, sem PyO3. O runtime usa um contrato de sessão de áudio e recebe uma factory de host executada na thread da máquina; o vídeo continua integrado por `sega-video-runtime`. Nenhum adapter PyO3 temporário será mantido.

A eliminação é completa: não manter Python sob `legacy/`, nem como suíte de testes, ferramenta de auditoria, benchmark, gerador, referência ou fallback. Dados e fixtures não executáveis podem permanecer quando tiverem licença e utilidade comprovadas.

## 5. Arquitetura de destino

```text
native/
  sega_app/                 # binário e menus egui/eframe
  sega_application/         # comandos, sessão e casos de uso Rust
  sms_runtime/              # worker e ciclo de vida da máquina SMS
  sms_core/                 # somente implementação SMS temporal
  z80_cpu/                  # único Z80 compartilhado
  z80_alu/
  z80_bus/
  sn76489/
  audio_runtime/
  audio_output/
  video_core/
  video_runtime/
  temporal_core/
  machine_scheduler/
  input_runtime/
legacy/
  rust_reference/
    sms_atomic/              # temporário, sem build/uso em runtime
    sg1000_atomic/            # temporário, sem build/uso em runtime
```

A árvore é alvo lógico; nomes e limites de crates serão ajustados somente para refletir dependências concretas e compilar sem duplicação. Consolidar os workspaces e lockfiles em um único workspace Cargo quando as dependências Windows/host permitirem. Não manter crates Python/PyO3.

## 6. Sequência obrigatória

1. **M0 — congelar a direção e mapear dependências:** inventariar arquivos, manifests, workflows e consumidores dos módulos SMS/SG-1000; identificar o conjunto mínimo compartilhado necessário ao SMS temporal. Baseline concluído.
2. **M1 — extrair runtime puro:** mover worker SMS, construção da máquina, seleção regional, comandos, telemetria e integração de áudio/vídeo para crates Rust comuns; manter o worker como único owner da máquina e do Scheduler. O crate `rust/runtime/sms-runtime` foi criado na branch de migração; validação CI ainda pendente.
3. **M5 — eliminar Python/Qt/PyO3 legado:** remover fontes, testes, ferramentas, dependências, lockfiles, artefatos e workflows. Esta remoção foi executada na branch de migração; não é prova de conclusão funcional nem substitui testes Rust.
4. **M2 — tornar SMS temporal exclusivo:** retirar o SMS atômico do grafo ativo, eliminar CPU/PSG/VDP duplicados e provar que todas as operações usam o Z80 compartilhado, PSG temporal e VDP temporal.
5. **M3 — shell egui operacional mínimo:** integrar menus nativos para abrir ROM, iniciar/pausar/parar/resetar, fechar sessão e sair, além de D3D12, áudio e input nativos; a UI não executa hardware.
6. **M4 — retirar SG-1000 legado:** somente depois do SMS executável, remover o crate antigo e iniciar uma implementação nova baseada no Z80 compartilhado.
7. **M6 — validação final:** compilar e testar em Rust, executar integração Windows física e validar ROM sintética/homebrew. Só então declarar a aplicação Rust-only operacional.

A ordem é dependente: não remover módulos Rust compartilhados necessários ao SMS temporal; não iniciar SG-1000 novo antes do SMS executável; não declarar migração completa apenas porque os arquivos Python foram apagados.

## 7. Gates de aceitação

### Arquitetura e dependências
- [ ] `cargo metadata --locked --format-version 1` resolve todos os crates ativos.
- [ ] Um único Z80 compartilhado é usado pelo SMS; não há CPU SMS local nem CPU SG-1000 ativa.
- [ ] O SMS temporal depende do Scheduler, SN76489 temporal e VDP temporal.
- [ ] O worker SMS e os serviços de host são bibliotecas Rust comuns, sem PyO3.
- [ ] `sega-app` inicia egui/eframe e apresenta menus nativos; não depende de Qt/Python.
- [ ] O core SMS atômico e o SG-1000 atômico não pertencem ao workspace nem ao grafo de runtime.

### Ausência de Python
- [ ] Nenhum arquivo `.py`, `.pyi`, `.pyx`, `pyproject.toml`, `uv.lock`, `.python-version` ou wheel Python permanece no repositório.
- [ ] Nenhuma referência operacional a Python, PySide6, PyQt, PyO3, Maturin, pytest, Ruff, setuptools ou uv permanece em manifests, scripts, CI ou instruções de build.
- [ ] Nenhum workflow instala Python ou executa testes Python.
- [ ] Distribuição Release funciona sem interpretador Python instalado.

### Qualidade e funcionalidade
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] Testes específicos dos workspaces/targets Windows e hosts nativos passam.
- [ ] SMS carrega ROM sintética/homebrew, inicia, pausa, retoma, reseta, produz frames e amostras não silenciosas, recebe input e encerra sem thread órfã.
- [ ] Execução contínua versus em blocos e save/restore são determinísticos.
- [ ] Testes físicos de D3D12, WASAPI/XAudio2 e input são classificados separadamente; NOT RUN/BLOCKED nunca é PASS.

## 8. Política de preservação e exclusão

O legado Rust temporário serve somente para inspeção durante a extração e será removido no marco definido. Não preservar fontes Python. Não manter a aplicação Qt como rollback. Não manter um segundo runtime SMS para compatibilidade. Não manter SG-1000 ativo por obrigação histórica.

ROMs comerciais, BIOS proprietárias e dumps protegidos nunca serão versionados.

## 9. Consequência

Esta decisão substitui a estratégia de migração gradual que mantinha Python/Qt/PyO3 como baseline de rollback. O baseline de comportamento passa a ser documentado por contratos, vetores de teste e evidências Rust. O critério de sucesso é um produto nativo compilável e executável, não a coexistência temporária de duas aplicações.
