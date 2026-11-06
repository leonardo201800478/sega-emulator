# Arquitetura — Sega Emulator

## Produto

O produto é nativo Rust. A interface é egui/eframe; não há frontend Python/Qt nem binding PyO3. A branch de migração removeu os arquivos e workflows legados correspondentes.

## Camadas

| Camada | Responsabilidade |
|---|---|
| Hardware Rust | CPU Z80 compartilhada, ALU, barramento, memória, mapper, VDP, PSG e interrupções |
| Tempo Rust | MachineScheduler e primitives temporais; única autoridade de clocks/eventos |
| Runtime SMS | `rust/runtime/sms-runtime`; worker dono da máquina e comandos coarse-grained |
| Hosts Rust | D3D12, WASAPI/XAudio2 e input; consomem dados do runtime sem controlar tempo emulado |
| Aplicação egui | Menus, apresentação, seleção/carregamento de mídia e status; sem lógica de hardware |

## Estado atual

O worker SMS temporal foi extraído para um crate Rust puro e desacoplado do backend físico de áudio por uma factory de sessão executada na thread do worker. A integração com o shell egui ainda não foi implementada. O SMS core continua híbrido, e a separação entre caminhos temporais e atômicos permanece pendente.

## Regras

- Uma única implementação compartilhada do Z80.
- Uma única máquina SMS temporal ativa; nenhum fallback atômico.
- A GUI envia comandos de sessão, não instruções, ciclos ou timestamps de hardware.
- O Scheduler define causalidade e ordenação de eventos.
- Áudio/vídeo físicos não podem alterar a timeline emulada.
- Sem JIT, frame skipping, hacks temporais ou otimizações especulativas.

## Validação

A compilação do crate não comprova fidelidade funcional. São obrigatórios os gates Rust, os testes temporais/determinísticos e a validação física de D3D12, áudio e input. O harness externo ZEXDOC/ZEXALL está NOT RUN até sua substituição por ferramenta Rust.
