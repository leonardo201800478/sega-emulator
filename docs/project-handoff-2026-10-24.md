# Registro de continuidade do projeto — 24/10/2026

Este documento preserva o estado confirmado para retomar o trabalho em outra conversa. Revalidar os estados do GitHub antes de tomar decisões: checks, branches e PRs podem mudar depois da data deste registro.

## Repositório e estratégia

- Repositório: https://github.com/leonardo201800478/sega-emulator
- Plataforma-alvo inicial: SG-1000.
- Linguagem de referência: Python 3.14.x (ambiente de trabalho anteriormente identificado como Python 3.14.7).
- Sistema de desenvolvimento: Windows x64, PowerShell e VS Code.
- Objetivo: emulação determinística, modular, testável e tecnicamente honesta sobre limites de precisão.
- Rust deve ser considerado seletivamente após profiling, contratos estáveis e testes de equivalência; não reescrever o núcleo preventivamente.
- Não distribuir ROMs comerciais ou BIOS protegidas. Usar homebrew, testes sintéticos e material legalmente obtido.

## Estado de base conhecido

Commits históricos confirmados em `main`:
- `6e8943ad6e1861daed1b09c93c45728c1b104342` — PR #30, aliases de portas do VDP, read-ahead e wrap de VRAM.
- `d6155dffa87fa262536303a23830594940717066` — PR #31, workflows de validação.
- `1dd7c75a185b965118157a494219f456bb1385ef` — PR #32, documentação SG-1000, memória e barramentos.

A integração inicial inclui CPU Z80, memória ROM/RAM, VDP TMS9918A e entradas básicas. A CPU executa instruções e retorna T-states nominais, mas ainda não agenda cada acesso de barramento ciclo a ciclo. A máquina espelha imagens pequenas de cartucho de forma simplificada; banking, SRAM e hardware especial de cartucho não são universais. O PSG ainda não está integrado à máquina. A compatibilidade geral com jogos reais não foi demonstrada.

## PR #33 — estado e correções recentes

- PR: https://github.com/leonardo201800478/sega-emulator/pull/33
- Título: `VDP: improve TMS9918A sprite evaluation and regression coverage`
- Branch: `fix/vdp-sprite-rendering`
- Base: `main`, SHA de base registrado: `1dd7c75a185b965118157a494219f456bb1385ef`
- Estado confirmado na última consulta: aberta, draft, sem merge.
- Último head confirmado: `65de177a016a13c085f9e4651c2b24343015276c`.

### O que a PR implementa

- Avaliação de sprites por scanline durante a renderização do quadro, em ordem de índice.
- Limite visual de quatro sprites por linha habilitado por padrão.
- Opção de diagnóstico `TMS9918A(sprite_limit_enabled=False)`, que permite compor visualmente todos os sprites da linha sem desligar o flag de overflow.
- Overflow com índice da quinta sprite e flag de colisão para pixels opacos sobrepostos.
- Tratamento de sprites transparentes, deslocamento horizontal de -32 pixels, clipping, tamanhos 8x8/16x16, magnificação e ordenação de padrões 16x16.
- Testes de regressão e documentação específica em `docs/hardware/tms9918a-sprite-rendering.md`.

### Falhas iniciais de CI e correções

No head anterior `ab1d8e42307bbd0db849f9f23e8dd8489fa1950e`, os logs apontaram:
1. `ruff format --check .` exigia quebra de linha na expressão `visible_sprites`.
2. Dois testes esperavam pixels pretos, mas o fundo renderizado pelo fixture era branco: `test_fifth_sprite_is_not_drawn_when_hardware_limit_is_enabled` e `test_sprite_can_be_shifted_left_by_32_pixels`.
3. A suíte executada naquele head reportou 419 testes aprovados e 2 falhas. Isso é resultado histórico do head antigo, não do head atual.

Correções enviadas diretamente à branch:
- `8d7d748a0020fc9744c62bf0d676b9351f734970` — formatação Ruff do renderizador.
- `65de177a016a13c085f9e4651c2b24343015276c` — expectativas de fundo dos dois testes ajustadas para branco.

### CI do head atual

Na consulta após o commit `65de177`, estes dois checks estavam **em andamento**, sem conclusão final:
- [Quality](https://github.com/leonardo201800478/sega-emulator/actions/runs/40107910560)
- [Repeatability, imports and package integrity](https://github.com/leonardo201800478/sega-emulator/actions/runs/40107910615)

Não foram confirmados resultados finais dos checks nessa consulta. Reabrir os links, buscar todos os check-runs associados ao head atual e ler os logs caso algum falhe. Não afirmar que os testes passaram até existir resultado final verde.

### Próximas ações para PR #33

1. Consultar novamente a PR e confirmar head SHA.
2. Verificar todos os workflows/checks, não somente os dois acima.
3. Se houver falhas, baixar logs do job e identificar a causa exata antes de alterar.
4. Se os checks passarem, revisar comportamento de overflow, prioridade, transparência, sprites acima da tela, clipping e quadrantes 16x16. Os testes atuais são regressões internas, não validação diferencial completa.
5. Manter draft e não fazer merge sem critério de aceitação e autorização explícita. Não declarar precisão ciclo a ciclo.

## Outras PRs abertas observadas em 24/10/2026

A listagem anterior mostrava as seguintes PRs abertas/draft. Revalidar todas antes de agir:

- [#27](https://github.com/leonardo201800478/sega-emulator/pull/27) — documentação de auditoria de VRAM/barramento; checks `quality` e `Full suite and opcode inventory` falharam na consulta anterior.
- [#26](https://github.com/leonardo201800478/sega-emulator/pull/26) — regressões de status de sprites; mesmos checks falharam na consulta anterior.
- [#25](https://github.com/leonardo201800478/sega-emulator/pull/25) — base de PSG; mesmos checks falharam na consulta anterior.
- [#24](https://github.com/leonardo201800478/sega-emulator/pull/24) — opcodes ED não atribuídos tratados como NOP; mesmos checks falharam na consulta anterior.
- [#23](https://github.com/leonardo201800478/sega-emulator/pull/23) — harness de auditoria Z80/Colab para Q/SCF/CCF; mesmos checks falharam na consulta anterior.
- [#19](https://github.com/leonardo201800478/sega-emulator/pull/19) — barramentos de VRAM/memória; os checks consultados estavam verdes.
- [#18](https://github.com/leonardo201800478/sega-emulator/pull/18) — reconciliação de correções de auditoria Z80; `quality` estava verde e o check da suíte ainda aparecia em andamento na consulta anterior.

Os logs das PRs #23–27 ainda não tinham sido analisados individualmente. **Não assumir que as falhas compartilham uma causa**. Recuperar os logs de cada PR e decidir se a mudança está correta, depende de alterações em `main`, ou deve ser atualizada/rebaseada. Não fazer merge em falha nem fechar PRs automaticamente.

## Z80: direção técnica

- Referências principais já documentadas em `docs/z80-coverage.md` e `docs/z80-code-audit-2026-10-22.md`.
- O inventário de instruções e a presença de caminho de execução não provam conformidade semântica completa.
- Priorizar flags, resultados, efeitos colaterais, interrupções e ciclos documentados com testes reproduzíveis.
- Distinguir instruções não documentadas de instruções documentadas. Não importar sem critério comportamento de Z180, R800 ou outros derivados.
- O harness de auditoria longa usa Google Colab quando a execução pode durar horas; reservar testes curtos para execução local. Ao preparar arquivos/scripts para Colab, garantir diretórios de entrada/saída corretos e confirmar o conteúdo do artefato retornado antes de tirar conclusões.

## Comandos de validação local no Windows

A partir da raiz do clone local, sem presumir que esteja sincronizado com o GitHub:

```powershell
git status --short --branch
git fetch origin
git log --oneline --decorate -10
.\.venv\Scripts\python.exe -m pytest -q
.\.venv\Scripts\python.exe -m ruff check .
.\.venv\Scripts\python.exe -m ruff format --check .
git diff --check
```

Antes de atualizar a branch local, inspecionar `git status`, commits locais e diferenças com `origin`. Não usar `git reset --hard` nem descartar alterações locais sem autorização. O clone em `C:\Users\leost\sega-emulator` foi citado anteriormente, mas sua existência e sincronização não foram verificadas nesta etapa.

## Protocolo para a próxima conversa

Começar pelo estado remoto atual, sem repetir toda a auditoria:
1. Ler este documento e o README.
2. Consultar a PR #33 e o status do head `65de177` (ou head mais novo, se já houver).
3. Finalizar a validação da PR #33 antes de ampliar o escopo.
4. Recuperar logs das PRs #23–27 individualmente.
5. Continuar a evolução incremental SG-1000; não iniciar Mega Drive, Sega CD ou 32X agora.

### Regras permanentes de colaboração

- Inspecionar branch, arquivos, testes e histórico antes de alterar.
- Fazer mudanças pequenas e coerentes; preservar trabalho concorrente.
- Usar GitHub diretamente quando disponível.
- Atualizar testes e documentação afetados.
- Registrar commits confirmados e links.
- Separar testes aprovados, reprovados e não executados.
- Não afirmar que arquivos foram alterados, testes executados ou workflows aprovados sem evidência.
- Pedir autorização para mudanças arquiteturais importantes, operações destrutivas, mudança de proteções ou merge.
