# Histórico de versões

## [0.2.0-dev.1] - 2026-10-30 — pré-release experimental

> **Não é release estável.** Esta versão serve para validar o novo fluxo de versionamento e registrar o estado integrado do SMS. Não habilita mappers especiais ainda não implementados.

### Incluído no baseline

- BIOS SMS opcional de 8 KiB mapeada integralmente em `$0000-$1FFF`, com controle pelo latch de memória.
- Identificação de firmware SMS combinando cabeçalho `TMR SEGA` e identidade exata via DAT, com recusa conservadora de conflitos, ambiguidades e hashes fracos para seleção de BIOS.
- DATs No-Intro instalados junto ao pacote e resolvidos tanto no checkout quanto na distribuição.
- Resolução regional determinística e perfil Brazil/PAL-M, mantendo território de BIOS separado do padrão de vídeo.
- Integração das configurações SMS entre Rust, PyO3, Python e Qt.

### Versionamento

- Introduz `VERSION` como versão canônica do projeto.
- Sincroniza metadados Python/PEP 440, extensão Maturin e crates Rust/Cargo.lock.
- Adiciona verificador automatizado de consistência de versões e política de rollback.

### Limitações explicitamente mantidas

- Mapper Sega padrão: SRAM de cartucho e persistência de saves ainda não implementadas.
- Mapper Codemasters e famílias coreanas/MSX/Nemesis/Janggun ainda não implementadas.
- A investigação do mapper de 128 Hap permanece inconclusiva; não há correção específica declarada.
- Não declarar boot/gameplay geral, LONG_PASS ou TEMPORAL_PASS sem evidência independente no emulador.

### Validação desta pré-release

- Baseline: `41c32007ae961b78b3d8fa6c604af9f01159ffad`.
- Checks desta alteração: aguardando execução no SHA candidato da PR.
- Gameplay manual no Windows: não executado nesta alteração.

---

## Estado SMS — 2026-10-29
- Publica fotografia técnica do SMS na base `b4a85c82b1d975a126f1dcbd3084dac51249b398`, após integração do runtime nativo Rust à aplicação Qt/PySide6.
- Registra separadamente os subsistemas implementados e as lacunas de Z80, VDP raster, scheduler temporal, FM/YM2413 e validação Windows.
- Não transfere resultados de testes/workflows de SHAs anteriores para a nova base; validações desta documentação permanecem NOT RUN.

# Histórico de versões

## Limpeza arquitetural — extensão PyO3
- Remove o executor Z80 independente `RustZ80Cpu` e o adaptador `NativeMemoryBus` não utilizados pela máquina integrada.
- Mantém `native/z80_core` como fronteira PyO3/Maturin para `NativeSG1000Machine`, cujo hardware ativo permanece em `native/sg1000_core`.
- Remove a dependência direta desnecessária de `sega-z80-alu` da extensão; a ALU standalone permanece como oráculo diferencial separado.
- Acrescenta teste de integração para impedir que os símbolos removidos voltem a ser exportados.
- Validação de build e testes fica condicionada aos workflows desta PR; nenhum resultado é presumido.

## v0.1.0 — Primeiro release SG-1000 (preparação)

### Marco de áudio e vídeo
- Registra a avaliação manual de 2026-10-26: áudio e vídeo do SG-1000 alcançaram qualidade percebida satisfatória para o primeiro marco; o áudio foi considerado muito próximo ao Gearsystem.
- Documenta explicitamente que essa avaliação é observacional e não certifica equivalência com hardware real, compatibilidade universal ou temporização ciclo a ciclo.
- Mantém como pendência a investigação de pequenos engasgos e possível descompasso durante sessões prolongadas.

### Interface Qt
- Adiciona modo imersivo de tela cheia dedicado à imagem do jogo, ocultando menus, barra de ações, status e margens.
- F11 e Alt+Enter alternam a tela cheia; Esc retorna à janela normal.
- Adiciona teste automatizado para ocultar e restaurar os elementos da interface.

### Preparação do primeiro release
- Atualiza README e cria registro do marco SG-1000, incluindo evidências, limites conhecidos e plano de validação de estabilidade.
- Adota licença MIT, declara os metadados de licença no pacote e atualiza a caixa Sobre com versão, escopo, créditos e aviso de independência.
- A publicação continua condicionada à conclusão dos checks no SHA candidato e à revisão final; não se presume que testes locais ou CI tenham passado.


## Próxima versão — em desenvolvimento

### Diagnóstico de áudio / PSG
- Adiciona gravação de comandos do SN76489 em arquivo VGM 1.71 pelo menu **Diagnóstico**.
- Registra valores escritos no PSG com timestamps do clock da máquina, no runtime Rust; a implementação Python anterior está arquivada e não é executada.
- A gravação é destinada à análise dos comandos de áudio e à comparação com capturas do Gearsystem; não altera a síntese nem os filtros de áudio.
- Validação automatizada adicionada para cabeçalho, temporização, intervalos longos, escrita de arquivo e ações do menu Qt.

**Limitações:** ainda requer execução dos testes no CI e validação manual no Windows. A gravação VGM registra writes de PSG, não PCM renderizado nem temporização sub-T-state que o modelo atual não exponha.
