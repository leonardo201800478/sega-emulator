# Modelagem das implementações físicas Z80 por console

- **Status:** inventário técnico em evolução; separa evidência de placa/serviço, inferência e hipótese.
- **Objetivo:** modelar as implementações de silício e integração do Z80 sem duplicar o núcleo semântico Rust.
- **PR relacionada:** #119 — modelo total do Z80 e quatro perfis de plataforma.

## Decisão de arquitetura

Não criar quatro implementações de CPU só por diferenças de fabricante, marcação, encapsulamento ou integração. Manter um núcleo semântico Z80 compartilhado e configurar o ambiente de execução com um perfil de silício e um perfil de plataforma.

A modelagem deve distinguir:

1. **Identidade física:** fabricante, marcação de peça, revisão de placa, encapsulamento e integração em ASIC.
2. **Semântica de CPU:** conjunto de instruções, flags, registradores, interrupções e comportamentos documentados/observados.
3. **Interface elétrica/temporal:** clock real da placa, reset, sinais de barramento, wait states, contenção, acesso a memória/I/O e arbitragem.
4. **Dispositivos adjacentes:** chips que podem gerar WAIT/INT ou controlar acesso ao barramento. Não confundir esses chips com o núcleo Z80.

Fabricante ou package diferentes não provam uma diferença semântica. Uma divergência de comportamento deve ser ligada a uma fonte, revisão e teste antes de criar um modo de compatibilidade.

## Inventário por plataforma

### 1. SG-1000 — NEC D780C-1 / µPD780C-1; outras placas com Sharp LH0080A

**Evidência disponível:**
- A documentação técnica de placa consultada identifica o NEC D780C-1 como CPU em uma placa SG-1000 e registra um exemplar de placa posterior com Sharp LH0080A.
- A mesma descrição registra que o clock é derivado do subsistema de vídeo e que o sinal /WAIT vem do chip de som.
- A família D780C é amplamente identificada como compatível com o Z80; a equivalência funcional detalhada de comportamentos não documentados ainda deve ser validada, não presumida.

**Modelo proposto:**
- `SiliconProfile::NecD780C1` para identidade física documentada.
- `SiliconProfile::SharpLH0080A` apenas quando a placa/revisão realmente o exigir.
- Núcleo Z80 semântico comum por padrão; adicionar política de silício específica somente se testes diferenciais demonstrarem diferença.
- O sinal WAIT deve pertencer ao modelo temporal/barramento da plataforma, não a uma regra hard-coded dentro do decoder.

**Falta confirmar:** revisão exata de placa, designação completa da peça (incluindo hífen/sufixo), tolerância/clock real, forma de geração de WAIT e quaisquer diferenças de instrução ou timing observadas entre os fabricantes.

### 2. Master System — Zilog Z80A / NEC D780C-1 e variações de placa

**Evidência disponível:**
- O manual de serviço do Master System identifica o Z80A/315-0041 em placas com CPU discreta.
- Listagens de placas específicas documentam variantes com NEC D780C-1 e Zilog Z0840004PSC.
- O 315-5216 e variantes como 315-5237 aparecem como gate array/I/O/decodificação de memória/controle de sinais em certas placas; não devem ser rotulados automaticamente como “CPU integrada”. A presença de um gate array próximo não prova que o núcleo Z80 esteja dentro dele.
- Algumas variantes usam diferentes chips de VDP e I/O, portanto clock, portas e linhas de interrupção devem ser configurados por revisão de plataforma.

**Modelo proposto:**
- Perfis físicos discretos: `ZilogZ80A`, `ZilogZ0840004PSC`, `NecD780C1` conforme a BOM/placa.
- Separar revisão regional/placa do perfil de silício: NTSC/PAL não é uma revisão de ISA; influencia clock e agenda temporal.
- Não introduzir perfil “CPU em ASIC Master System” sem número de peça, revisão e fonte primária que confirmem a integração.

**Falta confirmar:** cobertura completa de todas as revisões compactas e modelos regionais, exata relação entre gates/ASIC e sinais de CPU, e diferenças comportamentais comprovadas entre as peças.

### 3. Game Gear — Z80A discreto e ASIC de uma peça

**Evidência primária de serviço:**
- O manual de serviço Game Gear VA0/VA1 compara explicitamente as placas: VA0 lista uma CPU Z80A separada, além de VDP customizado e SCA customizado; VA1 descreve o LSI customizado de uma peça que integra CPU + VDP + SCA.
- Referências técnicas de placas identificam o ASIC VA1 como Sega 315-5535; revisões norte-americanas posteriores incluem 315-5682. Os números devem ser associados às revisões correspondentes, não tratados como um único chip universal.
- Portanto, a afirmação “Game Gear sempre usa o núcleo no 315-5441” não está confirmada pelas fontes consultadas e não deve ser adotada no modelo neste momento.

**Modelo proposto:**
- `GameGearVa0DiscreteZ80A`: CPU discreta e VDP/SCA separados.
- `GameGearVa1Asic3155535`: CPU, VDP e SCA integrados conforme manual de serviço.
- Revisões posteriores (por exemplo, 315-5682) precisam de perfil explícito quando a documentação e a evidência de compatibilidade forem suficientes.
- O código de CPU permanece compartilhado; o ASIC representa composição física e interface de eventos, não automaticamente uma ISA diferente.

**Falta confirmar:** clock efetivo por região/revisão, diferenças de temporização internas entre ASICs e Z80 discreto, erratas e quaisquer divergências observáveis de CPU. O manual de serviço é evidência de integração física, não uma especificação de cada transistor do núcleo integrado.

### 4. Mega Drive / Genesis — Z80 auxiliar discreto e integração em GOAC

**Evidência disponível:**
- O suplemento de serviço Mega Drive / Mega Drive 2 distingue o SUB CPU: Z80A DIP no Mega Drive original e Z84C0006 QFP em certas placas do Mega Drive 2.
- Listagens de placas do Model 1 documentam o Zilog Z0840004PSC como CPU Z80 discreta.
- Fontes de revisão de placas identificam a integração do 68000, Z80, VDP e som em ASICs posteriores: 315-5960 e 315-6123 aparecem em revisões diferentes; a composição exata varia por placa.
- **315-5313 não deve ser tratado como o ASIC do Z80:** fontes de hardware o identificam como VDP (Yamaha YM7101). O 315-6123 é identificado em fontes de revisão como GOAC que inclui 68000, Z80, VDP e áudio; 315-5960 também aparece em integrações anteriores. As funções e limites de cada ASIC devem ser vinculados à revisão correta.
- O Z80 do Mega Drive é um processador auxiliar. Seu acesso à memória e às portas é condicionado por controle/arbitragem com o 68000 e pelo subsistema de áudio.

**Modelo proposto:**
- `GenesisDiscreteZ80`: CPU auxiliar discreta (por exemplo, Z0840004PSC ou variante documentada).
- `GenesisIntegratedGoac3155960`: Z80 integrado no ASIC, quando a placa específica confirmar esse componente.
- `GenesisIntegratedGoac3156123`: perfil de integração posterior, com política de memória e sincronização correspondente à revisão.
- Implementar o modelo de ownership/arbítrio do barramento e os sinais de reset/enable como parte da plataforma, não do decoder de instruções.

**Falta confirmar:** correspondência por número de placa/VA para cada ASIC, clocks regionais reais, estados de espera, timing de acesso ao barramento e diferenças de silício entre Z80 discreto e integrado.

## Frequência: modelar a fonte, não uma constante universal

“3,58 MHz” é uma aproximação útil para muitas variantes NTSC, não um valor literal universal para todos os consoles, regiões e revisões. O clock de CPU deve derivar do oscilador/divisor documentado para a placa específica, preservando a razão de fase em relação aos demais subsistemas. Não codificar `3_580_000` como constante comum a todos os perfis.

O modelo temporal deve guardar, quando conhecido:
- frequência do oscilador da placa;
- divisor ou relação de frequência da CPU;
- região/modo de vídeo;
- fase inicial e regra de avanço em inteiros/racionais para evitar drift de ponto flutuante;
- eventos WAIT, reset, INT/NMI e arbitragem aplicáveis.

Se a fonte só disser “3,58 MHz”, registrar precisão como aproximada e manter pendente o valor exato. Não inferir a frequência de CPU apenas do rótulo de um cristal sem conferir o circuito divisor.

## Tipos Rust propostos

Os nomes abaixo são conceituais; devem ser alinhados aos crates existentes antes da implementação.

```rust
enum Z80Silicon {
    NecD780C1,
    SharpLh0080A,
    ZilogZ80A,
    ZilogZ0840004Psc,
    ZilogZ84C0006,
    SegaIntegrated { part: SegaAsicPart, revision: BoardRevision },
}

enum SegaAsicPart {
    GameGear3155535,
    GameGear3155682,
    Genesis3155960,
    Genesis3156123,
}

struct ClockSpec {
    oscillator_hz_num: u64,
    oscillator_hz_den: u64,
    cpu_divider: u32,
    phase_origin: u64,
}

struct CpuCompatibilityPolicy {
    silicon: Z80Silicon,
    undocumented_behavior: UndocumentedBehaviorProfile,
    timing: TimingProfileId,
}
```

Este exemplo define a intenção do contrato, não código Rust já adicionado ou compilado. Em particular, os campos `revision`, `UndocumentedBehaviorProfile` e `TimingProfileId` precisam de tipos concretos e evidência antes da implementação. O perfil de silício não deve controlar mapas de memória ou I/O; isso pertence à plataforma.

## Regras de implementação

1. Começar com uma política de compatibilidade Z80 documentada comum.
2. Separar identificação de silício da configuração de plataforma/placa.
3. Adicionar divergência por fabricante/revisão somente com vetor reproduzível ou evidência física.
4. O ASIC integrado deve compor CPU, VDP, SCA/áudio e I/O, mantendo contratos internos determinísticos; não transformar o número do ASIC em uma variante de opcode.
5. Modelar clock e WAIT/arbítrio com contratos explícitos e inteiros/racionais, não atrasos de parede ou floats acumulativos.
6. Construir testes de perfil com as mesmas instruções e estado inicial, alterando somente a política cuja diferença é investigada.
7. Preservar o marco SG-1000 como prioridade. O inventário de outras plataformas não autoriza ampliar agora o escopo de implementação do hardware delas.

## Fontes consultadas

### Manuais e documentação de serviço
- Sega, *Game Gear VA1 Service Manual*: https://segaretro.org/images/5/56/Sega_Service_Manual_-_Game_Gear_VA1_-_004_-_December_1993.pdf — fonte primária para a comparação VA0/VA1 e integração CPU+VDP+SCA.
- Sega, *Mega Drive / Mega Drive 2 Supplement*: https://gendev.spritesmind.net/files/md/tech/Sega%20Gen%202%20%26%20MD2%20Supplement%20A4.pdf — distingue Z80A DIP e Z84C0006 QFP.
- Sega Master System service manuals e BOMs reunidos por SMS Power: https://www.smspower.org/Development/SegaMasterSystemServiceManual e https://www.smspower.org/Development/PALMasterSystemM4POWERBASEPAL837-6097 — identificam Z80A, NEC D780C-1 e Zilog Z0840004PSC em placas específicas.

### Referências de placas (fontes secundárias, úteis para localização de peças; não substituir BOM/manual)
- SG-1000 board inventory: https://retrotechwiki.com/Sega_SG-1000 — identifica D780C-1 e um exemplar Sharp LH0080A.
- Game Gear board/revision inventory: https://consolemods.org/wiki/Game_Gear%3AGame_Gear_Model_Differences e https://consoleartisan.com/reference/game-gear/ — identifica ASICs 315-5535 e 315-5682 por revisão.
- Mega Drive board inventory: https://segaretro.org/Sega_Mega_Drive_PCB_revisions e https://retroisle.com/others/segamegadrive/techdetail.php — inventaria Z80 discreto e ASICs de revisões posteriores.
- Genesis ASIC summary: https://github.com/user-none/emmd/blob/main/docs/vdp_reference.md — lista 315-5960 e 315-6123 como integrações GOAC e distingue 315-5313 como VDP.

## Estado da evidência

| Afirmação | Estado |
|---|---|
| SG-1000 usou NEC D780C-1; há placa com Sharp LH0080A | Confirmado por inventário de placa secundário; revisão e detalhes de silício ainda requerem BOM/fonte primária |
| Master System usou NEC D780C-1 e Zilog Z80/Z80A em revisões | Confirmado por documentação/listagens de placas específicas |
| Game Gear VA0 tem Z80A discreto e VA1 integra CPU+VDP+SCA | Confirmado por manual de serviço |
| Game Gear ASIC VA1 = 315-5535 | Confirmado por inventários de revisão; confirmar marcação/placa ao selecionar uma revisão concreta |
| Game Gear ASIC universal = 315-5441 | Não confirmado; não codificar como fato |
| Genesis 315-5313 contém o Z80 | Contradito pelas fontes consultadas: 315-5313 é VDP |
| Genesis 315-5960/315-6123 integram Z80 em revisões específicas | Confirmado por fontes de revisão secundárias; associar cada número à revisão da placa antes de configurar |
| Todos os perfis têm exatamente 3,58 MHz | Simplificação não segura; clock deve ser especificado por oscilador/divisor e revisão/região |
| Clones têm comportamento idêntico em todo opcode não documentado | Não comprovado; exigir evidência diferencial ou manter política comum sem alegação universal |
