# SMS1 VDP 315-5124 — modelo de hardware e protocolo de três frentes

**Data:** 2026-11-02  
**Escopo:** Master System 1, ASIC 315-5124. SMS2/315-5246 fica para etapa posterior. NTSC/PAL são perfis regionais independentes da revisão ASIC.  
**Estado:** modelo arquitetural; não declara implementação cycle-accurate.

## 1. Autoridade e regras

O Master Scheduler da máquina é o único owner temporal. O VDP:
- não possui thread, scheduler ou clock independente;
- não avança por frame, scanline ou instrução como autoridade temporal;
- processa eventos somente até o timestamp solicitado pelo Scheduler;
- recebe transações externas após sincronização até o ponto de amostragem definido;
- nunca modifica retroativamente pixels, fetches, status, counters ou IRQ.

Nenhuma frequência nominal, duração aproximada, constante legada ou comportamento de outro ASIC será promovido a fase, offset ou prioridade sem evidência apropriada.

## 2. Três frentes independentes

As frentes investigam em paralelo, mas não implementam três versões concorrentes. Cada uma tem escopo exclusivo. Contratos compartilhados exigem PR própria e revisão cruzada.

### Frente A — Hardware, documentação e proveniência

**Pergunta central:** o que o ASIC 315-5124 realmente faz?

Responsabilidades:
1. Extrair requisitos de manuais, SMS Power, esquemas e medições.
2. Identificar placa, ASIC, região, revisão de manual, página/seção e possíveis erros de OCR.
3. Classificar cada afirmação: PRIMARY_DOC, MEASURED_HARDWARE, INDEPENDENT_TEST, COMMUNITY_DOC, EMULATOR_SOURCE, INFERENCE ou UNKNOWN.
4. Investigar H/V counters, active display/HBlank, fetch slots, latches, sprites, status, IRQ, VRAM/CRAM e contenção.
5. Manter Evidence Register e manifests de oráculos.
6. Não editar runtime.

**Entregáveis:** ficha por comportamento com fonte/localização, estímulo, observável, confiança, limitações, perguntas UNKNOWN e plano de medição. Registrar licença/proveniência de test ROM e código externo.

**Proibido:** inferir fase por frequência nominal; usar MAME/Ares como prova física; importar timing de Game Gear sem equivalência demonstrada; tratar 228 T-states/linha do legado como fato SMS1.

### Frente B — Arquitetura e implementação Rust

**Pergunta central:** como representar o comportamento aprovado sem inventar o que falta?

Responsabilidades:
1. Definir tipos, ownership, APIs e invariantes do VDP SMS1.
2. Manter independência do TMS9918A do SG-1000.
3. Implementar somente regras sustentadas por evidência suficiente.
4. Preservar fases e estados em voo para restauração.
5. Integrar com Scheduler por interface limitada; nunca possuir o Scheduler.
6. Evitar alocação/cópia por pixel e dispatch dinâmico no hot path sem medição.
7. Não usar o VDP legado como atalho.

**Entregáveis:** proposta de tipos, diffs pequenos e compiláveis, testes unitários, save-state serializável, benchmark após baseline e relatório de pendências.

**Proibido:** constantes de raster especulativas; renderer scanline como autoridade; atualização retroativa do framebuffer; fallback silencioso; dependência Python/PyO3 no runtime do chip.

### Frente C — Verificação independente

**Pergunta central:** como demonstrar que a implementação está errada quando estiver errada?

Responsabilidades:
1. Criar fixtures sem chamar funções internas do VDP para gerar expected values.
2. Implementar testes funcionais, property, state-transition, trace e save/restore.
3. Revisar se cada teste discrimina implementação correta de mutações plausíveis.
4. Comparar fontes e emuladores registrando ancestralidade.
5. Criar comparador que reporte primeiro evento e primeira coordenada divergente.
6. Rodar regressões SG-1000/SMS e gates de CI.
7. Não alterar produção para fazer testes passarem.

**Entregáveis:** testes com ID, estímulo, expected, fonte e limite; relatório PASS/FAIL/BLOCKED/NOT RUN por SHA; contraexemplo; análise de mutação e parecer independente da PR.

**Proibido:** renderer legado como oráculo; golden gerado automaticamente do candidato; concordância de emuladores derivados como prova; tolerância visual sem justificativa.

### Integração das frentes

- A publica evidência; B implementa requisitos suficientemente sustentados; C valida sem depender de B.
- C pode rejeitar assertion que B considere plausível.
- Conflitos documentais permanecem explícitos, sem votação.
- Mudança de contrato compartilhado exige PR dedicada.
- O arquiteto integra resultados; ninguém publica diretamente em main.
- No máximo três frentes ativas; sem sobreposição de edição nos mesmos arquivos.

**Nota operacional:** esta sessão não dispõe de mecanismo para iniciar três agentes autônomos separados. As três frentes estão formalmente definidas como mandatos de trabalho, não como agentes já executando tarefas em segundo plano.

## 3. Modelo lógico do SMS1 VDP

O VDP é um dispositivo digital com estado persistente, conectado ao barramento Z80 e sincronizado pelo relógio mestre. A saída visual é consequência dos eventos, não a fonte do tempo.

### 3.1 Identidade/configuração

Cada instância possui:
- revisão explícita SMS1 / 315-5124;
- perfil regional NTSC ou PAL separado;
- modo de vídeo;
- estado/configuração de compatibilidade explicitamente separados do perfil de hardware.

Não inferir ASIC pelo nome comercial do console ou pela região. Recursos exclusivos conhecidos do 315-5246 não podem ser ativados no perfil SMS1. Revisão desconhecida falha explicitamente.

### 3.2 Diagrama funcional

~~~text
                 Master Scheduler (único owner temporal)
                               |
                       timestamp / evento
                               |
                +--------------v---------------+
                |       Sms1Vdp3155124         |
                |                               |
 Z80 Bus <----->| Port / Bus Interface         |
                |   command/address latch      |
                |   read-ahead state           |
                |   VRAM / CRAM access         |
                |                               |
                | Register File + Latches      |
                |          |                    |
                | Timing Generator             |
                |   counters / blanking        |
                |   |                           |
                |   +--> Fetch Pipeline <VRAM>  |
                |   +--> Sprite Engine         |
                |               |               |
                |         Pixel Pipeline <CRAM> |
                |               |               |
                |         Frame/Line Output     |
                |                               |
                | Status + Interrupt Controller|
                +-------------------------------+
                               |
                         IRQ / pixels
~~~ 

O diagrama expressa dependências funcionais, não prioridade elétrica nem ordenação de eventos simultâneos.

### 3.3 Port/Bus Interface

Responsável por:
- decodificação de portas de dados e controle;
- sequência de comando de dois bytes;
- primeiro byte pendente;
- leitura de status e efeitos colaterais;
- read-ahead buffer;
- endereço interno, incremento e modo VRAM/CRAM;
- transações com timestamp/fase fornecidos pela máquina.

Não implementa WAIT por suposição. A cadência de acesso descrita em manual de software não prova injeção automática de wait-state.

### 3.4 RegisterFile e latches

Separar quando necessário:
- valor escrito pela CPU;
- valor ativo;
- valor latched para linha/frame/fetch;
- instante/evento de efetivação;
- máscara/decodificação específica do ASIC.

Não existe regra universal de atualização imediata ou no próximo scanline. Cada registrador exige política e teste próprios. R8/R9/R10 têm investigação específica; fronteiras exatas permanecem UNKNOWN até validação.

### 3.5 TimingGenerator

Estado conceitual:
- timestamp virtual;
- posição horizontal/vertical;
- perfil de clock/região;
- counters observáveis;
- active display, blanking e frame;
- latches de leitura H/V;
- próximos eventos temporais.

Responsabilidade: calcular eventos com aritmética inteira/racional determinística e produzir transições de raster. Não executa fetch nem desenha pixels. Não mantém clock independente.

H-counter offsets, dot count, reset phase, duração exata de HBlank e relações de fase ainda não são constantes aprovadas.

### 3.6 FetchPipeline

Representa solicitações à VRAM anteriores ao uso visual:
- endereço solicitado;
- classe do fetch;
- instante/fase;
- dado retornado e disponibilidade;
- latches e buffers em voo;
- vínculo com pixel/segmento consumidor.

Um write só afeta fetches ainda não realizados, conforme arbitragem comprovada. Não reconstrói pixels já emitidos. A sequência exata de fetch por dot não está aprovada; não preencher slots desconhecidos por interpolação.

### 3.7 SpriteEngine

Separar:
1. avaliação de elegibilidade;
2. leitura/fetch de atributos e padrões;
3. limites, prioridade e overflow;
4. composição e colisão.

Preservar índice SAT em avaliação, ordem dos sprites selecionados, contagem, dados de padrão, flags e momento de atualização. A regra comunitária sobre zoom horizontal dos últimos quatro sprites elegíveis no SMS1 deve ser caso de teste próprio. Overflow e collision serão testados separadamente, inclusive a discrepância documental dos bits de status.

### 3.8 PixelPipeline

Consome dados disponibilizados por fetch e SpriteEngine:
- background e sprite;
- prioridade e transparência;
- seleção de paleta;
- máscara de coluna, border e blanking;
- cor digital resultante.

Emite pixels ou spans somente quando equivalentes. Writes de CRAM, scroll ou registradores no meio de um span exigem divisão no evento correto. Pixels já emitidos são imutáveis.

Conversão digital de CRAM é separada de encoder/DAC/TV analógicos. Sem medição colorimétrica, não declarar fidelidade analógica.

### 3.9 InterruptController / Status

Separar condição geradora, enable, status latched, linha IRQ, clear/acknowledge, instante de assert/deassert e evento pendente.

Leitura de status e amostragem de IRQ pela CPU são eventos distintos. O VDP não chama diretamente a CPU; a máquina/Scheduler consome o estado da linha IRQ.

### 3.10 Memórias

- VRAM: 16 KiB conforme documentação SMS1.
- CRAM: 32 entradas de 6 bits conforme documentação de software.
- Decodificação, wrap e aliases explícitos.
- Acesso externo e fetch interno diferenciados.
- Nenhum acesso retroativo.

Máscaras R2–R6 precisam de vetores explícitos por revisão. Não inferir comportamento a partir do renderer legado.

## 4. Contrato temporal

Scheduler possui relógio mestre e ordenação global. A máquina converte transações Z80 em requests temporais. VDP expõe próximos eventos e processa até timestamp. Bus adapter resolve a transação. IRQ é publicada para a máquina.

API conceitual, não aprovada como assinatura final:

~~~rust
pub trait TimedDevice {
    fn next_event(&self) -> Option<MachineTime>;
    fn advance_to(
        &mut self,
        now: MachineTime,
        phase: HardwarePhase,
        sink: &mut dyn VdpEventSink,
    ) -> Result<(), VdpError>;
}
~~~

A implementação final pode usar generics/enums para evitar dispatch dinâmico no hot path. Um ordinal HardwarePhase não é automaticamente uma fase elétrica.

Para cada timestamp:
1. Scheduler seleciona próximo instante.
2. Dispositivos processam eventos anteriores.
3. Eventos simultâneos são resolvidos por regra específica, se conhecida.
4. Transação externa é amostrada/aplicada.
5. IRQ e novos eventos são publicados.
6. Nenhum estado anterior é alterado.

Desempate desconhecido permanece BLOCKED.

## 5. Save state e determinismo

Salvar revisão/perfil, timestamp/fase, raster, registradores escritos/ativos, eventos/latches, comando parcial, endereço/read-ahead, VRAM/CRAM, fetches em voo, avaliação de sprites, compositor/posição de saída, status/IRQ.

Salvar/restaurar em vários pontos de uma linha deve produzir traces, estado final e framebuffer idênticos à execução contínua. Isso prova determinismo/restauração, não fidelidade ao hardware.

## 6. Plano de execução

### T0 — caracterização
- [x] escopo 315-5124;
- [x] separar SMS1/SMS2 e região;
- [x] inventariar limitações do legado;
- [x] listar incertezas críticas;
- [x] matriz T0.1;
- [ ] revisar assertions funcionais;
- [ ] manter conflitos documentais bloqueados até resolução.

### T1 — estado temporal e contratos
Tipos de revisão/perfil, estado serializável, invariantes, interfaces de timing/ports/registers/memories, eventos pendentes e testes de save/restore. Sem offsets ou fetch schedule especulativos.

Aceitação: modelo coerente, testes funcionais independentes e compilação sem dependência temporal do renderer legado.

### T2 — raster e counters
Counters/transições com fonte, perfis regionais separados, H/V readback e blanking somente com expected values sustentados, traces com primeira divergência.

Aceitação: cada regra implementada possui fonte e teste reproduzível; UNKNOWN permanece fora do escopo.

### T3 — fetch e sprites
Pipeline de fetch sustentado por evidência; avaliação/fetch/composição separados; overflow/collision independentes; contenção VRAM apenas quando conhecida.

Aceitação: cenas sintéticas independentes aprovadas pela Frente C.

### T4 — pixel pipeline e latches
Emissão incremental, spans equivalentes, latches documentados, writes intralinha e diff da primeira coordenada/pixel.

Aceitação: nenhum pixel reescrito; latches implementados possuem testes de fronteira.

### T5 — integração temporal
Scheduler único, Z80 micro-ops, bus transactions, WAIT/arbitragem somente com evidência, IRQ/status com fase comprovada e traces CPU/VDP/bus.

Aceitação: traces determinísticos e oráculo adequado; somar T-states ao fim de instrução não basta.

### T6 — revisão e validação
Comparar 315-5124/315-5246 sem misturar perfis, NTSC/PAL, regressões SG-1000, homebrew próprio, testes redistribuíveis, emuladores com ancestralidade e hardware físico quando disponível.

TEMPORAL_PASS exige Scheduler + Z80 + bus + VDP integrados e comparação com trace/oráculo adequado.

## 7. Processo de PR e gates

- Uma PR por fatia: estado, portas, counters, fetch, sprites, pixel pipeline e integração.
- PRs empilhadas respeitam ordem de dependência.
- Refatoração estrutural e correção semântica não entram juntas sem justificativa/ADR.
- Golden master alterado exige PR separada com causa, diff e oráculo.
- Código externo exige licença/proveniência.
- Nenhuma ROM comercial, BIOS proprietária ou dump protegido no Git.
- Nunca publicar diretamente em main.

Gates obrigatórios no SHA candidato:
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --locked -- -D warnings
- cargo test --workspace --locked
- python -m pytest
- python -m ruff check .
- python -m ruff format --check .

Cada resultado é PASS, FAIL, BLOCKED ou NOT RUN, vinculado ao SHA exato. Check ausente ou executado em SHA anterior não é PASS.

## 8. Riscos e bloqueios iniciais

| ID | Risco/pergunta | Tratamento |
|---|---|---|
| VDP1-R01 | H-counter offset/fase desconhecidos | Não fixar constante; A pesquisa, C bloqueia |
| VDP1-R02 | 228 T-states/linha legado sem prova | Referência de compatibilidade apenas |
| VDP1-R03 | Fetch schedule por dot desconhecido | Não criar slots fictícios |
| VDP1-R04 | WAIT automático não comprovado | Cadência de software não prova wait-state |
| VDP1-R05 | Conflito em bits overflow/collision | Preservar conflito e buscar vetor independente |
| VDP1-R06 | Ancestralidade de oráculos | Manifest de origem/commit/parentage |
| VDP1-R07 | Golden master congela bug | Separar preservação de correção |
| VDP1-R08 | Mistura SMS1/SMS2 e NTSC/PAL | ASIC e região são dimensões distintas |
| VDP1-R09 | Test ROM sem licença declarada | Não copiar nem versionar |
| VDP1-R10 | Save state parcial incompleto | Testes em meio à linha/fetch |

## 9. Fontes de partida

Documentos do projeto:
- docs/architecture/sms-vdp-cycle-accurate-design.md
- docs/audits/sms1-315-5124-vdp-t0-characterization-2026-11-02.md
- docs/audits/sms1-315-5124-vdp-t01-validation-matrix-2026-11-02.md
- docs/architecture/cycle-accuracy-evidence-register.md
- docs/architecture/z80-temporal-bus-model.md

Fontes externas já identificadas:
- SMS Power SMS1/SMS2 VDPs: https://www.smspower.org/Development/SMS1SMS2VDPs
- SMS Power VDP Registers: https://www.smspower.org/Development/VDPRegisters
- SMS Power Sprites: https://www.smspower.org/Development/Sprites
- SMS Power Scanline Counter: https://www.smspower.org/Development/ScanlineCounter
- SMS Power Service Manual: https://www.smspower.org/Development/SegaMasterSystemServiceManual
- SMS Power Official Docs: https://www.smspower.org/Development/SMSOfficialDocs&num=5
- MAME 315_5124: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- Ares SMS VDP: https://github.com/ares-emulator/ares/tree/master/ares/ms/vdp

Fontes comunitárias e implementações de emuladores não substituem medições do ASIC. Este documento organiza o modelo e o processo; não fecha as lacunas de timing enumeradas.
