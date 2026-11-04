# Contrato técnico E7.2 — interrupções NMI/INT e saída de HALT

**Data:** 2026-11-04  
**Baseline:** `main` — `b1b5ced5c3d6053f8d59c8f4d932f40518db1064`  
**PR de origem:** #490 (E7.1 HALT/M1), merge commit `b1b5ced5c3d6053f8d59c8f4d932f40518db1064`  
**Referência normativa principal:** ZiLOG, *Z80 CPU User Manual*, UM008011-0816, seções “Interrupt Request/Acknowledge Cycle”, “Nonmaskable Interrupt Response”, “HALT Exit” e “CPU Response”.  
**Escopo deste documento:** fixar requisitos verificáveis antes de alterar o microsequenciador compartilhado. Não declara implementação de interrupções.

---

## 1. Estado real da baseline

O núcleo ativo está em `native/z80_cpu`. Na baseline auditada:

- `Z80MicroSequencerState` e `Z80MicroSequencer` já contêm `iff1`, `iff2`, `interrupt_mode`, `ei_delay`, `nmi_pending`, `int_pending` e `halted`.
- O codec de snapshot inclui esses campos e o schema é 28.
- BASE `DI (F3)` limpa IFF1/IFF2 e EI delay; BASE `EI (FB)` seta IFF1/IFF2 e `ei_delay = 1`.
- `begin_instruction()` ainda inicia diretamente um `OpcodeFetch`; não arbitra NMI/INT antes do fetch.
- Não há API pública de entrada de NMI/INT nem fases de serviço de interrupção no microsequenciador.
- `Z80BusOperation` atualmente cobre OpcodeFetch, MemoryRead/Write e PortRead/Write; não representa acknowledge de INT.
- O adaptador temporal SMS converte essas operações em ciclos do barramento, mas seu próprio cabeçalho declara interrupções externas ainda não conectadas nesta fatia temporal.
- Os campos serializados existentes não comprovam que NMI/INT sejam reconhecidas, amostradas ou atendidas.

Portanto, os campos presentes são estado arquitetural preparatório, não funcionalidade implementada.

## 1.1 Referência funcional existente: executor atômico

A auditoria ampliada encontrou semântica NMI já implementada no executor atômico compartilhado, em `native/z80_cpu/src/cpu.rs`. Isso evita reimplementar a regra arquitetural a partir de suposições, mas não substitui a implementação retomável.

- `request_nmi()` seta `nmi_pending`.
- `step_inner()` prioriza NMI antes de INT, HALT e fetch normal.
- `service_nmi()` consome a solicitação, sai de HALT, incrementa R preservando o bit 7, copia IFF1 para IFF2, limpa IFF1, empilha o PC, define PC/WZ em `0x0066` e retorna 11 T-states agregados.
- `push16()` decrementa SP e escreve primeiro o byte alto, depois decrementa novamente e escreve o byte baixo.
- A família RETN/RETI usa `IFF1 = IFF2` após recuperar o PC.
- O teste `native/sms_core/tests/z80_nmi_halt_semantics.rs` cobre NMI saindo de HALT mesmo com INT mascarada, PC de retorno, bytes empilhados, destino e incremento de R.
- O teste não cobre IFF1 inicialmente habilitada, preservação de IFF2 por NMI, request/snapshot durante cada fase, BUSREQ ou trace de barramento.

**Limite do oráculo atômico:** o executor retorna 11 T-states agregados e chama `push16()` diretamente. Ele não expõe acknowledge/fetch NMI, WAIT, M-cycles, fases ou operações de barramento. Portanto, serve como referência para estado arquitetural final, mas não prova a sequência temporal. A implementação E7.2A deve reproduzir a semântica acima por requests retomáveis, sem copiar a mutação atômica nem assumir que 11 T-states agregados provam o timing.

A auditoria também confirmou que `native/sms_core/tests/z80_interrupt_semantics.rs` cobre o executor atômico para IM0/1/2, mas esses casos não exercitam o microsequenciador nem o adaptador temporal.

## 2. Requisitos normativos extraídos do manual Zilog

### 2.1 Amostragem e prioridade

1. NMI e INT são amostradas na borda de subida do clock no final da instrução; durante HALT, as linhas são amostradas na borda de subida de cada T4. Durante um ciclo BUSREQ, nenhuma das duas é atendida.
2. NMI tem prioridade sobre INT e não pode ser mascarada por software.
3. INT só é aceita se IFF1 estiver habilitada. O manual também declara que, durante um ciclo BUSREQ, a CPU não pode ser interrompida por NMI nem INT. A interação com BUSREQ deve permanecer explícita enquanto a máquina temporal não expuser esse sinal.
4. Se NMI e INT elegível estiverem presentes na mesma janela de amostragem de HALT, NMI deve ser atendida.
5. Uma solicitação não elegível não pode provocar saída espúria de HALT.

### 2.2 NMI

- O processador aceita NMI independentemente de IFF1.
- Ao aceitar NMI, IFF1 é limpa; IFF2 preserva seu valor anterior para que RETN possa restaurar IFF1.
- O PC de retorno é empilhado na memória externa e o PC passa a `0x0066`.
- A resposta de NMI é semelhante a uma leitura normal de memória; o dado lido é ignorado. Não tratar NMI como INT acknowledge nem solicitar vetor externo.
- RETN restaura IFF1 a partir de IFF2. A semântica da instrução RETN já existente deve ser auditada, não presumida correta apenas pela presença do opcode.

### 2.3 INT e acknowledge

- INT é uma solicitação mascarável, amostrada no limite de instrução; a aceitação exige IFF1 habilitada.
- O acknowledge de INT é um M1 especial: IORQ é ativado em vez de MREQ para que o dispositivo coloque o vetor no barramento.
- O manual especifica dois wait states automáticos no ciclo de acknowledge para permitir estabilização de prioridade em daisy-chain. Esses dois T-states não devem ser confundidos com WAIT externo adicional.
- O ciclo deve preservar o vetor recebido e a identidade da operação através de WAIT, suspensão, retry e snapshot/restore.
- Ao aceitar INT, a CPU desabilita as interrupções mascaráveis durante o serviço. A implementação deve verificar e testar IFF1/IFF2 de acordo com a tabela de estado do manual.

### 2.4 Modos IM 0, IM 1 e IM 2

- **IM 0:** o byte do acknowledge é o primeiro opcode da instrução fornecida pelo dispositivo; bytes subsequentes de instruções multi-byte são lidos da memória em ciclos normais. Não reduzir IM 0 a RST.
- **IM 1:** a CPU executa restart para `0x0038`, empilhando o PC de retorno.
- **IM 2:** formar o ponteiro com I como byte alto e o vetor fornecido pelo dispositivo como byte baixo; ler dois bytes consecutivos da tabela em little-endian para obter o PC de destino. O requisito de vetor par descrito no manual deve ser coberto por teste.
- O reset seleciona IM 0. A validação do estado restaurado deve rejeitar modos fora de 0–2.

### 2.5 EI / DI

- DI limpa IFF1 e IFF2.
- EI habilita IFF1/IFF2, mas a aceitação de INT deve respeitar a inibição até a instrução seguinte ser concluída.
- A contagem atual `ei_delay` é um campo de estado; sua transição deve ser especificada por fronteiras de instrução e testada para sequências EI;NOP;INT, EI;HALT;INT, EI;DI e NMI durante o atraso.
- NMI não deve ser bloqueada pelo atraso de EI.

### 2.6 HALT

- Cada M1 de retenção executa NOP interno, ignora o byte lido e preserva PC.
- Durante cada T4 de retenção, amostrar NMI/INT; sair de HALT na fronteira definida pelo manual e iniciar o acknowledge correspondente no ciclo seguinte.
- NMI prevalece sobre INT simultânea.
- A solicitação não elegível deve deixar a CPU em HALT e manter os ciclos M1 de refresh.
- Reset durante HALT ainda é um requisito separado e não deve ser considerado resolvido por E7.2.

---

## 3. Contrato arquitetural proposto para o microsequenciador

A implementação deve permanecer em `sega-cpus/native/z80_cpu`; nenhuma lógica de CPU ou timing pode ser transferida para Python.

### 3.1 Entradas de interrupção

- Expor operações explícitas para solicitar NMI e atualizar a linha/solicitação INT com seu vetor de acknowledge.
- Distinguir a borda de NMI (latch até aceite) do nível de INT (mantido pelo dispositivo até desassertar conforme contrato do host).
- Não apagar uma solicitação INT apenas porque o CPU está mascarado; a elegibilidade deve ser decidida na amostragem.
- Definir o significado exato de `int_pending: Option<u8>` ou substituí-lo por um estado que represente separadamente linha, vetor e latch. Não sobrecarregar `None` para representar estados eletricamente distintos.
- As APIs não avançam tempo. O Scheduler continua sendo o único proprietário do relógio.

### 3.2 Operações de barramento

- Adicionar uma operação específica para INT acknowledge, com semântica M1/IORQ e dado de retorno obrigatório.
- NMI não deve reutilizar essa operação: sua resposta é memory-read-like, com dado ignorado, seguida do empilhamento do PC.
- A representação temporal deve permitir distinguir MREQ/RD, M1 e IORQ. Não mapear INT acknowledge como OpcodeFetch, MemoryRead ou PortRead genérico se isso apagar sinais necessários.
- Os dois T-states automáticos de INT acknowledge devem estar representados de modo determinístico e independente de WAIT externo.
- O adaptador SMS temporal e os demais consumidores compartilhados devem tratar a nova operação de forma exaustiva; qualquer consumidor sem suporte deve falhar explicitamente, nunca reinterpretar silenciosamente.

### 3.3 Fases retomáveis mínimas

O desenho final pode escolher nomes diferentes, mas deve representar estados explícitos para:

1. arbitragem no boundary de instrução e seleção NMI > INT elegível > fetch normal;
2. ciclo de resposta NMI e commit de IFF1;
3. decremento de SP e escrita do byte alto/baixo do PC, em ordem de barramento verificada;
4. ciclo INT acknowledge, incluindo os dois T-states automáticos e captura do vetor;
5. dispatch IM0, IM1 e IM2;
6. leituras da tabela IM2 (low/high) e atualização do PC/WZ;
7. saída de HALT após amostragem em T4;
8. suspensão/retry em cada request de barramento e ciclo interno.

Não implementar o serviço como uma mutação atômica de PC/SP sem requests de barramento.

### 3.4 Snapshot e invariantes

- Snapshot deve preservar linha/latch de NMI, nível de INT, vetor, modo, IFF1/IFF2, EI delay, HALT, fase de serviço, endereço/valor do request, request ID, WAIT externo e progresso dos T-states automáticos.
- A versão 28 não pode ser mantida se a estrutura serializada ou os tags do codec mudarem de forma incompatível. Incrementar schema e testar rejeição de versões antigas/inválidas conforme política do codec.
- Restore em cada safe point deve produzir o mesmo trace e estado final que execução contínua.
- Validar invariantes cruzadas: fase ↔ operação pendente ↔ endereço ↔ vetor ↔ IFF ↔ halted ↔ contadores M/T.

---

## 4. Plano de implementação em incrementos pequenos

| Incremento | Escopo | Critério de aceite |
|---|---|---|
| E7.2A | Contrato de entrada e serviço NMI | NMI edge-latched, prioridade, IFF1/IFF2, stack writes, PC=0066, RETN, WAIT e snapshots |
| E7.2B | Linha INT + acknowledge | IORQ/M1 distinto, vetor, dois T-states automáticos, WAIT externo e snapshots |
| E7.2C | IM 1 e IM 2 | RST 38, lookup IM2 low/high, vetor par, PC/WZ e stack |
| E7.2D | IM 0 | Primeiro opcode vindo do acknowledge; operandos subsequentes da memória; fluxo completo e retomável |
| E7.2E | HALT + interrupções + EI delay | Amostragem T4, NMI prioritária, EI delay e retomada sem interrupção perdida |
| E7.2F | Integração dos adaptadores e regressão | SMS/SG-1000 consumidores exaustivos, snapshots, WAIT e invariância de execução em blocos |

A ordem pode ser ajustada somente se uma dependência concreta exigir; não ampliar para E8/E9 antes de fechar os critérios desta etapa.

---

## 5. Matriz mínima de testes

- NMI com IFF1=0 e IFF1=1; IFF2 preservada; RETN restaura IFF1.
- NMI simultânea a INT: NMI vence; INT permanece conforme o nível/latch externo.
- INT com IFF1=0: não aceita; HALT permanece.
- INT com IFF1=1: aceita no boundary permitido; IFF e EI delay corretos.
- EI seguido por uma instrução: INT não aceita antes da conclusão da instrução seguinte.
- DI cancela a elegibilidade de INT; NMI continua elegível.
- HALT: NMI/INT amostradas em T4; bytes de memória durante M1 de retenção ignorados; PC não avança durante retenção.
- IM0: vetor RST e instrução multi-byte fornecida pelo dispositivo.
- IM1: PC empilhado e destino `0x0038`.
- IM2: vetor, endereço `(I << 8) | vector`, leitura low/high e destino.
- SP wrap em `0x0000/0xFFFF`; PC wrap; vetor/tabela nas bordas de página.
- WAIT zero, WAIT múltiplo e acknowledge com dois T-states automáticos.
- Snapshot/restore antes/depois de cada operação e entre bytes de stack/tabela.
- Rejeição de request ID incorreto, resposta sem dado, dado inesperado e snapshots incoerentes.
- Equivalência entre execução contínua e retomada em blocos.

Os testes devem separar semântica arquitetural, sequência de requests, M-cycles/T-states, WAIT, codec e integração. Testes que apenas comparam o microsequenciador ao executor atômico interno não constituem oráculo independente.

---

## 6. Gates e evidências

No SHA candidato da implementação:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Registrar separadamente:
- Rust fmt, Clippy, SMS direcionado, SG-1000 direcionado e workspace;
- Python pytest/Ruff;
- Windows x64 D3D12 e WASAPI/XAudio2;
- Scheduler deterministic stress;
- testes de interrupção e snapshots;
- traces temporais independentes.

CI da PR #490 (run #41762232781, HEAD `ed84fac03f5aa03c3438d67548354dd8c3c13999`) teve Rust fmt/Clippy/SMS/SG-1000/workspace PASS, Python pytest/Ruff PASS e Windows x64 D3D12/WASAPI/XAudio2 PASS. Scheduler deterministic stress foi SKIPPED. CI pós-merge #41765421167 da main SHA `b1b5ced5c3d6053f8d59c8f4d932f40518db1064`: Rust quality/unit regression, Python lint/regression e Windows x64 D3D12/WASAPI/XAudio2 PASS; Scheduler deterministic stress SKIPPED.

Nenhum desses resultados valida NMI/INT: E7.2 permanece NOT IMPLEMENTED até haver código, testes e evidência próprios. Não declarar SHORT_PASS/LONG_PASS/DIFFERENTIAL_PASS/TEMPORAL_PASS global.

---

## 7. Referências

- ZiLOG, *Z80 CPU User Manual*, UM008011-0816 (2016), pp. 12–20: ciclos de acknowledge, NMI, HALT Exit, IFF e modos IM0/IM1/IM2.
- Arquivo de referência do projeto: `um0080.pdf`.
- Runtime atual: `native/z80_cpu/src/sequencer.rs`, `micro_ops.rs`, `snapshot.rs`, `m1.rs`.
- Adaptador temporal atual: `native/sms_core/src/temporal_machine.rs`.
- Auditoria E7.1: `docs/audits/z80-e7-halt-m1-2026-11-04.md`.
