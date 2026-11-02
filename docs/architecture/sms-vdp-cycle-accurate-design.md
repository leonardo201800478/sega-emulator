# SMS VDP — arquitetura temporal e pipeline cycle-accurate

**Estado:** arquitetura-alvo aprovada para orientar a reconstrução/refatoração do VDP.  
**Implementação atual:** `native/sms_core/src/vdp.rs`, modelo por scanline; ver [sms-vdp.md](sms-vdp.md).  
**Precisão:** não implementada; não declarar dot-accurate nem cycle-accurate.

## 1. Objetivo

Construir um VDP SMS independente do TMS9918A do SG-1000, com estado temporal explícito e integração futura ao Master Scheduler. **A primeira implementação tem como alvo exclusivo o ASIC SMS1/315-5124.** A revisão SMS2/315-5246 será caracterizada e adicionada em etapa posterior, reutilizando apenas o que a evidência comprovar equivalente. Não presumir equivalência entre revisões.

A saída visual é produzida pelo VDP em tempo virtual. Vulkan/softbuffer/janela apenas apresentam o resultado; não determinam fetch, raster, sprites, IRQ ou clocks.

## 2. Regras de arquitetura

- Um único owner temporal externo: Master Scheduler.
- O VDP não avança por conta própria nem possui scheduler/clock paralelo.
- O VDP aceita sincronização até timestamp e processa apenas eventos internos vencidos.
- Toda escrita/leitura externa é aplicada após sincronizar o VDP até o ponto de amostragem definido pela transação.
- Nenhuma alteração retroativa de pixels, fetches, status ou IRQ.
- Processamento por lote é permitido quando comprovadamente equivalente.
- Não assumir que 1 pixel = 1 evento global; preservar granularidade de hardware observável.
- Perfil regional, modelo do console e revisão ASIC são dimensões distintas.
- Diferenças de revisão ficam encapsuladas em dados/políticas explícitas e testes, não em condicionais dispersas.
- Sem fallback silencioso para perfil desconhecido.

## 3. Decomposição lógica

### 3.1 VdpCore / revisão ASIC

Mantém estado compartilhado e delega pontos de variação à revisão:
- identificação/revisão explícita;
- regras de máscara/decodificação de endereço quando documentadas;
- diferenças de modos, counters, sprite limits/zoom, paleta e reset somente quando confirmadas;
- comportamento indefinido permanece explicitamente não especificado.

Evitar trait object no caminho de pixel/fetch. Preferir enum/const-generic/estratégia monomorfizada, selecionada na construção da máquina.

### 3.2 TimingGenerator

Responsável por:
- perfil NTSC/PAL e modo de vídeo;
- fase horizontal e vertical;
- active display, borders, HBlank/VBlank;
- contadores H/V e latches de leitura;
- transições de linha/frame;
- próximos eventos temporais do VDP.

As durações e offsets devem vir do registro de evidências, não de constantes copiadas sem fonte. NTSC/PAL não deve ser confundido com SMS1/SMS2.

### 3.3 FetchPipeline

Modela as solicitações de dados à VRAM necessárias para o modo ativo:
- endereço/fase de fetch;
- nome/tile/pattern/atributos conforme modo;
- buffers e latches internos;
- disponibilidade de dados para o compositor;
- interação com writes e leituras concorrentes.

O modelo deve permitir que um write altere somente fetches ainda não realizados, de acordo com evidência. Não redesenhar retroativamente dados já buscados.

### 3.4 SpriteEngine

Separar:
1. avaliação de sprites para linha/segmento;
2. fetch de atributos/padrões;
3. aplicação de limites/overflow;
4. composição e colisão.

O limite físico e o modo de compatibilidade desbloqueado devem ser distintos. O modo desbloqueado é opção de compatibilidade, não comportamento fiel ao ASIC. O ponto em que overflow/collision/status é atualizado precisa ser temporalmente testado.

### 3.5 PixelPipeline / compositor

Consome dados já disponibilizados pelo fetch pipeline e sprite engine:
- background;
- prioridade;
- sprite/background overlap;
- palette select;
- left column mask;
- blanking/border/backdrop;
- CRAM/DAC digital;
- emissão de pixel/segmento no instante apropriado.

O compositor pode produzir spans de pixels se não houver mudança observável dentro do span. Escritas CRAM/registro durante a linha exigem sincronização até o timestamp do write e política comprovada para o primeiro pixel afetado.

A conversão digital de cor deve permanecer separada da simulação analógica de encoder/composite. Não reivindicar colorimetria analógica sem dados medidos.

### 3.6 RegisterFile / latches

R0–R10 e demais registradores expostos devem ter:
- valor escrito pela CPU;
- valor efetivo/latched, quando distinto;
- instante/fase em que a escrita foi aceita;
- evento futuro de aplicação, quando aplicável;
- regras por revisão/modo/região sustentadas por fonte.

Não presumir que todos os registradores atualizam imediatamente ou todos são line-latched. R8/R9 e outros latches precisam de tabelas por modo e evidência.

### 3.7 VRAM / CRAM e portas

Acesso externo deve ocorrer via transação de barramento:
- decodificação de portas;
- latch de comando;
- read-ahead;
- incremento/endereço;
- efeitos colaterais;
- instante de leitura/escrita;
- contenção/arbitragem se confirmada.

VRAM/CRAM não são arrays atemporais para o renderer: fetch pipeline consome dados em fases definidas. Ainda assim, não inventar contenda nem wait states sem evidência.

### 3.8 InterruptController / Status

Modelar separadamente:
- status flags;
- enable bits;
- condição de geração;
- instante de assert;
- duração/clear/acknowledge;
- linha de IRQ observável pela CPU;
- relação com amostragem/acknowledge Z80.

HINT/VINT não podem ser atualizadas apenas no fim da instrução ou scanline se o comportamento exigir fase mais precisa.

## 4. Fluxo temporal

1. Scheduler escolhe próximo timestamp/evento relevante.
2. VDP avança internamente até esse instante, processando fetches, sprite events, pixel spans, counters e IRQs que vencem antes ou naquele ponto conforme fase definida.
3. Uma transação CPU/bus é amostrada/aplicada na fase correta.
4. VDP atualiza estado e agenda próximos eventos.
5. O compositor não altera saída anterior; publica novos pixels conforme pipeline.
6. Ao concluir frame, disponibiliza framebuffer imutável ao host.

O tratamento de eventos simultâneos deve ser específico ao perfil e validado. Não definir uma prioridade universal entre fetch e write sem fonte.

## 5. API conceitual

Assinaturas ilustrativas, não aprovadas como API final:

```rust
trait TimedDevice {
    fn next_event(&self) -> Option<MachineTime>;
    fn advance_to(&mut self, now: MachineTime, ctx: &mut VdpContext);
}

struct SmsVdp<Revision> {
    timing: TimingGenerator,
    registers: RegisterFile,
    vram: Vram,
    cram: Cram,
    fetch: FetchPipeline,
    sprites: SpriteEngine,
    pixels: PixelPipeline,
    interrupts: VdpInterruptState,
    // phase and in-flight state required for exact restore
}
```

Não passar o scheduler inteiro mutável para o VDP. Contexto deve oferecer apenas interfaces limitadas para agenda/IRQ/bus trace, evitando dependência circular e múltiplos owners temporais.

## 6. Save state

Serializar integralmente:
- revisão ASIC e perfil regional/modo;
- timestamp e fase horizontal/vertical;
- contadores, latches e pending register writes;
- pipeline de fetch e dados em voo;
- avaliação de sprites e buffers parciais;
- posição do compositor e pixels já produzidos;
- VRAM/CRAM e estado de portas/read-ahead;
- status/IRQ e eventos pendentes associados.

Testar save/load em meio à linha e em meio ao fetch; a execução após restore deve gerar trace e framebuffer idênticos ao caminho sem interrupção.

## 7. Plano de implementação

### VDP-T0 — caracterização SMS1/315-5124

- congelar golden frames e testes atuais;
- identificar explicitamente o ASIC 315-5124; não inferir ASIC a partir do gabinete/modelo comercial;
- manter NTSC/PAL como perfil independente da revisão ASIC;
- mapear registradores/ports/modos e máscaras de endereço específicas do 315-5124;
- coletar fontes para clocks, contadores, HBlank/VBlank, fetch, latches, sprites, CRAM/DAC, IRQ e acesso VRAM;
- marcar cada regra como documentada, medida, comparativa, inferida ou desconhecida;
- não habilitar modos extra-height 224/240 no perfil 315-5124 sem evidência contraditória específica.

### VDP-T1 — estado temporal sem alterar semântica existente

- criar tipos de timestamp/fase e perfil;
- separar timing generator, register file, memories e status;
- manter adapter de compatibilidade scanline para preservar regressões;
- nenhum claim de precisão.

### VDP-T2 — raster e counters

- modelar H/V phase, blanking e counter reads;
- testes de transições e regiões/modos;
- comparar traces contra fontes/oráculos.

### VDP-T3 — fetch e sprites

- implementar pipeline de fetch;
- sprite evaluation/fetch/limits/overflow/collision em fases documentadas;
- testes de disputa e alteração de VRAM.

### VDP-T4 — pixel pipeline e latches

- emissão incremental de pixels/spans;
- writes intralinha em registradores/CRAM;
- latches R8/R9 e demais regras por modo;
- testes raster sintéticos e primeira divergência de pixel.

### VDP-T5 — integração temporal

- conectar scheduler, CPU micro-ops e bus transactions;
- validar writes/reads em cada fase relevante;
- IRQ subinstrução quando suportada por evidência;
- remover scanline-only como autoridade, mantendo-o apenas como adapter de saída/teste se necessário.

### VDP-T6 — revisão e validação

- fechar validação do 315-5124 primeiro;
- em etapa posterior, comparar 315-5246 e adicionar policies explícitas para diferenças confirmadas;
- NTSC/PAL e modos suportados por revisão;
- cenas sintéticas, homebrew de teste, Ares/MAME/MEKA/GearSystem com ancestralidade documentada;
- hardware real quando disponível;
- TEMPORAL_PASS somente após evidência integrada.

## 8. Matriz mínima de testes

- Reset e valores/estados indefinidos.
- Cada modo suportado e transições entre modos.
- H/V counters em bordas e wrap.
- VBlank/HBlank e IRQ enable/disable/clear.
- R8/R9 e latches em várias posições horizontais/verticais.
- R10/HINT, reload, decrement e IRQ.
- VRAM read-ahead, address wrap, command latch e mirrored ports.
- CRAM write em blanking e active display.
- Background tile fetch, scroll fine/coarse, locks e left column.
- Sprite count, priority, zoom, 8x8/8x16, overflow e collision.
- Writes de CPU em instantes antes/depois de fetch/latch/pixel.
- Eventos simultâneos e desempate.
- Save/load em meio à linha/fetch.
- Regressão golden-frame e determinismo entre chunk sizes.

## 9. Critério de aceitação

Não remover o renderer legado nem alterar golden masters até o pipeline novo ter cobertura equivalente ou superior e comparação independente. Um teste visual final não basta: cada efeito temporal precisa de teste que fixe timestamp/posição e a primeira saída afetada.

O VDP só recebe `TEMPORAL_PASS` quando integrado ao scheduler e ao Z80/barramento temporais. Antes disso, pode receber status específicos como IMPLEMENTED/QUICK_PASS por subsistema, mas a precisão da máquina permanece INTEGRATION_PENDING.

## 10. Referências de partida

- [Status atual do VDP SMS](sms-vdp.md)
- [Investigação de timing raster (2026-10-29)](../audits/sms-vdp-raster-timing-investigation-2026-10-29.md)
- [Auditoria de latches/raster (2026-10-30)](../audits/sms-vdp-latches-raster-audit-2026-10-30.md)
- [Contrato temporal Z80/bus](z80-temporal-bus-model.md)
- [SMS Power — VDP Registers](https://www.smspower.org/Development/VDPRegisters)
- [SMS Power — Modes](https://www.smspower.org/Development/Modes)
- [SMS Power — Sprites](https://www.smspower.org/Development/Sprites)
- [MAME — 315_5124](https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp)
- [Ares — Master System VDP](https://github.com/ares-emulator/ares/tree/master/ares/ms/vdp)
