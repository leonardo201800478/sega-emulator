# F0 — inventário remoto e reconciliação das PRs

**Data da captura:** 2026-10-31  
**Baseline remoto de `main`:** `e2eff776ec248674da07d32ff480b70d9db49a1f`  
**Branch padrão:** `main`  
**Escopo:** auditoria read-only do código, dependências, issues, PRs, diffs e workflows. Nenhum runtime foi alterado nesta etapa.

## 1. Resultado executivo

A auditoria confirma que a extração de um único motor Z80 compartilhado é tecnicamente viável como refatoração incremental: os arquivos de CPU de SG-1000 e SMS contêm o mesmo código executável, enquanto os contratos de barramento e ALU já possuem componentes compartilhados no workspace, mas ainda são usados de forma desigual pelos cores.

A arquitetura temporal tem uma fundação real candidata na PR #315, com testes e workflows aprovados no SHA avaliado. Ela ainda não contém scheduler de máquina nem execução temporal da CPU. A PR #313 é a implementação concreta mais avançada de input Rust, mas sua integração precisa respeitar o relógio da máquina e resolver a fronteira de polling por frame. As PRs antigas #300, #301 e #298 são principalmente especificações/documentação; #297 contém código de ferramenta e está muito defasada em relação à `main`.

**Decisão F0:** baseline remoto e inventário estrutural concluídos; auditoria detalhada de todos os patches, testes e consumidores ainda está em andamento. Os resultados já permitem preparar os contratos F1, mas não autorizam merge nem alteração de gameplay.

## 2. Baseline e limitações da auditoria

- `main` foi consultada novamente e o último commit remoto observado foi `e2eff776ec248674da07d32ff480b70d9db49a1f`.
- Os sete heads de PR abaixo foram resolvidos diretamente pelos nomes de suas branches e comparados com `main`; os SHAs são os heads efetivamente observados nesta captura.
- Foram consultados metadados, listas completas de arquivos alterados, estatísticas de diff e workflows por SHA; os jobs retornados foram verificados individualmente. Patches foram lidos em amostras representativas, não em todos os arquivos de todas as PRs.
- A tentativa de `git ls-remote`/sincronização de clone local falhou por resolução DNS de `github.com`. Portanto, testes locais, builds locais e inspeção de working tree local estão **NOT RUN**. A evidência de CI descrita abaixo é remota e vinculada aos SHAs listados.
- Não foi executado agente independente A4 nesta rodada. A auditoria aqui é do coordenador, com evidência das APIs e workflows do GitHub; não deve ser descrita como revisão independente por IA.

## 3. Inventário Z80 em `main`

### 3.1 CPU SG-1000 e SMS

Arquivos consultados:

| Arquivo | Linhas | Blob SHA |
|---|---:|---|
| `native/sg1000_core/src/cpu.rs` | 3.487 | `b561e4e921b0940d37ed560785394dfdb31a83d9` |
| `native/sms_core/src/cpu.rs` | 3.488 | `a8d358e2c3e28064d204da96e281cd324811f1ee` |

Após remover comentários e linhas em branco e normalizar espaços, os dois arquivos têm **3.257 linhas executáveis idênticas, na mesma ordem**. O código difere nos cabeçalhos/documentação, mas não no conteúdo executável normalizado. Isso é evidência direta de duplicação quase integral, não apenas semelhança conceitual.

O CPU usa somente referências a `crate::alu` e `crate::bus`; não há referência direta a memória, VDP, PSG, mapper ou outro módulo de plataforma no arquivo do executor. Isso reduz o risco arquitetural da extração, mas não elimina a necessidade de testar os contratos genéricos de barramento.

### 3.2 ALU e barramento

- O workspace raiz já inclui `native/z80_alu` e `native/z80_bus`.
- O código executável das ALUs locais de SG-1000 e SMS é idêntico ao código de `native/z80_alu/src/lib.rs`, após normalização de comentários/espaços.
- `native/sms_core/src/bus.rs` já reexporta os tipos de `sega_z80_bus`.
- `native/sg1000_core/src/bus.rs` ainda mantém uma cópia local do contrato Z80, além do adaptador de memória SG-1000.
- `native/sms_core` depende de `sega-z80-bus`; `native/sg1000_core` ainda não depende dos crates compartilhados Z80.

**Consequência:** o novo crate `sega-z80-cpu` deve reutilizar `sega-z80-bus` e `sega-z80-alu`, e os cores de máquina devem manter somente adaptadores de barramento/perfil. Não criar uma terceira cópia da ALU nem uma segunda API de trace.

### 3.3 Contrato temporal existente

O contrato compartilhado de barramento já expõe snapshots, traces de instrução, eventos lógicos e amostras T-state. A própria documentação do contrato limita a instrumentação temporal a famílias selecionadas; ela não constitui trace completo por opcode nem sequência física de pinos validada em hardware.

Issues de controle ainda abertas:

- [#158 — matriz de opcodes e semântica](https://github.com/leonardo201800478/sega-emulator/issues/158): espaços BASE/CB/ED/DD/FD/DDCB/FDCB, prefixos, flags, WZ/Q/R e cobertura demonstrável.
- [#159 — interrupções, estado e temporalidade](https://github.com/leonardo201800478/sega-emulator/issues/159): reset, HALT, EI delay, INT/NMI, IM0/1/2, acknowledge, estado e separação entre T-states agregados e bus trace.

A PR #314 cobre um caso de fronteira útil, mas não fecha a matriz da #158 nem a auditoria de interrupções da #159.

## 4. PRs abertas — comparação com o HEAD atual

Todas as PRs abaixo foram encontradas abertas. Os valores ahead/behind são relativos ao HEAD remoto `e2eff...`; merge-base e head foram retornados pelo endpoint de comparação.

| PR | Head SHA | Ahead / Behind | Merge-base | Conteúdo e avaliação |
|---|---|---:|---|---|
| [#315 Temporal Core](https://github.com/leonardo201800478/sega-emulator/pull/315) | `bb1aec75cf99c1ffd49f4e4c591250de43f1c0cb` | 15 / 13 | `3cf846d688798080d35f997f6e0d34680372d7e3` | Crate `sega-temporal-core`, relógios racionais, fase e fila determinística. T0; ainda sem scheduler de máquina. |
| [#314 Z80 LD (nn),HL](https://github.com/leonardo201800478/sega-emulator/pull/314) | `7ff733f79d38f94e21bdb01f989b6eea5694f47e` | 3 / 13 | `3cf846d688798080d35f997f6e0d34680372d7e3` | Teste de wraparound little-endian e ordem lógica de escrita; sem alteração do executor. Preservar e portar para o crate compartilhado. |
| [#313 Input Rust + SDL3](https://github.com/leonardo201800478/sega-emulator/pull/313) | `1c64734c56413276b27ab0d85d9a8ec6978f3c41` | 106 / 13 | `3cf846d688798080d35f997f6e0d34680372d7e3` | Runtime compartilhado de input Rust, SDL3 Windows, perfis e ponte PyO3/Qt. Candidata principal; exige adaptação temporal e validação física. |
| [#300 Input Rust — especificação anterior](https://github.com/leonardo201800478/sega-emulator/pull/300) | `eeb8b8ae37f7ce116d2211fccd338afe4c29b3fb` | 1 / 210 | `2318fb35715c49c26e63f71debadacdb1816c79c` | Somente documento de migração. Preservar critérios de aceitação; não integrar segundo runtime de input. |
| [#301 Video Rust](https://github.com/leonardo201800478/sega-emulator/pull/301) | `d2d15431896d52f20929a56b72efdfd24aeb9b8e` | 1 / 210 | `2318fb35715c49c26e63f71debadacdb1816c79c` | Somente documento/plano. Preservar requisitos de ownership, surface lifecycle, frame pacing e separação do relógio lógico. |
| [#298 Modelo funcional Qt](https://github.com/leonardo201800478/sega-emulator/pull/298) | `f821e3e0acee8b1ad1f056eb31e18e3744cfe17a` | 10 / 210 | `2318fb35715c49c26e63f71debadacdb1816c79c` | Dois documentos com 35 decisões funcionais. Preservar o modelo; reconciliar com o frontend atual antes de implementar. |
| [#297 Auditoria de ROMs SMS](https://github.com/leonardo201800478/sega-emulator/pull/297) | `3b19837cacd18a048e313e5a8f5a86c97a02837d` | 27 / 272 | `41c32007ae961b78b3d8fa6c604af9f01159ffad` | Ferramenta Qt/Python que executa instâncias SMS isoladas no runtime Rust e exporta telemetria sem bytes de ROM/BIOS. Código precisa rebase e revalidação extensa. |

### 4.1 Workflows observados por SHA

| PR | Workflows/jobs consultados | Resultado remoto |
|---|---|---|
| #315 | Rust formatting, Workspace Clippy, Workspace Rust tests, suíte externa Z80, CI quality, validação sequencial SG-1000, Rust SG-1000 Workspace, Qt Rust SG-1000 integration e SMS Core Fast | **PASS** — todos os jobs retornados concluídos com sucesso no SHA `bb1aec...` |
| #314 | CI quality, SMS Core Fast, SMS Z80 Platform Validation e Qt Rust SG-1000 integration | **PASS** — todos os jobs retornados concluídos com sucesso no SHA `7ff733...` |
| #313 | CI quality, Python 3.14 quality, Qt tests, UI/controller, Rust SG-1000 Workspace, SMS Core Fast, SMS Z80 Platform Validation, Qt Rust integration e validações sequenciais | **PASS** — todos os jobs retornados concluídos com sucesso no SHA `1c647...` |
| #297 | CI, Qt frontend, UI/controller, Qt Rust integration, integrity/determinism e validação funcional | **PASS** — todos os jobs retornados concluídos com sucesso no SHA `3b198...` |
| #298 | CI quality | **PASS** no SHA documental `f821...` |
| #300 | CI quality | **PASS** no SHA documental `eeb8...` |
| #301 | CI quality | **PASS** no SHA documental `d2d1...` |

Os resultados acima comprovam os workflows executados nos SHAs de head listados, não a integração dessas PRs com a `main` atual. O status combinado tradicional retornou lista vazia; a evidência positiva foi obtida nos runs/jobs individuais. A consulta de workflows do próprio SHA `e2eff...` não retornou runs nesta captura. Portanto, **não declarar CI PASS para o HEAD atual de main** com base nesta auditoria.

## 5. Auditoria de integração por PR

### 5.1 #315 — Temporal Core

Código observado:

- `MachineTime`: timestamp inteiro de 128 bits; unidade definida pelo perfil da máquina.
- `ClockRatio` / `ClockDomain`: conversão racional sem ponto flutuante, fase preservada, monotonicidade e overflow verificados.
- `EventPriority` / `EventQueue<T>`: ordenação por timestamp, prioridade numérica e sequência de inserção; bloqueia inserção causal retroativa em timestamp já despachado.
- Testes para chunk invariance, fase fracionária, avanço retroativo, overflow, ordenação e causalidade.

Limites declarados e confirmados pelo conteúdo:

- Não há `MachineScheduler`, micro-operações Z80, bus cycles completos, WAIT, contenção, eventos raster ou integração PSG.
- Não há frequências específicas de console codificadas como verdade universal.
- O crate não altera os runtimes SG-1000/SMS.

**Decisão:** manter #315 como fundação temporal T0 candidata. A fase T1 deve acrescentar scheduler headless por instância de máquina, relógio virtual, safe points e política delta-cycle, sem depender de Qt/SDL/áudio/vídeo. A integração com CPU e dispositivos permanece pendente. Não declarar `TEMPORAL_PASS`.

### 5.2 #314 — regressão Z80

O teste novo verifica `LD (nn),HL` com `nn=0xFFFF`, escrita little-endian em `0xFFFF` e `0x0000`, wrap de WZ, preservação de registradores/flags, incremento de R, PC final e 16 T-states nominais.

A ordem verificada é de chamadas lógicas ao barramento, não uma sequência de eventos por T-state. O vetor SingleStepTests citado tem ancestralidade no Ares; é evidência útil de regressão, não oráculo independente de hardware.

**Decisão:** preservar o teste como gate da extração Z80 e portar o cenário para o crate compartilhado. Não interpretar o PASS dos workflows como cobertura completa do opcode ou da matriz BASE.

### 5.3 #313 e #300 — input

A #313 fornece `sega-input-runtime`, contratos host-independent, estado bruto de botões/eixos/hats, perfis, hotplug e backend SDL3 dinâmico inicialmente Windows. O backend mantém handles na thread de criação e faz scan de hotplug a cada 250 ms. A ponte PyO3 expõe máscaras compactas.

Pontos que exigem decisão/validação antes da integração:

1. `DeviceReading.timestamp_ns` é monotônico do host; deve ser convertido/associado à timeline da máquina na fronteira, sem permitir que o relógio do host avance a emulação.
2. `NativeInputManager` é `#[pyclass(unsendable)]` e `poll_actions` é chamado pela fachada Python na fronteira de input do frame. Não há polling Python das APIs físicas, mas existe uma chamada PyO3 por frame; medir custo e latência e avaliar como o scheduler consumirá eventos ordenados.
3. `set_backend_preference` atualmente valida `auto`/`sdl3`, mas não altera seleção de backend. Corrigir o contrato ou não expor uma preferência que ainda não tem efeito.
4. SDL3 é o único backend físico implementado nesta fatia. A matriz de controles físicos, hotplug real e latência device-to-runtime/button-to-photon permanece **NOT RUN**.\n5. O backend mantém um único contador `sequence` e o incrementa ao iterar `HashMap::values()` em `poll_into`. Isso não implementa sequência por dispositivo e a ordem entre dispositivos pode variar entre processos. Antes de usar essas leituras como eventos temporais, definir sequência por dispositivo e um desempate total determinístico (por exemplo timestamp, device id e sequência), sem depender da ordem de iteração do HashMap.

A #300 é documentação anterior, não deve originar um segundo runtime. Seus critérios (perfis, dispositivos genéricos, hotplug, compatibilidade e latência) permanecem requisitos de aceitação para a #313.

**Checagem da API SDL3 (documentação oficial):** a versão `3.4.18` usada pela PR é a release estável mais recente listada em 2026-10-31. O SHA-256 do ZIP Windows x64 no script de instalação coincide com o hash publicado na release oficial. Os valores numéricos usados para classificar joystick type (2 wheel, 3 arcade stick, 4 flight stick e 8 arcade pad) correspondem ao enum oficial. O código também respeita a ownership de `SDL_GetGamepadJoystick`: fecha o gamepad e não fecha separadamente o joystick emprestado. Isso valida os pontos de ABI/ownership revisados, não a compatibilidade de todos os controles físicos.

Referências oficiais:
- [SDL 3.4.18 release](https://github.com/libsdl-org/SDL/releases/tag/release-3.4.18)
- [SDL_JoystickType](https://wiki.libsdl.org/SDL3/SDL_JoystickType)
- [SDL_GetGamepadJoystick](https://wiki.libsdl.org/SDL3/SDL_GetGamepadJoystick)
- [SDL_CloseGamepad](https://wiki.libsdl.org/SDL3/SDL_CloseGamepad)

### 5.4 #301 — vídeo

A PR é especificação documental. A integração deve manter Qt como shell de janela/UI e transferir ownership de renderer, recursos e apresentação para Rust somente após decisão de API, surface lifecycle, device loss, fallback e medições. VSync/VRR/frame pacing não pode avançar o relógio lógico da máquina. Não há migração de vídeo implementada por essa PR.

### 5.5 #298 — frontend Qt

A PR contém modelo funcional e arquitetura-alvo documental, incluindo 35 decisões. Preservar explicitamente:

- Pause mantém sessão e ROM carregadas.
- Stop encerra sessão, persiste dados nativos pendentes e descarrega ROM sem fechar Qt.
- Migração defensiva de configurações e registro de incompatibilidades.
- Separação de Diagnóstico e Debug técnico.
- Relatório de backend solicitado/ativo/fallback/motivo/capacidades.
- Restauração global e por categoria com confirmação.

A PR está 210 commits atrás; não integrar sua branch sem reconciliação com os componentes atuais e rastreamento de callbacks/consumidores.

### 5.6 #297 — auditoria em lote SMS

A ferramenta executa cada ROM em uma instância isolada de `SMSMachine`, sem frame pacing, e registra hashes/telemetria sem gravar bytes de ROM ou BIOS. A classificação `BOOT_CONFIRMED` exige avanço de frames e mais de uma cor RGB; ausência de variação visual é inconclusiva, não falha automática.

É ferramenta de diagnóstico em Python, não um segundo runtime de hardware. Revalidar após a estabilização da API pública/headless do SMS e garantir que a sessão de jogo ativa permaneça intacta. O head está 272 commits atrás e requer rebase/revisão antes de qualquer integração.

## 6. Contratos arquiteturais que F1 deve congelar

1. **Z80:** um único crate `sega-z80-cpu`; ALU e bus contract compartilhados; adaptadores de plataforma finos. O crate CPU não conhece mapas/registradores de console.
2. **Execução híbrida:** stepping por instrução para compatibilidade e testes; execução temporal retomável consumindo a mesma semântica/micro-operações. Proibido manter dois decodificadores.
3. **Tempo:** um scheduler por máquina, múltiplos domínios de clock; unidade de `MachineTime` documentada por perfil. Eventos host são convertidos na fronteira.
4. **Input:** Rust possui captura/normalização; eventos são ordenados e entregues à máquina. A integração não pode fazer polling físico em Python nem deixar SDL/Qt definir o tempo emulado.
5. **Vídeo:** apresentação física não avança o scheduler; backend efetivo/fallback/capacidades são observáveis.
6. **Qt/PyO3:** comandos, configurações, eventos e snapshots coarse-grained; sem callbacks Python por instrução, acesso de memória, pixel ou amostra PCM.
7. **Ferramentas:** #297 consome API pública headless e instâncias isoladas, sem acoplar-se a internals de core.

## 7. Sequência de integração atualizada

1. **F1 — ADRs e contratos:** registrar crate Z80, bus/ALU compartilhados, execução híbrida, ownership temporal, input e host boundaries.
2. **T1 — scheduler headless:** desenvolver sobre a fundação #315 em branch própria, depois de revisar seus contratos e testes.
3. **Z1 — extração Z80:** criar `native/z80_cpu` / `sega-z80-cpu`; portar testes existentes e os gates #158/#159; adaptar SG-1000 e SMS sem mudança semântica.
4. **Reconciliar #314:** atualizar sua base e preservar o teste no novo crate, repetindo checks no SHA candidato.
5. **Reconciliar #313:** integrar input sem duplicar #300, com eventos compatíveis com timeline e validação física pendente explicitamente.
6. **#298/#301:** preservar documentos e incorporar requisitos às APIs/ADRs, sem tratar documentos como implementação.
7. **#297:** rebasear e adaptar à API pública final do SMS/headless runner.
8. Somente após os gates, integrar os componentes e remover os executores duplicados.

Nenhuma branch será atualizada em lote. Cada PR deve ser reconciliada individualmente, com diff, conflitos, testes e workflows reavaliados no SHA final.

## 8. Estado e próximo gate

| Item | Estado |
|---|---|
| Baseline remoto e heads das sete PRs | **Inventário estrutural concluído; auditoria profunda pendente** |
| Duplicação executável Z80 SG/SMS | **Confirmada** |
| Reuso potencial de ALU/bus compartilhados | **Confirmado; integração pendente** |
| Temporal Core T0 | **IMPLEMENTED na PR #315; INTEGRATION_PENDING** |
| MachineScheduler T1 | **NOT IMPLEMENTED** |
| Z80 compartilhado | **INTEGRATION_PENDING** |
| Input Rust SDL3 | **Implementado na PR #313; validação física NOT RUN; integração pendente** |
| Vídeo Rust | **Especificação documental; implementação NOT RUN** |
| Modelo Qt F01–F35 | **Documentado na PR #298; reconciliação pendente** |
| Auditoria em lote SMS | **Implementada na PR #297; rebase/integração pendentes** |
| A4 — revisão independente | **NOT RUN** |
| Testes/build locais nesta sessão | **NOT RUN — DNS impediu sincronização** |

**Próximo gate:** concluir a leitura detalhada dos patches/consumidores e executar A4 independente sobre F0 + ADRs. As ADRs F1 foram redigidas como propostas, ainda não aprovadas como APIs definitivas. Só depois desse gate abrir branches de implementação para o crate Z80 compartilhado e T1 temporal, sem sobrepor arquivos. Nenhum merge é autorizado por este relatório.


## 9. Correção de maturidade — auditoria comparativa SMS × SG-1000

A igualdade executável descrita na seção 3 não deve ser interpretada como maturidade equivalente. A auditoria específica [SMS × SG-1000](sms-z80-maturity-audit-2026-10-31.md), feita no mesmo baseline `d54d2751ce19a11c2591b400f1ef43b00f797b6c`, confirma:

- O SMS é a origem histórica consolidada e o baseline de validação semântica; o cabeçalho do SG-1000 declara que seu executor foi portado da implementação SMS.
- Os executores e ALUs atuais são idênticos no código executável normalizado. O SG não mantém uma variante semântica independente que deva ser promovida.
- Ambos têm 49 testes inline no arquivo CPU, mas o SMS possui mais 37 arquivos de testes dedicados ao Z80. O SG não possui `native/sg1000_core/tests`; seus testes específicos estão principalmente inline em CPU/machine.
- O SMS já reexporta `sega-z80-bus`; SG mantém cópia local do contrato.
- A suíte SMS inclui vetores SingleStepTests, modelos combinatórios, prefixos/indexed-CB, ED, interrupções, integração de memória e traces ED delimitados, além do runner externo fixado. Esses testes não são todos oráculos independentes: a ancestralidade Ares do SingleStepTests e os limites dos modelos internos estão documentados.
- A PR #160 rejeitou duas diferenças propostas a partir do SG após falhas nos vetores Q de `EX AF,AF'` e DAA. A PR #201 corrigiu DAA posteriormente no baseline SMS; o código SG atual já contém a mesma correção.

**Direção corrigida:** extrair o novo crate a partir do executor SMS consolidado e migrar os testes de CPU/ALU portáveis para o crate compartilhado. Não duplicar a suíte em SG. Manter testes de plataforma SMS e criar/fortalecer testes próprios de integração SG-1000. `native/z80_alu` é código duplicado equivalente, não oráculo independente.
