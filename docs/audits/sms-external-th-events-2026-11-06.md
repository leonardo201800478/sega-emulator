# Auditoria — eventos TH externos e Light Phaser no caminho temporal SMS

**Data:** 2026-11-06  
**Baseline:** `c2194cbf6e44d4fc1098d6cc3ddc82b160357439`  
**Escopo:** comportamento de bordas TH originadas por periféricos, caminho Light Phaser e lacunas de integração com `SmsTemporalMachine`.  
**Natureza:** auditoria de arquitetura e evidências; nenhuma alteração de runtime.

## 1. Conclusão

A PR #584 integrou a captura do H-counter provocada por software ao escrever no I/O control `$3F` (saída baixa → entrada alta) no boundary de conclusão do barramento. Isso não completa a captura provocada por periféricos.

A auditoria confirma três caminhos distintos que não devem ser confundidos:

1. **Software / I/O control:** já integrado no `SmsTemporalBus::write_port`; a escrita é aplicada após o Scheduler avançar o VDP até o timestamp de conclusão do ciclo.
2. **Light Phaser sintético legado:** `SmsVdp::set_light_phaser_input` e `complete_scanline` produzem um pulso aproximado por scanline e escrevem diretamente `h_counter_latch = 0x08 + x/2`. Isso não é uma captura do contador no timestamp de uma borda TH real.
3. **Borda TH externa no caminho temporal:** não há evento tipado de entrada TH no `SmsTemporalEvent`, nem método de entrada temporal correspondente em `SmsTemporalMachine`; permanece ausente.

Portanto, não declarar a captura externa TH integrada. O estado da lacuna é **NOT IMPLEMENTED / NOT RUN** no caminho temporal.

## 2. Evidência documental — SMS Power

A documentação SMS Power para Light Phaser descreve:
- TL como trigger e TH como sensor de luz;
- ambas as linhas configuradas como entradas;
- o sensor ativo (luz detectada) puxa TH para baixo;
- o software observa TH ativo em `$DD` e lê H/V counter para determinar a posição;
- a transição de retorno à escuridão (TH volta a alto) encerra o trecho visível, mas não é a borda que o software usa para registrar o limite direito.

Fonte: https://www.smspower.org/Development/LightPhaser

A página de portas confirma que TH é uma linha bidirecional e que periféricos podem conduzir entradas externas; joypad passivo deixa TH desconectado. Mark III não possui TH; o SMS japonês possui peculiaridade de readback das saídas TH.

Fonte: https://www.smspower.org/Development/PeripheralPorts

Essas fontes descrevem a interface e o comportamento observável, mas não definem o atraso interno exato entre borda de sensor e captura de H-counter.

## 3. Engenharia reversa — MAME

Fontes consultadas:
- https://github.com/mamedev/mame/blob/master/src/mame/sega/sms_m.cpp
- https://github.com/mamedev/mame/blob/master/src/mame/sega/sms.cpp

O modelo MAME mantém estado independente para TH1/TH2 e executa callbacks quando o dispositivo externo altera o nível:

- O callback só altera o comportamento de captura quando o TH correspondente está configurado como entrada (`$3F bit 1` ou `bit 3`).
- TH externo baixo arma um latch interno do controlador.
- Quando TH volta de baixo para alto, se o nível anterior era baixo, MAME agenda a captura do H-counter por timer.
- A captura não é feita diretamente no callback: `lphaser_hcount_latch()` agenda `lphaser_th_generate` para o HPos atual acrescido de `m_lphaser_x_offs`.
- Na configuração padrão sem offset customizado por mídia, MAME usa 19 posições horizontais. O offset também pode vir do cartucho/card/expansion slot.

Esse comportamento é um modelo de emulação comparativo. Não é evidência de medição física independente; o offset específico não deve ser copiado para o projeto como constante universal sem identificar a origem/variante e sem testes.

A escrita de software em `sms_io_control_w` é um caminho distinto: se o novo estado configura TH como entrada, o estado anterior tinha direção/nível de saída baixos e o estado externo atual é alto, MAME captura H-counter no próprio boundary da escrita.

## 4. Confronto com o código Sega atual

### 4.1 `SmsTemporalBus`

`native/sms_core/src/temporal_bus.rs`:
- `write_port` implementa o caso software de saída-baixa → entrada-alta.
- `SmsTemporalEvent` atualmente contém BusCompletion, InternalCycleComplete e M1TStateComplete; não há evento TH externo.
- `SmsTemporalBus::set_controller` atualiza botões sem avançar tempo; não modela TH externo.
- `SmsTemporalMachine` expõe `set_controller`, mas não um contrato temporal para transições TH de periféricos.
- A leitura de `$DD` deriva TH do estado do Light Phaser sintético, sem uma sequência de bordas temporalmente agendada.

### 4.2 `SmsVdp` e caminho legado

`native/video_core/src/sms/vdp.rs`:
- `set_light_phaser_input(x,y,trigger)` guarda coordenadas e trigger.
- `complete_scanline` ativa `light_pulse` na linha Y indicada quando o trigger está pressionado.
- O mesmo bloco calcula `h_counter_latch = 0x08 + x/2` diretamente da coordenada lógica X.
- Esse valor é sintético e aproximado; não resulta da conversão do contador no instante de uma borda TH externa.
- `SmsMachine` copia o nível sintético de `light_pulse` para `SmsIo`; o caminho `SmsTemporalMachine` não possui integração equivalente de input Light Phaser.

Não remover nem reinterpretar esse caminho legado sem testes de compatibilidade; ele é uma aproximação funcional separada da integração temporal.

## 5. Critérios de aceitação para uma futura implementação

Uma alteração futura deve, no mínimo:

1. Modelar TH1 e TH2 como sinais externos independentes, preservando polaridade ativa-baixa.
2. Aceitar transições em timestamps explícitos de `MachineTime`; não aplicar callbacks host fora do Scheduler nem amostrar a entrada apenas no próximo frame/instrução.
3. Serializar eventos pendentes, níveis TH atuais e estado de borda/latch no snapshot; validar restore antes e depois da transição.
4. Aplicar a transição somente se o TH correspondente estiver configurado como entrada, conforme o modelo de periférico.
5. Separar a borda externa do caso de software de `$3F`; ambos devem possuir testes independentes.
6. Determinar e documentar a política de atraso do Light Phaser: offset padrão, metadado de cartucho, perfil/variante e unidades. Não fixar 19 HPos como verdade física universal.
7. Provar que a leitura de `$DD` observa o nível correto e que `$7F` retém o H-counter capturado nos boundaries definidos.
8. Manter o frontend fora da lógica de raster/timing; ele apenas fornece eventos normalizados e timestamps ou amostras convertidas para o contrato do backend.
9. Testar equivalência entre execução contínua e segmentada, inclusive eventos TH entre blocos e snapshot/restore com evento pendente.
10. Não declarar precisão dot-level ou Light Phaser físico completo sem validar a geração do pulso contra raster/pixel e evidência independente.

## 6. Matriz de estado

| Item | Estado |
|---|---|
| Software TH output-low → input-high no temporal bus | IMPLEMENTED; testes da PR #584 |
| TH externo baixo/alto com timestamp explícito | NOT IMPLEMENTED |
| Latch de H-counter disparado por borda externa TH | NOT IMPLEMENTED |
| Atraso/offset de Light Phaser por mídia | NOT VERIFIED |
| Light Phaser sintético por scanline no VDP legado | IMPLEMENTED_APPROXIMATE |
| Light Phaser integrado ao SmsTemporalMachine | NOT IMPLEMENTED |
| Snapshot/restore de transição TH externa pendente | NOT RUN |
| Validação em SMS físico | NOT RUN |

## 7. Decisão

Não adicionar callback imediato em `set_controller` ou `set_input`: isso violaria a propriedade temporal do Scheduler. Não copiar o offset MAME de 19 HPos para uma regra universal. A próxima implementação deve começar pelo contrato de eventos externos tipados e serializados, e só depois ligar o modelo de Light Phaser ao raster com política de offset explícita.

Esta auditoria fecha apenas a identificação da lacuna e seus critérios; não fecha a implementação.
