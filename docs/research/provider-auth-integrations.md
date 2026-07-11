# Pesquisa: providers, escopo de quota e histórico local

**Data da verificação:** 11 de julho de 2026

**Escopo:** OpenAI Codex + Hermes Agent, Claude Code e Cursor.
**Regra de segurança:** nenhuma credencial, claim de JWT, e-mail, ID de conta ou conteúdo de conversa foi extraído. A inspeção local ficou restrita a existência de arquivos/binários, nomes de chaves, versão e metadados operacionais não secretos.

## Veredito executivo

| Capacidade | Veredito | Por quê |
|---|---|---|
| Quota Codex ao vivo | **Implementável agora / estável** | `codex app-server` documenta `account/rateLimits/read`, com percentual usado, duração da janela e reset. É o mecanismo que o QuotaBuddy já usa. |
| Atividade Codex da conta por dia | **Implementável agora / estável no Codex atual** | `account/usage/read` retorna resumo de tokens da conta e buckets diários. Não divide por cliente ou modelo. |
| Mostrar que Hermes pode consumir a quota Codex | **Implementável agora como descoberta, sem atribuição** | Hermes possui provider oficial `openai-codex` via OAuth e usa o backend Codex da conta ChatGPT. É seguro detectar provider/configuração; não é seguro afirmar que a conta é a mesma sem ler identidade da credencial. |
| Dizer quanto da quota foi gasto especificamente pelo Hermes | **Não implementável com confiança** | O snapshot Codex é do escopo da conta/plano e não traz divisão por cliente. Os rollouts locais do Codex também não contêm as chamadas feitas diretamente pelo Hermes. |
| Claude Code individual: quota atual | **Implementável com bridge opt-in** | O contrato oficial de `statusLine` entrega percentuais e resets de 5 horas/7 dias, sem consumir tokens, mas somente enquanto o Claude Code está ativo e depois da primeira resposta. Não há comando standalone documentado para polling silencioso da assinatura. |
| Claude Code Team/Enterprise: histórico | **Implementável com credencial administrativa** | A API oficial `usage_report/claude_code` fornece dia, modelo, tokens e custo estimado. Exige chave Admin/Analytics e não está disponível para conta individual. |
| Cursor individual: quota/créditos | **Não disponível oficialmente** | O CLI oficial não oferece comando de usage e não há API pública individual estável. Não se deve raspar dashboard, SQLite interno ou storage do Electron. |
| Cursor Teams: uso/custo | **Implementável, ainda experimental** | A Admin API oficial está em “first release”, exige chave de administrador do time e expõe eventos, tokens e custos. |
| Histórico Codex 7d/30d/tudo | **Implementável agora / híbrido** | `account/usage/read` cobre atividade agregada da conta; rollouts locais acrescentam modelo e tipos de token. A leitura local precisa ser estrita e somente de campos permitidos. |
| “Custo” da assinatura | **Não calcular** | Preço de API, créditos Codex e mensalidade da assinatura são sistemas diferentes. O app pode mostrar apenas **custo equivalente de API** e, quando houver tabela oficial compatível, **créditos Codex estimados**. |

## 1. OpenAI Codex e Hermes Agent

### 1.1 O que `account/rateLimits/read` mede

O protocolo oficial do app-server define:

- `usedPercent`: consumo atual dentro da janela de quota OpenAI;
- `windowDurationMins`: duração da janela;
- `resetsAt`: timestamp Unix do próximo reset;
- `rateLimitReachedType`: classificação do limite atingido;
- `individualLimit`: limite mensal efetivo, quando o backend o disponibiliza.

Fonte: [OpenAI Codex app-server, seção Rate limits](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md#7-rate-limits-chatgpt).

Isso é **quota da conta/workspace ativa**, não contagem de tokens do processo local. A documentação da OpenAI trata Codex app, CLI, extensão de IDE e web como superfícies do mesmo produto autenticadas pela conta ChatGPT; o uso Codex entra no limite agentic do plano, que também pode ser compartilhado com outras superfícies agentic elegíveis. Portanto, o percentual pode mudar por uso fora do computador atual. Fontes: [Using Codex with your ChatGPT plan](https://help.openai.com/en/articles/11369540-using-codex-with-your-chatgpt-plan) e [Codex rate card](https://help.openai.com/en/articles/20001106-codex-rate-card).

O Codex atual também documenta `account/usage/read`, separado da quota, para buscar um resumo de atividade de tokens da conta e buckets diários. O contrato expõe `lifetimeTokens`, pico diário, duração do maior turno, streaks e buckets `{ startDate, tokens }`; não expõe modelo nem tipos de token. Fontes: [API overview](https://github.com/openai/codex/blob/5c19155cbd93bfa099016e7487259f61669823ff/codex-rs/app-server/README.md#L1794-L1805) e [tipos da resposta](https://github.com/openai/codex/blob/5c19155cbd93bfa099016e7487259f61669823ff/codex-rs/app-server-protocol/src/protocol/v2/account.rs#L392-L452).

**Conclusão prática:** clientes Codex autenticados na mesma conta/workspace observam o mesmo escopo de quota. O RPC não informa qual cliente originou cada parcela do consumo. `account/usage/read` permite mostrar “atividade total da conta”, enquanto os rollouts permitem mostrar “detalhe local por modelo”.

### 1.2 Como o Hermes suporta Codex OAuth

No código oficial instalado do Hermes:

- `openai-codex` é um provider separado de `openai-api`; o primeiro usa OAuth externo e o segundo usa API key ([registro de providers](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/hermes_cli/auth.py#L176-L195));
- a documentação descreve OpenAI Codex como ChatGPT OAuth via device code ([providers oficiais](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/website/docs/integrations/providers.md#L15-L18));
- o login pode importar uma sessão existente do Codex CLI, mas recomenda criar uma sessão OAuth separada para evitar conflito de rotação do refresh token ([fluxo de login](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/hermes_cli/auth.py#L6920-L6991));
- as credenciais próprias do Hermes ficam no auth store do Hermes, não no arquivo do Codex ([persistência separada](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/hermes_cli/auth.py#L3361-L3386));
- o endpoint de inferência do provider é `chatgpt.com/backend-api/codex`, o mesmo domínio de produto Codex ([configuração do provider](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/hermes_cli/auth.py#L176-L195)).

O upstream do Hermes hoje consulta `/wham/usage` (ou `/api/codex/usage`) e envia `ChatGPT-Account-Id` quando disponível, exatamente o formato de escopo usado pelo backend Codex. Fontes: [resolução do endpoint](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/agent/account_usage.py#L428-L448) e [consulta de quota](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/agent/account_usage.py#L466-L540).

Se o usuário autenticar Hermes e Codex na **mesma identidade e mesmo workspace/account ID**, ambos consultam e consomem as mesmas janelas server-side. A sessão OAuth separada evita conflito de refresh token; não cria outra franquia. Ainda assim, o QuotaBuddy não deve abrir tokens para comparar identidades: a relação exibida deve ser `confirmed` somente após confirmação explícita do usuário e `inferred` quando vier apenas da configuração.

### 1.3 Descoberta segura do Hermes

O Hermes resolve `HERMES_HOME` por variável de ambiente e, no Windows, usa `%LOCALAPPDATA%\hermes` como padrão ([resolução oficial de diretório](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/hermes_constants.py#L34-L87)). O QuotaBuddy pode detectar somente:

1. `hermes_installed`: executável/diretório esperado existe;
2. `config_present`: `config.yaml` existe;
3. `configured_provider`: valor permitido de `model.provider` (`openai-codex`, `auto` ou outro slug público);
4. `codex_credential_record_present`: existe a chave estrutural `credential_pool.openai-codex` ou `providers.openai-codex` no auth store;
5. `codex_consumer_state`: `active`, `possible`, `not_configured` ou `unknown`.

Não ler nem devolver `access_token`, `refresh_token`, `api_key`, `secret_fingerprint`, label de conta, e-mail ou mensagens de erro cruas. Também **não chamar diretamente** `get_codex_auth_status()` para a UI: a função oficial retorna `api_key` no mesmo objeto de status ([fonte](https://github.com/NousResearch/hermes-agent/blob/60b1f6ce3f26c57dac480265fbf4a38e7a5c3a25/hermes_cli/auth.py#L6151-L6213)).

Para diagnóstico manual, o upstream oferece `hermes auth status openai-codex` e `hermes auth list openai-codex`, cujas saídas públicas são limitadas a login, contagem, label, auth type, source e status/cooldown ([status](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/hermes_cli/auth_commands.py#L487-L504), [list](https://github.com/NousResearch/hermes-agent/blob/7acaff5ef2bcbaa22bd23b72efe60906123a4f55/hermes_cli/auth_commands.py#L415-L438)). Mesmo assim, o app deve preferir leitura estrutural nativa e não abrir um CLI a cada refresh; se um diagnóstico sob demanda vier a usar o comando, o processo deve ser oculto e a saída passar por allowlist.

Copy recomendada quando `openai-codex` estiver ativo:

> Hermes detectado como consumidor Codex. Este limite é compartilhado quando Hermes e Codex usam a mesma conta. Por privacidade, o QuotaBuddy não compara identidades de login.

Quando apenas `~/.codex/auth.json` existir, não mostrar Hermes como consumidor: presença de credencial Codex não prova que Hermes a importou ou usa esse provider.

## 2. Histórico local do Codex

### 2.1 Campos oficiais úteis

O protocolo do Codex define `TokenUsage` com:

- `input_tokens`;
- `cached_input_tokens`;
- `output_tokens`;
- `reasoning_output_tokens`;
- `total_tokens`.

`TokenUsageInfo` contém tanto `total_token_usage` cumulativo quanto `last_token_usage` da atualização mais recente e `model_context_window`. Fonte: [OpenAI Codex protocol.rs](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs#L2025-L2046).

O evento `TokenCount` carrega usage e, opcionalmente, um `RateLimitSnapshot`; são métricas diferentes. Fonte: [TokenCountEvent e RateLimitSnapshot](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs#L2106-L2122).

O modelo efetivo fica em `turn_context.payload.model` e pode mudar no meio do mesmo rollout. Fonte: [TurnContextItem.model](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs#L3218-L3241).

Cada linha JSONL possui timestamp próprio. Para histórico correto:

1. aceitar somente `type=turn_context` e `type=event_msg` com `payload.type=token_count`;
2. associar cada `token_count` ao último modelo observado no `turn_context` anterior;
3. somar `last_token_usage`, não repetir o cumulativo `total_token_usage`;
4. usar o timestamp no topo da linha como `occurred_at` e `ordinal`, quando presente, para ordenação/dedupe;
5. manter input normal, cached input, output e reasoning separados;
6. ignorar completamente `response_item`, `user_message`, tool outputs, cwd e conteúdo de conversa;
7. quando `last_token_usage` faltar em versões antigas, calcular delta monotônico do cumulativo por arquivo; em reset/truncamento, iniciar nova sequência;
8. marcar cobertura parcial se houver linha malformada, modelo desconhecido ou evento sem usage.

O scanner atual do QuotaBuddy busca recursivamente qualquer chave `model`, `input_tokens` e `output_tokens` e usa o máximo por arquivo. Isso é suficiente para a estimativa simples da V1, mas não para histórico por data/modelo, troca de modelo ou preço de cached input. A evolução deve usar caminhos JSON estritos e manter compatibilidade com a estimativa atual até os novos testes provarem paridade.

O código atual já faz um request best-effort a `account/usage/read` e descarta o payload. Portanto, a primeira evolução de menor risco é normalizar os campos allowlisted desse RPC. O detalhamento local por modelo continua em um scanner separado; nunca misturar os dois como se tivessem a mesma cobertura.

### 2.2 Histórico de quota

`account/rateLimits/read` é um snapshot atual e não traz timestamp de captura. O QuotaBuddy deve anexar `captured_at` local em cada sucesso e persistir uma amostra somente quando:

- percentual/reset/limit ID mudar; ou
- passar um intervalo de heartbeat (por exemplo, 15 minutos).

Rollouts podem conter `rate_limits`, mas o campo é opcional e snapshots reprocessados podem estar antigos. Para o gráfico, priorizar `source=live_rpc`; usar `source=rollout` apenas como backfill marcado como estimado/stale.

### 2.3 Snapshot seguro desta máquina

Leitura somente de metadados em 11/07/2026:

- amostra: 50 rollouts recentes;
- cobertura temporal da amostra: 16/05/2026 a 11/07/2026;
- 16.698 eventos `token_count`; 16.696 continham snapshot de rate limit;
- model IDs encontrados em `turn_context`: `gpt-5.6-sol`, `gpt-5.6-terra` e `gpt-5.5`;
- campos de token observados: input, cached input, output, reasoning output e total.

Isso confirma viabilidade local, mas os totais dos **rollouts** não incluem chamadas do Hermes direto ao backend, tarefas cloud sem rollout local ou outros produtos que compartilhem o pool agentic. Os buckets de `account/usage/read` são a visão agregada apropriada para esse total; eles não permitem atribuição por consumidor nem cálculo confiável de custo por modelo.

## 3. Claude Code

### 3.1 Interface individual suportada

O comando interativo `/usage` mostra limites e resets; `/status` ajuda a confirmar a credencial ativa. Porém, a referência do CLI não documenta um comando standalone, não interativo e somente-leitura para consultar a quota de uma assinatura. Fontes: [Claude Code errors/usage limits](https://code.claude.com/docs/en/errors#usage-limits) e [CLI reference](https://code.claude.com/docs/en/cli-reference).

A superfície oficial adequada é `statusLine`. O Claude Code envia JSON local para um comando configurado pelo usuário e documenta:

- `model.id` e `model.display_name`;
- `rate_limits.five_hour.used_percentage` e `rate_limits.seven_day.used_percentage`;
- os dois `resets_at` em epoch seconds;
- `context_window.current_usage`, input/output e contexto;
- `cost.total_cost_usd`, explicitamente descrito como estimativa client-side que pode diferir da fatura;
- `session_id`, `prompt_id`, `transcript_path` e versão.

A própria documentação afirma que a status line roda localmente e não consome tokens. Os campos de quota aparecem para assinantes Pro/Max depois da primeira resposta. Fonte: [Customize your status line](https://code.claude.com/docs/en/statusline#available-data).

**Contrato recomendado:** bridge opt-in que recebe o JSON da status line, mantém apenas campos allowlisted e grava atomicamente um cache do QuotaBuddy. O app lê esse cache; não abre o arquivo de credenciais e não inicia sessões Claude.

Restrições obrigatórias:

- não substituir silenciosamente uma `statusLine` existente;
- oferecer integração/desintegração reversível e preservar o comando anterior;
- deduplicar histórico por `prompt_id` (ou `session_id` + assinatura do uso em versões antigas);
- remover `cwd`, workspace, transcript path e session ID antes do frontend;
- mostrar “ao vivo quando Claude Code está ativo”;
- nunca executar `claude -p "/usage"` em refresh periódico: cria uma sessão, pode consumir quota e não é um endpoint de monitoramento.

### 3.2 Times e organizações

Para Team/Enterprise, há interface oficial mais forte. `GET /v1/organizations/usage_report/claude_code` retorna data, tipo de cliente, modelo, tokens (input, output, cache creation/read) e custo estimado. Exige chave Admin/Analytics e não serve para conta individual. Fontes: [Claude Code Usage Report](https://platform.claude.com/docs/en/api/admin/usage_report) e [Usage and Cost API](https://platform.claude.com/docs/en/manage-claude/usage-cost-api).

Uma integração futura é segura se a chave ficar no Windows Credential Manager, tiver consentimento explícito e nunca cruzar a boundary Rust → frontend.

### 3.3 Escopo compartilhado Claude

Anthropic documenta que Claude web, Desktop e Claude Code contam para o mesmo limite quando usados sob a mesma assinatura; IDEs suportadas também entram nesse limite. Fonte: [Use Claude Code with your Pro or Max plan](https://support.claude.com/en/articles/11145838-use-claude-code-with-your-pro-or-max-plan).

Logo, a UX pode mostrar “Escopo: assinatura Claude” e consumidores oficiais detectados, mas não atribuir percentuais por app.

## 4. Cursor

### 4.1 Individual

O CLI oficial documenta login/logout, execução de agente, modelo e formatos de saída, mas não um comando de usage/créditos. Fonte: [Cursor CLI parameters](https://docs.cursor.com/en/cli/reference/parameters).

Não há fonte oficial estável para o QuotaBuddy consultar quota de conta individual. São **não seguros**:

- endpoints privados observados no dashboard;
- cookies/session storage;
- `state.vscdb`, IndexedDB ou arquivos internos do Electron;
- automação de tela do dashboard;
- interceptação de requests do editor.

Para usuário individual, mostrar apenas `Cursor instalado · medição indisponível oficialmente`.

### 4.2 Teams

A Admin API oficial do Cursor, ainda descrita como primeira versão, permite:

- `daily-usage-data`: requisições incluídas, API key e usage-based, além de modelo mais usado;
- `spend`: gasto e ciclo da assinatura;
- `filtered-usage-events`: timestamp, model, kind, custo, token-based flag e, quando disponível, input/output/cache tokens e total em centavos.

Ela exige chave criada por administrador do time. Fonte: [Cursor Admin API](https://docs.cursor.com/en/account/teams/admin-api).

Veredito: adapter opcional para Teams, marcado `experimental`, sem qualquer fallback para scraping.

## 5. Preços e linguagem de custo

### 5.1 Fontes atuais

Preços API relevantes aos modelos observados localmente:

| Modelo | Input / MTok | Cached input / MTok | Output / MTok | Fonte |
|---|---:|---:|---:|---|
| GPT-5.6 Sol | US$ 5,00 | US$ 0,50 | US$ 30,00 | [OpenAI](https://developers.openai.com/api/docs/models/gpt-5.6-sol) |
| GPT-5.6 Terra | US$ 2,50 | US$ 0,25 | US$ 15,00 | [OpenAI](https://developers.openai.com/api/docs/models/gpt-5.6-terra) |
| GPT-5.5 | US$ 5,00 | US$ 0,50 | US$ 30,00 | [OpenAI models](https://developers.openai.com/api/docs/models) |
| GPT-5.3-Codex | US$ 1,75 | US$ 0,175 | US$ 14,00 | [OpenAI](https://developers.openai.com/api/docs/models/gpt-5.3-codex) |

Para Anthropic, a tabela oficial atual separa input, cache write de 5 min/1 h, cache read e output. Por exemplo, Opus 4.6 é US$ 5 / 6,25 / 10 / 0,50 / 25 por MTok; Sonnet 4.6 é US$ 3 / 3,75 / 6 / 0,30 / 15. Fonte: [Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing).

Preços mudam. A tabela embarcada precisa ter `version`, `checked_at`, `effective_from`, `source_url` e todas as categorias de token; modelo não reconhecido deve ficar como “sem preço”, nunca cair silenciosamente em outro modelo.

### 5.2 Três métricas diferentes

1. **Quota restante:** percentual/janela do plano. É o indicador operacional principal.
2. **Créditos Codex estimados:** somente quando o rate card oficial tiver mapeamento explícito para aquele model ID e workspace. O rate card de 2026 passou a mapear input/cached/output em créditos, mas workspaces legados podem seguir outra tabela.
3. **Custo equivalente de API:** `tokens × preço público da API`, sempre rotulado como estimativa comparativa.

Nunca chamar (2) ou (3) de “cobrança”, “fatura”, “gasto real” ou “economia da assinatura”. Uma assinatura pode incluir uso, usar créditos adicionais, ter descontos, fast mode, long-context multiplier, ferramentas pagas ou preço negociado.

Copy curta recomendada:

> Equivalente API estimado — comparação pelo preço público atual. Não é cobrança da sua assinatura.

## 6. Contrato seguro proposto

```ts
type Confidence = "confirmed" | "inferred" | "unknown";
type IntegrationState = "available" | "experimental" | "unsupported";

type ProviderDiscovery = {
  provider: "codex" | "claudeCode" | "cursor" | "hermes";
  installed: boolean;
  credentialRecordPresent: boolean; // nunca a credencial
  activeProvider?: string;           // slug público allowlisted
  integration: IntegrationState;
  lastCheckedAt: string;
};

type QuotaScope = {
  id: "openai-agentic-active" | "claude-subscription-active" | "cursor-team-active";
  label: string;
  scopeKind: "account" | "workspace" | "organization" | "unknown";
  consumers: QuotaConsumer[];
};

type QuotaConsumer = {
  consumer: "codexApp" | "codexCli" | "codexIde" | "codexWeb" |
            "hermesOpenAiCodex" | "claudeWeb" | "claudeCode" | "cursor";
  detected: boolean;
  active?: boolean;
  sharesScope: Confidence;
  reasonCode: "officialSameAccount" | "providerConfigured" |
              "credentialMetadataOnly" | "notMeasurable";
};

type LocalUsageEvent = {
  provider: "codex" | "claudeCode" | "cursor";
  occurredAt: string;
  modelId: string;
  inputTokens: number;
  cachedInputTokens: number;
  cacheWriteTokens?: number;
  outputTokens: number;
  reasoningOutputTokens?: number;
  source: "codexRollout" | "claudeStatusLine" | "providerAdminApi";
  dedupeKey: string; // hash local; nunca thread/session/account ID na UI
};

type UsageSummary = {
  range: "7d" | "30d" | "all";
  totals: {
    inputTokens: number;
    cachedInputTokens: number;
    outputTokens: number;
    reasoningOutputTokens: number;
  };
  byModel: Array<{ modelId: string; tokens: number }>;
  apiEquivalentUsd?: number;
  estimatedCodexCredits?: number;
  pricingVersion?: string;
  coverage: "completeForSource" | "partial" | "unknown";
  disclaimer: string;
};

type AccountTokenActivity = {
  provider: "codex";
  lifetimeTokens?: number;
  peakDailyTokens?: number;
  longestRunningTurnSeconds?: number;
  currentStreakDays?: number;
  longestStreakDays?: number;
  daily: Array<{ startDate: string; tokens: number }>;
  coverage: "accountAggregate";
};

type QuotaHistorySample = {
  scopeId: QuotaScope["id"];
  capturedAt: string;
  windowMinutes?: number;
  usedPercent?: number;
  resetsAt?: string;
  source: "liveRpc" | "rolloutBackfill" | "claudeStatusLine" | "providerAdminApi";
  stale: boolean;
};
```

### Regras de boundary

- O frontend recebe somente os contratos normalizados acima.
- Paths, stdout/stderr, provider error bruto e identificadores de conta não atravessam a boundary.
- Chaves administrativas ficam no Windows Credential Manager; cache local contém somente agregados.
- Discovery não valida token, não decodifica JWT e não chama endpoint de identidade.
- Histórico guarda agregados e cursores de leitura; nunca copia prompt, resposta ou tool output.
- Exportação diagnóstica informa cobertura, versões e reason codes, mas não nomes de arquivos pessoais.

## 7. Direção de UX

No card Codex:

- título secundário: `Escopo: conta OpenAI / pool agentic`;
- chips de consumidor: `Codex app`, `CLI`, `IDE`, `Web` e, quando detectado, `Hermes`;
- Hermes com estado `mesma conta não verificada` até haver confirmação explícita do usuário;
- tooltip: `O percentual é do limite da conta. O backend não informa qual app consumiu cada parcela.`;
- histórico com tabs `7 dias`, `30 dias`, `Tudo`;
- linha de cobertura: `Histórico local Codex — não inclui Hermes e tarefas sem rollout local`;
- alternância clara: `Conta — todos os clientes` (buckets do RPC) e `Neste PC — por modelo` (rollouts);
- métricas: tokens por modelo, cached ratio, equivalente API e curva de quota.

No card Claude:

- `Conectar monitor local` para instalar a bridge de status line com consentimento;
- `Atualiza quando Claude Code responde`;
- se houver configuração preexistente, pedir escolha e preservar integralmente;
- Team/Enterprise: opção separada `Conectar Analytics API`.

No card Cursor:

- individual: `Instalado · uso não exposto oficialmente`;
- time: `Conectar Admin API (experimental)`;
- nunca sugerir que falta de integração é erro do usuário.

## 8. Ordem recomendada

1. **Agora:** escopo/consumidores do Codex + descoberta segura do Hermes, sem mudar a leitura de quota existente.
2. **Agora:** normalizar `account/usage/read` para atividade total da conta e, em paralelo, criar extrator estrito de rollouts para detalhe local 7d/30d/all e preço versionado.
3. **Depois:** bridge opt-in Claude `statusLine`, preservando configuração existente e sem subprocesso no refresh do QuotaBuddy.
4. **Depois:** Claude Team/Enterprise Analytics API com Credential Manager.
5. **Opcional:** Cursor Teams Admin API como experimental.
6. **Deixar quieto:** Cursor individual por scraping; comparação de tokens/identidade OAuth; atribuição de consumo ao Hermes; qualquer leitura de credencial.

## 9. Evidência local resumida

- Hermes Desktop/source oficial instalado na revisão `60b1f6ce3f26c57dac480265fbf4a38e7a5c3a25`.
- Configuração Hermes observada: provider `auto`, modelo `anthropic/claude-opus-4.6`; nenhum pool estrutural `openai-codex` presente no auth store no momento da inspeção. Logo, **não é correto marcar Hermes como consumidor Codex ativo nesta máquina hoje**.
- O arquivo de auth do Codex CLI existe; isso, isoladamente, não prova uso pelo Hermes.
- Claude Code `2.1.205` instalado; arquivo de credencial presente e `statusLine` já configurada. Não foi feita validação do token.
- Cursor Desktop instalado; `cursor-agent` não detectado.

Esses estados são um snapshot e devem ser redetectados sem cache longo.
