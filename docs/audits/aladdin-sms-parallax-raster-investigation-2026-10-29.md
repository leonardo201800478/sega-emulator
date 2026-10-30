# Aladdin (Master System) — investigação de parallax e sincronização raster

Data: 2026-10-29  
Estado: **INVESTIGATION_PENDING**  
Runtime alterado: **não**  
ROM no repositório: **não**

## Relato do problema

O usuário relata que **Aladdin para Sega Master System**, na primeira fase, apresenta defeitos gráficos acentuados durante o scroll horizontal automático. As camadas visuais do fundo (descritas no relato como “sprites de fundo”) parecem perder sincronização horizontal, produzindo desalinhamento/parallax incorreto.

Este é um relato do usuário; ainda não foi reproduzido independentemente nesta auditoria. Não se conclui ainda se a falha está no VDP, no agendamento da CPU, na apresentação de frames ou na interpretação do efeito usado pelo jogo.

## Contexto técnico conhecido no código

A implementação SMS atual declara explicitamente um modelo determinístico por scanline, não exato por ponto/ciclo, em `native/sms_core/src/vdp.rs`. A linha é renderizada ao completar a scanline e usa o estado de registradores/CRAM disponível nesse momento. Esse modelo pode ser insuficiente se o jogo depender de escritas durante a varredura horizontal para simular múltiplas velocidades de scroll ou separação entre camadas.

A limitação é uma hipótese plausível, não uma causa confirmada. O efeito visual chamado “parallax” pode envolver alterações de scroll, mudanças de tilemap/VRAM, paleta, sprites hardware ou combinação dessas técnicas. No SMS, devemos identificar o mecanismo concreto em vez de assumir múltiplos planos de fundo independentes.

Investigação relacionada: PR #261 — https://github.com/leonardo201800478/sega-emulator/pull/261. Esta PR é um caso de reprodução específico para Aladdin; não substitui nem duplica a investigação geral de temporização do VDP.

## Hipóteses concorrentes

1. **Granularidade raster insuficiente:** escritas nos registradores de scroll/VDP durante a linha só afetam a renderização inteira da linha, quando o jogo espera um efeito intra-scanline.
2. **Ordem temporal CPU–VDP:** a escrita do jogo e o avanço do VDP são processados numa ordem ou fronteira temporal incorreta.
3. **Semântica de latch/scroll:** o momento em que valores horizontais/verticais passam a valer difere do hardware SMS.
4. **Dados de fundo e sprites:** endereçamento de tilemap, padrões, atributos ou prioridade podem ser interpretados incorretamente e produzir um defeito visual parecido com dessincronização.
5. **Cadência de frame/apresentação:** ritmo de apresentação, descarte ou repetição de frames pode exagerar o problema, embora não explique por si só desalinhamentos espaciais dentro de uma mesma imagem.

## Plano de investigação

1. Registrar identificação local da ROM (tamanho e SHA-256) apenas em ambiente local, sem incluir a ROM nem dados que permitam reconstruí-la no Git.
2. Reproduzir a primeira fase desde o mesmo ponto e capturar a mesma sequência no Sega Emulator e em um emulador de referência, preferencialmente Ares; registrar versão, região e padrão de vídeo.
3. Distinguir erro dentro de um frame (pixels/linhas deslocados) de erro entre frames (cadência/judder).
4. Instrumentar, apenas em execução de diagnóstico, escritas relevantes nos registradores do VDP e eventos de avanço raster: frame, scanline, T-state na linha, porta/registrador e valor. Evitar logs contínuos no caminho crítico fora da reprodução controlada.
5. Determinar se a divergência aparece após uma escrita intra-scanline, na mudança de linha, na leitura de VRAM/sprites ou somente na apresentação.
6. Criar uma regressão sintética determinística que represente a semântica isolada encontrada (por exemplo, mudança de scroll/registrador em fase horizontal conhecida), sem usar a ROM comercial como teste automatizado.
7. Implementar a menor alteração temporal compatível com a evidência; manter a renderização scanline para casos que não exigem granularidade maior se isso preservar os contratos existentes.

## Critérios de aceitação

- [ ] O sintoma é reproduzido e documentado com configuração de emulação comparável.
- [ ] A divergência é classificada como intra-frame/raster, dados gráficos, temporização CPU–VDP ou apresentação.
- [ ] Há um teste sintético determinístico que falha antes e passa depois da correção.
- [ ] A correção não regressa scroll fino, scroll vertical, sprites, colisão/overflow, CRAM, IRQ, PAL/NTSC nem o SG-1000.
- [ ] Testes relevantes de Rust e integração são executados no SHA candidato.
- [ ] A reprodução manual de Aladdin no Windows é verificada, sem adicionar ROM comercial ao repositório.

## Validação e pendências

- Inspeção estática do cabeçalho de `native/sms_core/src/vdp.rs`: **QUICK_PASS** apenas para confirmar o modelo scanline declarado; não valida gameplay.
- Reprodução independente: **NOT RUN**.
- Teste sintético específico para Aladdin/parallax: **NOT IMPLEMENTED**.
- Testes Rust/Python e build Release/Maturin nesta fatia documental: **NOT RUN**.
- CI: consultar checks no SHA desta PR; ausência de checks não significa aprovação.
- `TEMPORAL_PASS`: **NOT ACHIEVED**.
- ROM comercial: não incluída.

## Próxima etapa

Obter capturas comparáveis e localizar as escritas de scroll/VDP associadas ao efeito. Só então decidir se a correção exige rasterização parcial por posição horizontal, sincronização CPU–VDP mais fina ou uma correção funcional de registrador/mapeamento.
