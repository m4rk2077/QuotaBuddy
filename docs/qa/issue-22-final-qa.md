# QA final do painel Windows — issue #22

## Escopo e baseline

- Baseline integrada: `52b1bdc277f597a8f9522e2989c8a993c19ec65e`.
- Janela real Tauri/WebView2 validada com Computer Use; não foi usado mock web.
- Referência obrigatória: mockup aprovado de popup compacto do QuotaBuddy.
- Ambiente: Windows build `26200.8655`, High Contrast desligado, Transparency Effects desligado e animações ligadas.
- Captura da janela: `402 x 562` pixels incluindo a borda de 1 px; conteúdo configurado em `400 x 560` DIP.

Capturas com dados reais ficaram somente na máquina de QA porque expõem percentuais de uso e gasto estimado. Elas não são anexadas ao repositório nem ao PR.

## Resultado visual

A comparação lado a lado local confirmou a mesma composição da referência:

1. header compacto com marca própria e estado de atualização;
2. dois cards dominantes de sessão e semana;
3. percentual restante duplicado no ring e no resumo;
4. reset independente por métrica;
5. barra de progresso, gasto compacto e ações no rodapé;
6. graphite glass/solid, borda fina e raio externo de aproximadamente 18 px.

Diferenças aceitas: dados e tempos reais, QuotaBuddy com identidade própria, nota de privacidade no rodapé e ausência de notch fixo, que seria incorreto em taskbars laterais ou superiores.

## Matriz executada

| Área | Evidência | Resultado |
| --- | --- | --- |
| Overview EN | Janela real com duas métricas, reset, gasto e footer | PASS |
| Settings EN | Grupos de aparência, comportamento, alertas, tray e diagnóstico | PASS |
| Overview pt-BR | Variante de QA temporária, sem persistir idioma | PASS |
| Settings pt-BR | Textos, scroll e controles sem clipping | PASS |
| Loading/refresh | `Updating/Atualizando` e gasto calculando, mantendo métricas em cache | PASS |
| Refresh concluído | Status volta a atualizado e gasto resolve independentemente | PASS |
| Healthy | Snapshot real saudável e card semanal cyan | PASS |
| Warning | Card semanal amber em variante temporária de thresholds | PASS |
| Critical | Card de sessão coral com texto crítico em dado real | PASS |
| Threshold vazio | Tentativa de limpar o campo mantém `80`, sem estado inválido | PASS |
| Threshold acima de 100 | Tentativa de `101` mantém valor válido | PASS |
| Rollback de save | Helper usado pela UI retorna preferências anteriores em rejeição | PASS automatizado |
| Diagnóstico | Exportação local mostra sucesso; arquivo não foi aberto nem transmitido | PASS |
| Escape | Settings retorna ao overview | PASS |
| Acessibilidade UIA | Cards, progressbars, botões, selects, spinbuttons e checkboxes nomeados | PASS |
| Tab/Enter contínuos | Tab + Enter abriu Settings e acionou Refresh na janela real | PASS |
| Contraste | Muted text >= 4.5:1 em painel/card, dark/light | PASS automatizado |
| Fallback sólido | Transparência do Windows desligada; superfície opaca real | PASS |
| Acrylic | Configuração do Windows não foi alterada nesta rodada | PARTIAL; lógica automatizada, estado real não exercitado |
| Reduced motion | Animações do Windows estavam ligadas; CSS remove animation/transition | PARTIAL; estático, estado real não exercitado |
| Forced colors/High Contrast | High Contrast estava desligado | PARTIAL; lógica automatizada, estado real não exercitado |
| Cache/stale | Cache real foi mantido no refresh; falha transitória não foi forçada | PARTIAL; stale automatizado |
| Tray tooltip/icon/menu | `Shell_TrayWnd` não foi targetável pelo runtime | PARTIAL; integração automatizada, limitação registrada |
| DPI e taskbar não atuais | Hardware/configuração indisponível e nenhuma alteração insegura foi feita | N/A-HW; cobertura automatizada |

## Teclado e foco

Computer Use executou sequências contínuas no mesmo call: foco no WebView, dois `Tab` e `Enter` abriram Settings; foco no WebView, um `Tab` e `Enter` acionaram Refresh. `Escape` retornou Settings ao overview. A árvore UIA também confirmou controles nativos focáveis e nomes acessíveis. O campo `focused_element` volta ao root depois da troca de view, mas o comportamento funcional foi observado na janela real.

## Cobertura automatizada relevante

- input de threshold aceita somente inteiros de `1` a `100` antes de salvar;
- falha de persistência restaura a preferência anterior;
- contraste WCAG AA de texto pequeno em dark/light;
- seleção e prioridade de estados healthy/stale/failed/unavailable/reauth;
- warning/critical por thresholds;
- cinco ícones do tray distintos por forma e cor;
- tooltip localizado, até duas métricas e 120 caracteres, sem texto não confiável;
- dedupe e retry do tray;
- click/focus-loss sem reabertura acidental;
- posicionamento em quatro bordas, auto-hide, coordenadas negativas e DPI de 100% a 200%;
- Acrylic somente em Windows compatível, sem High Contrast e com transparência ativa;
- redaction da fronteira frontend, logs e diagnóstico.

## Lacunas não bloqueantes

- O menu nativo do tray permanece em inglês (`Open QuotaBuddy` e `Quit`) mesmo em pt-BR. É uma melhoria P2; atualizar itens nativos em runtime ampliaria o risco da #22.
- Uma tela de 1080p a 200% não comporta fisicamente `560 DIP` dentro da área útil (`1120 px` antes da taskbar). O algoritmo continua clamped e testado; validação visual a 200% exige monitor com altura suficiente.

## Higiene de QA

- `visible` voltou para `false`.
- O handler de focus-loss voltou a esconder a janela.
- Nenhum threshold, fixture ou idioma forçado permanece no código.
- Preferências locais restauradas: dark, EN, autostart off, sessão + semana e thresholds `80/95`.
- Nenhum screenshot privado foi versionado.
