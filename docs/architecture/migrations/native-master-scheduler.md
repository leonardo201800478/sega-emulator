# Plano de migração — Master Scheduler Rust

**Tipo desta etapa:** arquitetura e critérios de aceitação.  
**Runtime alterado:** não.  
**Estado:** DESIGN / BLOCKED ON VIDEO AND INPUT PORTS.

## Dependências

- [ ] Integrar e validar a PR #301 (vídeo, saída e apresentação Rust-owned).
- [ ] Integrar e validar a PR #300 (captura, normalização e despacho de input nativos).
- [x] Reutilizar o runtime de áudio Rust já presente na main; não duplicar sessão, mixer, fila ou sink.
- [ ] Revalidar os contratos e SHAs das dependências imediatamente antes de iniciar implementação.

Esta proposta não autoriza merge das PRs #300/#301 nem presume que estejam concluídas.

## Fases propostas

### Fase A — inventário e contrato
- [x] Registrar dependências, responsabilidades e critérios de aceitação.
- [ ] Auditar o fluxo real da main após integração de vídeo e input.
- [ ] Mapear todos os chamadores de `run_frame`, timers Qt, pontos de captura de input e consumidores de áudio/vídeo.
- [ ] Definir domínio temporal, resolução, overflow, política de eventos simultâneos e safe points por plataforma.
- [ ] Fixar testes sintéticos antes de alterar execução.

### Fase B — relógio virtual e scheduler headless
- [ ] Introduzir clock/timeline inteiro ou racional sem depender de wall-clock.
- [ ] Implementar fila determinística de eventos e política explícita de desempate.
- [ ] Manter um único proprietário do estado da máquina.
- [ ] Integrar primeiro SG-1000/SMS sem mudar a semântica dos dispositivos além do necessário.
- [ ] Adicionar replay de input normalizado e checkpoints determinísticos.

### Fase C — integração temporal dos dispositivos
- [ ] Integrar CPU/barramento e eventos temporais sem confundir T-states agregados com bus trace.
- [ ] Sincronizar VDP/raster/IRQ com a timeline e aplicar efeitos de registradores nos pontos corretos.
- [ ] Avançar PSG e demais fontes de áudio até os mesmos instantes lógicos.
- [ ] Formalizar eventos simultâneos e safe points para input, reset, pause e troca de ROM.
- [ ] Refinar waits, contenção e micro-operações apenas com evidência e testes direcionados.

### Fase D — saída e pacing independentes da UI
- [ ] Remover QTimer como autoridade de avanço da máquina.
- [ ] Deixar captura de input publicar eventos; scheduler os consome em instantes definidos.
- [ ] Deixar áudio consumir a fila no relógio do dispositivo sem avançar hardware.
- [ ] Deixar vídeo apresentar frames produzidos pelo runtime sem regular a timeline lógica.
- [ ] Definir política de backpressure, frames descartados, áudio atrasado e perda de dispositivo sem alterar determinismo.

### Fase E — validação integrada
- [ ] Testes determinísticos headless e replay.
- [ ] Regressão SG-1000 e SMS com ROMs sintéticas/homebrew legalmente obtidas.
- [ ] Integração Windows com dispositivos reais de input, WASAPI/XAudio2 e backend gráfico efetivo.
- [ ] Testes longos, hotplug, pause/reset/load/stop, foco e device loss.
- [ ] Medir deriva A/V, latência ponta a ponta, underruns, fila e frame pacing.
- [ ] Publicar matriz de estados e evidências no SHA candidato exato.

## Regra de avanço

Não iniciar a Fase B enquanto os ports de vídeo e input não estiverem integrados e seus contratos de saída/captura estiverem estáveis. Não declarar sincronização total até que a Fase E apresente evidências em testes determinísticos e hardware Windows. A precisão ciclo a ciclo permanece um objetivo progressivo, não um pressuposto desta migração.


## Revisão de dependências — 2026-10-31

A ordem anterior que bloqueava o desenvolvimento da Fase B até integrar vídeo (#301) e input (#300) foi relaxada para as etapas headless. T0 (tempo/domínios/fila) e T1 (scheduler virtual/headless) podem ser implementadas e testadas com dispositivos sintéticos e replay determinístico, sem Qt, SDL3, renderer ou dispositivo de áudio.

#300/#301 continuam gates para integração final de input e vídeo do host. Antes de integrar a timeline às máquinas, comparar os contratos e preservar regressões SG-1000/SMS. A PR #315 é uma proposta de T0, não prova de MachineScheduler nem de integração temporal. Ver [plano de reorganização Z80/runtime](shared-z80-runtime-reorganization.md).
