# Roadmap mestre — Sega Emulator

**Status:** única rota normativa de implementação.
**Regra de atualização:** este documento define a sequência normativa, não uma fotografia de branches/PRs. Antes de cada tarefa, auditar o SHA corrente da `main`, código, PRs e workflows.
**Prioridade imediata:** migração integral para Rust/egui e SMS temporal como único runtime inicial.
**Política de arquitetura:** [ADR 0010 — produto 100% Rust e SMS temporal](adr/0010-rust-only-sms-temporal-first.md).
**Política de fidelidade:** [ADR-0016](architecture/adr/0016-observable-fidelity-and-proportional-timing.md).

## Diretriz vigente — 2026-11-06

A decisão arquitetural é definitiva: o produto não manterá Python, Qt/PySide6, PyO3 ou Maturin. A aplicação será nativa Rust com egui/eframe. Nesta etapa, a UI terá somente menus operacionais; biblioteca visual, frontend rico e serviços não essenciais ficam fora do escopo.

O único runtime ativo inicial será o Master System temporal, usando o Z80 compartilhado, PSG temporal, VDP temporal e MachineScheduler. O SMS atômico antigo e o SG-1000 atômico serão desconectados do workspace/runtime e arquivados temporariamente como referência Rust. O SG-1000 legado será eliminado depois que o SMS estiver executável; uma implementação SG-1000 nova será iniciada posteriormente sobre o Z80 compartilhado. Nenhum código Python será preservado, inclusive em diretórios legacy.

A migração de aplicação deixa de ser paralela aos gates do SMS: ela é a prioridade atual. O frontend Qt/Python e seus serviços foram removidos na branch de migração, sem port de funcionalidades de biblioteca visual/scraper. Preservar somente contratos funcionais necessários ao produto mínimo e reimplementá-los/testá-los em Rust.

## Sequência normativa

1. Inventariar dependências, manifests, workflows e módulos temporais versus atômicos; baseline concluído.
2. Extrair o worker SMS para Rust puro. O crate `rust/runtime/sms-runtime` já existe na branch de migração; validação CI pendente.
3. Remover Python/Qt/PyO3 e tooling legado sem portá-los. A remoção foi executada na branch de migração; não substitui validação funcional.
4. Isolar o SMS atômico antigo e remover duplicidade de CPU/PSG/VDP do caminho ativo.
5. Integrar o runtime SMS temporal ao binário egui/eframe com menus mínimos, D3D12, áudio e input nativos.
6. Retirar o SG-1000 atômico do workspace; eliminá-lo após SMS executável. Criar SG-1000 novo somente em etapa posterior.
7. Validar o produto exclusivamente com Cargo, testes Rust e gates físicos Windows aplicáveis.

A implementação deve ocorrer em PRs pequenas, compiláveis e com CI verde no HEAD exato. Não fazer merge com jobs obrigatórios SKIPPED, FAIL, BLOCKED ou NOT RUN.

## Gates do SMS executável

- Carregamento de ROM sintética/homebrew.
- Z80 compartilhado como única CPU.
- Scheduler único coordenando CPU, PSG e VDP.
- Framebuffer real apresentado pelo host D3D12.
- PCM não silencioso entregue a WASAPI/XAudio2.
- Input nativo e comandos coarse-grained.
- Start/pause/resume/stop/reset/close sem threads órfãs.
- Execução contínua versus em blocos e snapshot/restore determinísticos.
- Validação física Windows separada de testes headless.

O fechamento semântico global do Z80 continua obrigatório conforme o scorecard e a matriz semântica. Não promover E8 antes do fechamento de E7.

## Regras de arquitetura

- Hardware, Scheduler, áudio, vídeo, input, mídia e aplicação em Rust.
- egui/eframe somente apresentação e menus; sem lógica de hardware ou clocks.
- Nenhum binding Python/PyO3 ou fallback para cores antigos.
- Nenhuma duplicidade de CPU, PSG, VDP, worker ou Scheduler.
- Cores antigos em legacy não são membros Cargo, não são executáveis e não são usados em testes ativos.
- Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos.
