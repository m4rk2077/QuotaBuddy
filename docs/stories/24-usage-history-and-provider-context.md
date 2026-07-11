# Story 24 — Histórico de uso e contexto de provedores

## Objetivo

Dar ao usuário uma leitura simples e privada de como os modelos Codex foram usados neste PC, quanto esse uso equivaleria nos preços públicos da API e como isso se relaciona com a quota compartilhada da assinatura.

## Escopo

- Eliminar janelas de console durante atualização e detecção.
- Preservar o popup atual de 400 × 560 e adicionar uma tela rolável de Histórico.
- Mostrar períodos de 7 dias, 30 dias e todo o histórico local disponível.
- Agregar tokens por modelo, participação percentual, cached input e equivalente API estimado.
- Mostrar atividade agregada da conta retornada por `account/usage/read` e o perfil mínimo (`authMode`/`planType`) sem expor email ou identificadores.
- Explicar que a quota da assinatura pode ser compartilhada por Codex app, CLI, IDE, Web e outros clientes autenticados na mesma conta/workspace.
- Detectar apenas metadados seguros do Hermes; nunca atribuir a ele uma quantidade de tokens que a fonte não informa.
- Não raspar Cursor, não alterar configuração do Claude e não ler tokens OAuth.

## Regras de dados

- O frontend recebe apenas contadores, datas, modelo, percentuais, status e rótulos normalizados.
- O parser local aceita somente `turn_context.payload.model` e `event_msg` do tipo `token_count`.
- `last_token_usage` é o delta principal; `total_token_usage` só pode ser usado como fallback monotônico.
- Reasoning tokens já pertencem ao output e não são cobrados novamente.
- Modelo sem preço continua visível e reduz a cobertura da estimativa; nunca recebe preço inventado.
- A estimativa usa o texto “Equivalente API estimado” e deixa claro que não é cobrança da assinatura.

## Critérios de aceite

1. Atualizar não abre `cmd`, PowerShell, terminal ou janela auxiliar visível.
2. Overview, bandeja, tooltip, alertas, configurações e atualização automática continuam funcionando.
3. Histórico abre e volta por mouse, teclado e Escape sem redimensionar a janela.
4. 7 dias, 30 dias e Tudo recalculam totais, gráfico e lista de modelos.
5. Uma sessão com troca de modelo é atribuída corretamente por turno.
6. Cached input usa preço próprio e reasoning não é duplicado no custo.
7. O usuário vê cobertura da estimativa quando houver modelo sem preço.
8. A aba Conta diferencia atividade agregada da conta de uso local por modelo.
9. Hermes só aparece como ativo/configurado quando metadados allowlisted comprovarem isso; nenhum token ou segredo entra no contrato, logs ou diagnóstico.
10. Falha dos RPCs opcionais de perfil/atividade não derruba a leitura das quotas.
11. Testes Rust e frontend, build web e build release passam.
12. QA final no app instalado confirma layout 400 × 560, scroll, estados e ausência de pop-ups.

## Fora de escopo

- Cobrança real da assinatura ou da API.
- Atribuição de consumo por cliente remoto (por exemplo, “Hermes gastou X%”).
- Integração pessoal não oficial de Cursor.
- Alteração automática do `statusLine` do Claude.
