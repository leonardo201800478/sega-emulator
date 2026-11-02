# Exemplo de integração: SMS VDP

**Status atual:** integração futura. O VDP SMS legado não é automaticamente compatível com a arquitetura temporal alvo.

## 1. Responsabilidade

O VDP mantém estado temporal próprio: raster, fetch, sprites, pixel pipeline, registradores/latches, VRAM/CRAM, status e IRQ. Ele não possui scheduler global nem avança tempo por conta própria.

## 2. Domínio e eventos

O perfil SMS registra o domínio VDP e define razão/fase com base em revisão de hardware e evidência. O VDP anuncia próximos limites observáveis: fetches, mudança de estado de raster, IRQ, latch e outras fronteiras sustentadas por evidência.

Não inventar frequência, fase ou slot de fetch. Diferenças SMS1/SMS2/Game Gear devem ser perfis explícitos quando comprovadas.

## 3. Acesso compartilhado

Writes de CPU a VRAM/CRAM/registradores devem chegar no instante de apresentação/efetivação definido pelo bus e perfil. A ordem relativa com fetch do VDP deve ser documentada por fonte/medição. O Scheduler não resolve contenção.

## 4. Avanço agregado

Pode processar spans de pixels/ciclos internamente até o próximo ponto observável, mas deve preservar fetches, latches, IRQ e efeitos de escrita. Provar equivalência com stepping detalhado e testar writes intralinha.

## 5. Save state

Preservar posição raster, estado de fetch/pixel pipeline, sprite evaluation, latches, status/IRQ, acesso pendente e fase de domínio. Restaurar no meio da linha deve reproduzir os mesmos pixels e eventos restantes.
