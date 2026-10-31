# Ordem direta de arquitetura — áudio exclusivamente em Rust

**Classificação:** ORDEM ARQUITETURAL OBRIGATÓRIA  
**Aplicação:** toda implementação presente e futura de áudio no Sega Emulator, em todas as plataformas e expansões Sega previstas no projeto.  
**Autoridade:** esta ordem complementa e torna explícita a política Rust-first do projeto. Em caso de ambiguidade em documentos de áudio, prevalece a regra aqui definida.

## 1. Ordem direta

**Todo código executável de áudio e toda lógica de hardware relacionada ao som DEVEM ser implementados em Rust. Python NUNCA será utilizado como runtime de emulação, processamento ou execução de áudio.**

Esta determinação vale para o caminho ativo de produção, não apenas para novos componentes. É proibido introduzir um runtime Python paralelo, temporário, experimental ou de fallback para suprir funcionalidades ainda ausentes em Rust.

O runtime Rust é a única autoridade para o comportamento dos chips, avanço temporal, geração de áudio, processamento digital, mistura, buffers e entrega ao dispositivo de saída.

## 2. Responsabilidades obrigatórias de Rust

Devem ser construídos e executados em Rust:

- Modelos digitais dos chips e dispositivos de áudio: registradores, leitura/escrita, estado interno, divisores, timers, FIFO, interrupções e geração de sinal.
- Integração dos chips com CPU, barramento, DMA, scheduler, relógios e timeline determinística da máquina.
- Eventos temporizados de escrita/leitura e avanço dos dispositivos até os timestamps definidos pelo hardware.
- Geração de amostras e fontes digitais mono ou estéreo.
- Roteamento, pan e ganhos próprios do hardware, mantendo-os separados das preferências de volume do host.
- Mixer multifuente, DSP, filtros, resampling, conversão de formato, clipping/quantização e buffers.
- Filas limitadas, backpressure, underrun/overflow, métricas e recuperação de falhas.
- Ciclo de vida da sessão de áudio e workers nativos.
- Integração direta com APIs de saída do sistema operacional, incluindo WASAPI e XAudio2 no Windows.
- Save/load de estados de áudio, quando aplicável, preservando fases, filas e eventos necessários ao determinismo.

O pipeline compartilhado deverá acomodar, conforme os marcos de hardware, PSG SN76489, YM2413, YM2612/YM3438, DAC, RF5C164, CD-DA e PWM 32X, sem pressupor que todos estejam implementados ao mesmo tempo.

## 3. Responsabilidades permitidas de Python

Python é permitido exclusivamente nas camadas de frontend e ferramentas. Inclui:

- Interface gráfica Qt/PySide6: janela, menus, diálogos, controles e apresentação de configurações.
- Seleção/carregamento de ROMs e mídias e configuração de perfis.
- Comandos coarse-grained de ciclo de vida e configuração enviados ao runtime Rust.
- Consulta de snapshots, métricas, diagnósticos e mensagens de erro produzidos pelo runtime.
- Testes automatizados, fixtures, geração de casos de teste, auditorias, benchmarks e automação de desenvolvimento.
- Scripts de build, empacotamento e integração PyO3/Maturin.

Testes Python devem invocar ou validar o componente Rust real, ou verificar contratos de fronteira. Um modelo Python não poderá substituir o dispositivo Rust em testes de integração nem ser usado como oráculo permanente de comportamento do hardware. Oráculos independentes devem ter procedência e limitações documentadas.

## 4. Proibições explícitas

É proibido:

1. Implementar chips de áudio ou lógica de registradores em Python.
2. Executar síntese PSG/FM/PCM/PWM, reprodução CD-DA ou temporização de hardware em Python.
3. Fazer mixagem, DSP, filtragem, resampling, conversão ou empacotamento PCM em Python.
4. Transportar blocos de amostras ou buffers PCM através de PyO3 para processamento no frontend.
5. Fazer polling, callbacks ou chamadas Python por instrução, acesso de barramento, evento de chip ou amostra.
6. Usar Python como fallback quando um chip ou backend Rust ainda não estiver pronto.
7. Manter implementações Python e Rust concorrentes do mesmo dispositivo no caminho ativo.
8. Permitir que Qt, QTimer, WASAPI, XAudio2 ou o relógio do host avancem ou corrijam o tempo lógico dos chips.
9. Criar um segundo mixer, uma segunda sessão de áudio ou um runtime paralelo para contornar contratos existentes sem decisão arquitetural documentada.

**Quando um chip ainda não estiver implementado, sua implementação preliminar deverá ser Rust.** Ela poderá oferecer apenas estado mínimo, registradores documentados, sincronização temporal e saída silenciosa/controlada. Deve declarar suas limitações e não simular fidelidade que não possui. A ausência de síntese não autoriza fallback Python.

## 5. Fronteira Rust/Python e APIs de saída

PyO3/Maturin constitui uma fronteira fina de integração. Python pode enviar comandos coarse-grained, configurações e receber snapshots/telemetria. A fronteira não deve transportar amostras PCM nem executar callbacks Python no caminho crítico.

WASAPI e XAudio2 são consumidores de PCM produzido pelo runtime Rust. Seus workers e callbacks de host não executam lógica de chip e não são fontes de tempo emulado. A seleção, inicialização, fallback, métricas, recuperação e encerramento dos backends permanecem sob ownership Rust, com resultados observáveis pela UI.

O scheduler da máquina é a autoridade temporal. A taxa do dispositivo de saída pode exigir resampling ou compensação de deriva na fronteira de consumo, mas nunca pode alterar clocks, fases ou eventos digitais do hardware emulado.

## 6. Estratégia incremental obrigatória para os chips

A implementação deve ocorrer por etapas, sem exigir que todos os chips sejam concluídos simultaneamente:

1. **Preservar e integrar o PSG existente:** SG-1000 e SMS são a referência funcional atual. Migrar para contratos compartilhados sem reescrever sua síntese validada. Fixar baseline de amostras, estado, determinismo e desempenho antes de alterações.
2. **Consolidar contratos e pipeline compartilhado:** interfaces de dispositivo, timeline, fontes mono/estéreo, roteamento, mixer, DSP, resampling, buffers e saída nativa em Rust.
3. **Adicionar stubs Rust para os demais dispositivos:** YM2413, YM2612/YM3438, RF5C164, CD-DA e PWM. Inicialização, registradores e status mínimos documentados, avanço temporal, silêncio controlado, ciclo de vida e testes de não bloqueio.
4. **Implementar síntese por dispositivo:** um chip por vez, com documentação técnica, revisão/variante correta, vetores fixos, oráculos independentes, testes temporais e regressões.
5. **Expandir validação:** testes headless, integração com máquinas, testes longos, backends Windows físicos, métricas e benchmarks comparáveis.

A ordem incremental não autoriza duplicar infraestrutura. Reutilizar e evoluir `sega-audio-runtime`, `sega-audio-output` e o Temporal Core quando seus contratos forem adequados. Mudanças incompatíveis exigem baseline, justificativa e migração explícita.

## 7. Critérios de aceitação para qualquer contribuição de áudio

Uma contribuição somente pode ser aceita se:

- Toda lógica executável de hardware e processamento de áudio alterada estiver em Rust.
- Python estiver limitado a frontend, configuração, integração coarse-grained, testes e ferramentas.
- Não houver runtime, fallback ou modelo de hardware Python no caminho ativo.
- Os chips consumirem a timeline Rust, sem relógios independentes.
- Nenhum PCM/amostra crua for processado ou transportado pelo frontend/PyO3.
- Filas e workers forem limitados, com ownership, shutdown e tratamento de falhas explícitos.
- Testes Rust cobrirem os contratos e testes Python, quando usados, exercitarem o runtime Rust real.
- O PSG SG-1000/SMS mantiver suas regressões e baseline de desempenho.
- Stubs forem identificados como preliminares e silenciosos/controlados, sem alegações de fidelidade.
- Estados `IMPLEMENTED`, `QUICK_PASS`, `SHORT_PASS`, `LONG_PASS`, `INTEGRATION_PENDING` e `TEMPORAL_PASS` forem registrados separadamente, com evidências no SHA candidato.

Compilação ou existência de uma interface não prova fidelidade do chip. T-states agregados ou amostras produzidas por frame não provam precisão ciclo a ciclo.

## 8. Aplicação e precedência

Esta ordem é obrigatória para SG-1000, Master System, Game Gear, Mega Drive/Genesis, Sega CD, 32X e sistemas compostos futuros, incluindo Sega CD + 32X.

Toda nova issue, PR, ADR, plano de migração ou tarefa de agente que envolva áudio deve referenciar e obedecer a este documento. Revisores devem bloquear contribuições que introduzam runtime Python, processamento PCM Python ou lógica de chip fora de Rust.

Esta ordem não declara novos chips implementados, não altera o runtime existente e não autoriza por si só mudanças de código. Cada implementação continua sujeita ao inventário, critérios de aceitação e validação no SHA candidato exato.
