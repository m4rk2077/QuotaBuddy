# Story 25 — Reset Run

## Objetivo

Transformar o estado de limite Codex esgotado em uma espera útil e memorável: o usuário pode jogar um runner local enquanto acompanha o horário real do reset.

## Escopo

- Detectar limite de sessão (5h) ou semanal/ciclo com `100%` usado.
- Exibir um estado de cooldown no overview com o limite atingido e o countdown real.
- Oferecer o minigame opcional `Reset Run`; nunca abrir o jogo automaticamente.
- Usar um mascote original do QuotaBuddy, o Buddy, com linguagem visual de terminal arcade.
- Rodar totalmente local, sem rede, telemetria, conta ou dependência nova.
- Preservar histórico, configurações, refresh, alertas, tray, gasto e diagnósticos.

## Regras de ativação

- Considerar somente snapshot `codex` saudável, não stale e com uso disponível.
- Considerar atingida somente métrica `session`, `weekly` ou `cycle` com `usedPercentage >= 100`.
- Exigir `resetsAt` válido e no futuro para cada métrica usada pelo gatilho.
- Se sessão e semanal estiverem esgotados, usar o reset mais tardio como desbloqueio efetivo.
- Nunca abrir o cooldown com dado indisponível, expirado, falho ou sem reset confiável.
- Quando o refresh indicar que o limite voltou, fechar o jogo e retornar ao overview.

## Experiência

- Estado inicial: “Codex em cooldown”, countdown e botão `Jogar enquanto espera`.
- Jogo: runner lateral; `Espaço`, seta para cima, clique ou toque fazem Buddy pular.
- Obstáculos usam símbolos abstratos de tokens/contexto; sem logo oficial da OpenAI.
- Objetivo da rodada: alcançar o portal `RESET` em aproximadamente 60–90 segundos.
- Vitória concede pontuação e recorde local; não promete alterar a quota real.
- Countdown real permanece visível durante toda a partida.
- `Escape` ou botão Voltar retorna ao monitor sem encerrar o app.

## Critérios de aceite

1. Limite de sessão esgotado com reset futuro mostra o cooldown.
2. Limite semanal/ciclo esgotado com reset futuro mostra o cooldown.
3. Dois limites esgotados usam o reset efetivo mais tardio.
4. Snapshot stale, falho, indisponível ou sem reset futuro não ativa o jogo.
5. O usuário inicia, pula, colide, reinicia e alcança o checkpoint usando teclado ou ponteiro.
6. Vitória deixa claro que o reset da quota continua obedecendo ao horário real.
7. Recorde fica apenas no armazenamento local do WebView.
8. Game pausa quando a janela perde visibilidade e não mantém loop após desmontagem.
9. Tema claro/escuro, reduced motion, foco visível e rótulos acessíveis continuam válidos.
10. Testes frontend, build web e gates Rust passam.
11. QA visual no app real confirma o layout 400 × 560 sem overflow.

## Fora de escopo

- Alterar, burlar ou antecipar limites do Codex.
- Ranking online, login, sincronização ou telemetria.
- Itens pagos, skins remotas ou multiplayer.
- Uso de nome, logo ou mascote oficial da OpenAI como personagem.
- Integração do jogo com Claude, Cursor ou OpenCode.
