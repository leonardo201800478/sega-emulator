# Consolidação da correção SMS R10 e parallax raster

**Branch:** `fix/sms-r10-raster-parallax-consolidation`  
**Base:** `main` em `151eae75c8851aeb9cdbc056d99cfa093013e46c`  
**Objetivo:** consolidar a correção candidata de R10 e as auditorias diretamente relacionadas ao raster/parallax em uma única PR verificável, sem incluir ROM comercial.

## Escopo consolidado

Esta PR reúne o trabalho documental anteriormente separado nas PRs #261, #265, #273 e #274:
- investigação geral de temporização raster do VDP;
- caso de reprodução relatado em Aladdin (Master System), primeira fase, início do scroll lateral automático;
- revisão dos artefatos de auditoria Z80/VDP, incluindo CSV VDP de 50 linhas e CSV Z80 de 1.452 linhas;
- auditoria de latches R8/R9/R10, CRAM/VRAM, IRQ e integração CPU↔VDP;
- alteração candidata do comportamento de escrita de R10 e regressão determinística.

## Correção candidata de R10

Ao escrever o registrador 10, o valor escrito passa a atualizar o valor de reload, sem sobrescrever imediatamente o countdown já ativo. O contador ativo é recarregado nos eventos já existentes de underflow e wrap do quadro.

O teste `mid_irq_r10_write_changes_next_reload_not_active_countdown` modela:
1. intervalo inicial de 64 linhas com R10=63;
2. após o IRQ, handler grava R10=47;
3. intervalo que já estava em andamento permanece com 64 linhas;
4. intervalo seguinte passa a usar 48 linhas.

**Limite da evidência:** o teste prova a regra implementada e protege contra regressão interna. Não é um oráculo independente do VDP físico e não prova, sozinho, que todos os jogos de parallax serão corrigidos.

## Evidência arquitetural que permanece relevante

O runtime atual executa uma instrução Z80 inteira e depois avança o VDP pela duração agregada. Escritas em ports VDP podem alterar estado antes desse avanço. O renderer continua baseado em scanlines e não possui timestamp horizontal geral para R8, CRAM ou VRAM.

Portanto, R10 é uma correção funcional isolada e plausível, enquanto a sincronização raster intrainstrução/intralinha continua sendo uma limitação arquitetural separada. Esta PR não declara scheduler mestre nem temporização ciclo a ciclo.

## Alvos de validação em jogo

- Aladdin (Master System), primeira fase, começo do scroll automático com nuvens em camadas.
- Outros títulos relatados pelo usuário que empregam scroll/parallax raster; os nomes e os trechos exatos ainda precisam ser registrados.
- Capturas A/B no mesmo frame, resolução nativa 256 px, sem scaling/shaders/filtros, comparadas com Ares ou hardware.
- Trace local (não versionar ROM nem dados derivados) de scanline, R10 escrito, countdown, IRQ, R8 e instante de escrita.

## Critérios de aceitação

- [x] Preservar a semântica funcional já validada do VDP e SG-1000.
- [x] Escrita em R10 não reinicia o countdown ativo.
- [x] Reload de R10 ocorre nos eventos de underflow/frame-wrap já modelados.
- [x] Teste determinístico para alteração de R10 durante o handler.
- [x] Testes de caracterização documentam as limitações atuais de R8/CRAM intralinha.
- [ ] Confirmar a semântica de R10 contra documentação primária ou evidência independente de hardware.
- [ ] Executar ROM de Aladdin no build candidato e comparar o mesmo frame com Ares/hardware.
- [ ] Validar pelo menos um segundo jogo com o mesmo efeito.
- [ ] Inspecionar trace R10/IRQ/R8 e localizar a primeira divergência.
- [ ] Executar todos os workflows aplicáveis no SHA final desta PR.
- [ ] Revisão independente do patch e regressão SMS/SG-1000.

## Estado de validação

- Alteração de runtime: candidata; ainda não validada in-game.
- Teste R10: incluído; requer confirmação de execução no SHA consolidado.
- CI herdada das PRs anteriores não é transferida automaticamente para este novo SHA.
- `TEMPORAL_PASS`: **NOT ACHIEVED**.
- Gameplay A/B: **NOT RUN**.
- Não incluir ROMs, BIOS protegidas, screenshots com dados privados ou traces que revelem conteúdo da ROM no Git.

## Decisão de integração

Esta PR substitui o trabalho disperso das PRs #261, #265, #273 e #274. As PRs anteriores só devem ser encerradas como superseded depois de a PR consolidada existir e seu diff/arquivos serem conferidos. Não fazer merge em `main` até os critérios de aceitação aplicáveis estarem satisfeitos.
