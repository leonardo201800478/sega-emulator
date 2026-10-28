# Política permanente de desenvolvimento, fidelidade e entrega

**Status:** obrigatória para todo trabalho futuro no Sega Emulator.  
**Escopo:** arquitetura, implementação, validação, pesquisa técnica, agentes, GUI, backends de host, CI e publicação.  
**Repositório oficial:** https://github.com/leonardo201800478/sega-emulator

Esta política complementa, e não substitui, os contratos específicos de cada componente. Em caso de contradição, registrar a divergência e alinhar a documentação antes de prosseguir. Não presumir que uma decisão documentada já foi implementada: verificar o HEAD e o código real.

## 1. Objetivo e marcos

Construir um emulador determinístico, modular, testável e de máxima fidelidade demonstrável no Windows. Avançar por marcos sem ampliar o escopo prematuramente:

SG-1000 → Master System/Game Gear → Mega Drive/Genesis → Sega CD → 32X.

O componente atual deve ser inventariado e suas lacunas reais fechadas antes de reescrever ou duplicar trabalho. Preservar comportamento validado, contratos e mudanças concorrentes. O marco standalone só é considerado funcional quando os componentes de hardware exigidos estão integrados e validados; compilar, abrir a GUI ou exibir uma tela não basta.

## 2. Estratégia permanente de implementação por componente

Aplicar esta sequência a cada CPU, barramento, memória, VDP, PSG, I/O, scheduler ou outro componente:

1. **Inventário:** ler o código atual, contratos, testes, documentação, histórico, dependências e evidência no GitHub. Classificar cada capacidade como ausente, implementada, testada ou validada; não confundir essas categorias.
2. **Implementação das lacunas:** completar a modelagem funcional e os comportamentos sustentados por especificações e evidências. Fazer fatias pequenas, compiláveis, limpas e revisáveis.
3. **Testes rápidos:** após cada fatia, executar formatação, lint, build/checks relevantes e testes unitários/regressões essenciais. Estes testes detectam falhas rápidas, mas não autorizam por si só a liberação do componente.
4. **Validação abrangente curta:** testar famílias de instruções/casos, limites, wraparound, flags, erros, memória/I/O, interrupções, repetibilidade e comparação diferencial independente conforme o componente. Corrigir divergências e repetir os testes afetados.
5. **Validação longa:** executar suites completas, testes de estresse, determinismo, integração e benchmarks quando pertinentes. Registrar PASS, FAIL, BLOCKED e NOT RUN.
6. **Liberação e próximo componente:** avançar quando os critérios funcionais do componente forem satisfeitos, as regressões relevantes estiverem verdes e os riscos/limitações forem registrados.
7. **Validação temporal integrada:** quando a máquina SMS estiver completa e funcional, executar a campanha abrangente de sincronização e precisão temporal entre CPU, memória/barramento, I/O, VDP, PSG e scheduler. Corrigir divergências e repetir a campanha relevante.

Não adiar testes de regressão baratos ou testes de referência que possam detectar erros funcionais conhecidos. O que fica para a integração final é a campanha **completa** de temporização ciclo a ciclo, não a arquitetura que a tornará possível.

### Contrato temporal desde o início

Componentes que futuramente exigirão precisão temporal devem preservar os dados e contratos necessários (T-states, tipos de acesso, ordem de eventos, estados de espera e sinais relevantes). Não declarar ciclo a ciclo com base apenas em contagem agregada por instrução. Introduzir interfaces temporais incrementalmente, quando houver contrato e critérios claros, sem criar abstrações especulativas para hardware futuro.

## 3. Linguagens e arquitetura

- **Rust nativo é obrigatório para todo o runtime de emulação:** CPUs, ALU, memória, barramento, mapper, I/O, VDP, PSG, temporização, scheduler e estado da máquina. Não introduzir um segundo runtime ativo de hardware em Python.
- **Python** é para Qt/PySide6, GUI, configuração, seleção/carregamento de ROMs e perfis, logs de aplicação, ferramentas, automação, pytest, testes diferenciais e benchmarks.
- **PyO3/Maturin** é uma fronteira fina entre UI/ferramentas e Rust. Evitar callbacks Python por instrução/byte, cópias de grandes buffers no caminho crítico e liberação do GIL/threads sem modelo explícito de ownership e sincronização.
- Código limpo: módulos coesos, responsabilidades explícitas, tipos e erros claros, APIs mínimas, nomes consistentes, comentários sobre decisões não óbvias, sem duplicação evitável. Não refatorar por estética sem benefício verificável.
- Não adotar JIT, paralelismo artificial, otimizações especulativas ou alocação no caminho crítico sem profiling, baseline reproduzível e testes equivalentes.
- Mudanças de linguagem e mudanças de fidelidade devem ser separadas em PRs diferentes, salvo justificativa técnica registrada.

## 4. Fidelidade e pesquisa técnica

- Tratar a emulação como reprodução de hardware. Distinguir explicitamente comportamento **documentado**, **observado**, **inferido** e **hipótese**.
- Consultar a documentação técnica atual na web quando a tarefa depender de APIs, versões, protocolos, hardware, comportamento pouco documentado ou bibliotecas. Priorizar manuais do fabricante, especificações oficiais, notas de versão e documentação primária. Registrar URLs, revisão/versão e o que a fonte realmente sustenta.
- Consultar código aberto de emuladores de terceiros **somente para estudar lógica, arquitetura, casos de borda e estratégias de teste**. Não copiar nem adaptar trechos, estruturas substancialmente expressivas, testes ou assets sem revisão explícita de licença, procedência, atribuição e compatibilidade. Preferir implementação independente, baseada em especificação e oráculos independentes. Registrar quando duas referências compartilham ancestralidade de código e portanto não são oráculos independentes.
- Gearsystem é referência de estudo para SG-1000/SMS/Game Gear, mas declara GPL-3.0: não derivar código sem análise de licença. MAME, ares, MEKA, Genesis Plus GX e outros também devem ser usados respeitando suas licenças e proveniência; referência não é oráculo infalível.
- Não inventar clocks, ciclos, flags, estados de registradores, comportamento elétrico ou efeitos analógicos. Não afirmar precisão ciclo a ciclo sem modelo temporal e testes de eventos ordenados.
- Usar ROMs sintéticas/homebrew, vetores fixos e materiais obtidos legalmente. Nunca versionar ROMs comerciais, BIOS protegidas ou artefatos que revelem seu conteúdo.

## 5. Limite de agentes de IA

Usar **no máximo três agentes de IA simultâneos** para qualquer tarefa do projeto. Distribuir papéis independentes e sem sobreposição desnecessária, por exemplo:

1. pesquisa de fontes/especificações e análise de referências;
2. auditoria de código, riscos e proposta de implementação;
3. revisão independente de testes, CI e regressões.

O coordenador integra resultados, resolve conflitos com evidências e evita que agentes modifiquem simultaneamente os mesmos arquivos. Preferir validadores somente leitura. GitHub Actions são validadores automatizados, **não** agentes de IA, salvo quando houver de fato um agente de IA executando uma tarefa. Não alegar revisão autônoma se apenas foram executados comandos ou workflows.

## 6. Testes e GitHub Actions

GitHub Actions é o validador reproduzível central. Antes de editar, confirmar repositório, branch, HEAD/SHA, PRs e estado de CI. Definir critérios de aceitação antes da implementação.

Comandos-base, conforme o escopo e o ambiente:
- Rust: `cargo fmt --all -- --check`
- Rust: `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Rust: `cargo test --workspace --locked`
- Python: `python -m pytest`
- Python: `python -m ruff check .`
- Python: `python -m ruff format --check .`
- Quando pertinente: build Release/Maturin, testes de integração da extensão, `compileall`, `pip check`, determinismo e benchmarks.

Verificar os filtros `paths` dos workflows; um job ignorado ou não disparado não é PASS. Inspecionar logs, artefatos e resultados no SHA exato do candidato. Não usar sucesso de upload de artefato como prova de aprovação. Se lint/format impedir testes, marcar os testes bloqueados como NOT RUN. Após correção, repetir os checks afetados e regressões.

Para cada mudança comportamental, criar regressão que falhe antes da correção quando viável. Preferir oráculos independentes e expectativas fixas, não valores calculados pelo próprio código testado. Hashes iguais não provam correção absoluta: investigar a primeira divergência.

Benchmarks devem usar baseline e candidato comparáveis, mesmo workload/configuração/build, aquecimento e repetições; relatar mediana, variação e limitações.

## 7. Publicação, PR e testes in-game

- Não editar nem publicar diretamente em `main` para mudanças de código/documentação não triviais. Criar branch, preservar o SHA-base, abrir PR e revisar o diff.
- Para mudanças que afetam execução de jogos, CPU, timing, vídeo, áudio, input, integração ou regressões in-game, o merge para `main` exige: checks aplicáveis aprovados no SHA candidato **e evidência de teste in-game** com jogo/ROM legal, build/configuração, passos e resultado. Se o teste depender da máquina do mantenedor e não puder ser executado pelo agente, marcar BLOCKED/PENDING e solicitar o resultado; não inventar aprovação.
- Mudanças puramente documentais podem dispensar teste in-game, mas ainda exigem revisão de links, consistência e checks pertinentes.
- Não fazer merge automaticamente por conveniência. Só realizar merge quando solicitado/autorizado e quando os gates aplicáveis estiverem satisfeitos. Não alterar proteções, permissões, branch padrão ou releases sem autorização.
- Nunca usar force-push, reset destrutivo ou sobrescrever trabalho alheio sem consentimento explícito e avaliação de impacto.

Relatar sempre: objetivo, arquivos, branch, PR, SHA, checks e evidências de teste in-game, limitações, itens NOT RUN/BLOCKED e próximo passo.

## 8. APIs de baixo nível, input lag e apresentação

Avaliar APIs nativas por necessidade mensurável e contratos existentes; não adotar todas por padrão.

- **Vídeo/apresentação:** Vulkan, Direct3D 12, Direct3D 11 e backends do Qt/QRhi/Qt Quick conforme suporte real, compatibilidade, latência, custo de cópia e medição. Não presumir que selecionar uma API comprova que ela está ativa; registrar backend efetivo, versão, fallback e erros.
- **Áudio:** WASAPI (compartilhado/exclusivo quando suportado e apropriado), PortAudio/sounddevice e outras camadas existentes. Medir latência, tamanho de buffer, underruns/overruns, estabilidade e sincronismo A/V antes de mudar a arquitetura.
- **Input/controles:** XInput, SDL3 Gamepad/Joystick e APIs nativas adequadas. Normalizar snapshots na fronteira de host e passá-los à máquina; não chamar APIs de host por instrução. Medir latência de ponta a ponta, hot-plug e comportamento de desconexão.
- Usar Rust para caminhos nativos de host críticos quando houver benefício demonstrado e contrato claro; manter a GUI Qt/PySide6 em Python e as bindings finas. A escolha de API deve considerar compatibilidade, suporte, manutenção e medição, não apenas proximidade ao hardware.
- Não criar múltiplos backends redundantes nem bypassar Qt/SDL sem provar que a camada atual impede um requisito. Documentar ownership, sincronização, buffers e thread model.

## 9. GUI, diagnóstico e logs

- Evoluir a GUI com layout moderno, consistente, acessível e responsivo, preservando atalhos, configurações e comportamento existente. Mudanças visuais devem incluir validação em resolução/escala adequadas e estados de erro/fallback.
- Implantar logs e ferramentas de diagnóstico essenciais: sessão, versão/build, SHA da ROM quando aplicável, backend efetivo, configuração relevante, erros, checkpoints e métricas de desempenho/sincronismo. Evitar log excessivo por instrução no caminho normal; usar tracing/instrumentação seletiva ativável.
- Logs e snapshots podem conter dados de ROM; mantê-los locais, excluir do Git e redigir dados sensíveis. Instrumentação deve ser determinística quando fizer parte de testes e não alterar indevidamente a temporização observada.
- Distinguir métricas de aplicação (FPS enviado à UI, tempo de frame) da apresentação física do monitor, underruns reais e latência end-to-end; não alegar medições que o backend não expõe.

## 10. Estados de conclusão de componente

Registrar separadamente:
- `IMPLEMENTED`: escopo funcional implementado;
- `QUICK_PASS`: checks rápidos aprovados;
- `SHORT_PASS`: validação funcional curta aprovada;
- `LONG_PASS`: campanha longa aprovada;
- `INTEGRATION_PENDING`: aguardando validação integrada/temporal;
- `TEMPORAL_PASS`: critérios temporais declarados aprovados.

Esses estados são evidências independentes, não badges decorativos. Um componente só avança quando os critérios definidos para sua fase forem satisfeitos. A aprovação funcional não implica aprovação temporal ou compatibilidade universal.

## 11. Referências oficiais iniciais

Consultar versões atuais e registrar as páginas específicas usadas em cada mudança:
- Zilog Z80 CPU User Manual: https://www.zilog.com/docs/z80/um0080.pdf
- Rust Reference: https://doc.rust-lang.org/reference/
- Cargo Book: https://doc.rust-lang.org/cargo/
- PyO3 guide: https://pyo3.rs/
- Maturin user guide: https://www.maturin.rs/
- Qt for Python: https://doc.qt.io/qtforpython-6/
- Qt graphics/RHI: https://doc.qt.io/qtforpython-6/overviews/qtdoc-topics-graphics.html
- Vulkan specification/docs: https://docs.vulkan.org/
- Microsoft Learn (Direct3D, WASAPI, XInput): https://learn.microsoft.com/windows/
- SDL3 documentation: https://wiki.libsdl.org/SDL3/

A lista é ponto de partida, não substitui consulta à documentação específica da versão/API usada.
