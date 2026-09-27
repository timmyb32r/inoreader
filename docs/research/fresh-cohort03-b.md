# Независимый аудит candidate04: группа B, семь новых статей

**Завершено: проверены все 7/7 финальных review responses. Подтверждённых ложных существенных утверждений не найдено. Это не безусловный pass: в #442 добавлена расшифровка HCL, отсутствующая в разрешённом snapshot/справочнике, а локальные определения, списки и отбор деталей во всех семи случаях исполнены не полностью.** Formal JSON/title gate пройден 7/7; semantic source-grounding и стиль рассмотрены отдельно. Проверка этой группы не отменяет ошибок в других группах candidate04 и не означает авторской приёмки.

Полностью прочитаны семь исходников, семь разрешённых авторских эталонов и семь settled final outputs; draft из каждого review request также сопоставлен с результатом. Reasoning не читался.

Candidate04 заморожен **2026-09-26T23:35:36.469837+00:00** до чтения новых эталонов/ответов: draft v17 / deepseek-v4-pro high / 16 384 output tokens; review v3 / deepseek-v4-pro low / 16 384. Во время аудита промпты, параметры, исходники и ответы не меняются. Новых API-вызовов аудит не делает. Читается только `message.content`, не reasoning. Отбор фиксирован: **916, 409, 602, 1286, 838, 9, 442**. Из `corpus/posts.jsonl` до отображения отфильтрованы только эти семь IDs.

Источники: `.inoreader-state/style-research/pairs/final-03.jsonl`; все семь `text_path` прочитаны целиком, всего **62 429 bytes / 555 строк**. Строки ниже отсчитываются от единицы в соответствующем `.txt` / `article_snapshot.text`. Источники получены с актуальных сайтов, не из архивов на день Telegram-поста; изображения не OCRed. Exact title/URL подтверждены исходным metadata, смысловое соответствие ниже проверено отдельно.

| ID | Source text file | Строк | Слов полного авторского reference* | Статус результата |
|---|---|---:|---:|---|
| 916 | `sources/final03-916-direct.txt` | 37 | 183 | Проверен: definite facts не найдены; style partial |
| 409 | `sources/final03-409-direct.txt` | 111 | 112 | Проверен: facts pass; иной отбор, списки не выполнены |
| 602 | `sources/final03-602-direct.txt` | 107 | 258 | Проверен: facts pass; нет Trace, local acronym partial |
| 1286 | `sources/final03-1286-direct.txt` | 41 | 303 | Проверен: facts pass; шаги стрелками, определения partial |
| 838 | `sources/final03-838-utf8.txt` | 203 | 137 | Проверен: experiments/units/status верны; jargon dense |
| 9 | `sources/final03-9-jina.txt` | 29 | 70 | Проверен: authorization scope верен; definitions partial |
| 442 | `sources/final03-442-jina.txt` | 27 | 253 | Проверен: нет false claim; HCL expansion вне source |

\* Здесь `reference.text.split()` целиком, включая заголовок/URL, если они есть. Эти числа не следует смешивать с `body_words` evaluator, исключающим первую строку заголовка.

## До чтения результатов: source/reference matching и существенные границы

### 916 — Monte Carlo Remediation Agent

Совпадают заголовок и основная тема. Reference подробно объясняет разделение плана/исполнения списками, hand-off при недостаточных доказательствах, права и подтверждение; завершает разговорным авторским выводом. Source L5,19: после **завершённого** анализа Troubleshooting Agent автоматически создаётся план, новый отдельный запуск не нужен. L19: classification/summary/confidence. L21: fix code / rerun / tune / close, при слабых доказательствах hand-off. L23: coding agent получает через MCP дефект, suspect PR/query, fix и verification steps; L27: **Tuning Agent** выводит настройку из истории алертов и применяет её. L29: Monte Carlo не трогает код; подтверждение и account permissions описаны в контексте coding agent. Не расширять это до «никаких изменений нигде», поскольку monitor tuning действительно применяется. L37: rolling out, accounts with Troubleshooting analyses; hosted MCP требует конфигурации для передачи плана.

### 409 — Microsoft Databases

Reference выбирает четыре инвестиционных направления, не награды/отзывы; это точно соответствует source L79/83/89/93. Source L79: SQL Server 2025 vectors, embedding, semantic search, RAG; не обещана безусловная изоляция всех данных от внешних систем. L83: Azure SQL Hyperscale, auto tuning, elastic management, Foundry/Fabric. L89: **Azure HorizonDB public preview** и Oracle→PostgreSQL tooling; L93: Cosmos vector/hybrid search, reranking, GSI, per-partition failover, change feed, Linux Emulator. Результаты PeerSpot — отзывы/recognitions (L9–23), не сравнительный benchmark. Авторский reference называет должность автора, однако извлечённый полный article body её не содержит: атрибуцию должности нельзя подтверждать одной этой snapshot. Хорошая компрессия может отбросить маркетинговый вводный блок, не превратить roadmap/preview в повсеместно доступное GA.

### 602 — Cloudflare Agentic Internet

Reference выделяет основные продукты и противопоставляет **Precursor** и demo **Precursor Trace**. Source L1–5: люди могут вредить, боты помогать, сессия переключается human↔agent; анализ непрерывного поведения. L11–13: risk и trust — независимые, взаимосвязанные величины, не просто противоположные концы шкалы. L26/30: BotBase каталог также плохих ботов; verified можно потерять. L34–38: CDN-injected JS и continuous session assessment. L44: **206 million evaluation events за 24 h / 73 438 zones**, не люди/боты/успешные блокировки. L56–59: Trace демонстрирует лишь часть механизма. L61–69: **Adaptive Intelligence coming soon**, не уже всем доступен. L75–91: random actions, Labyrinth Maze/Summary/Poison и queuing для хороших ботов планируются ближе к концу года; не гарантированно ловят каждого бота. Старый reference пишет о модели настоящим временем и не сохраняет rollout-ограничения; соответствие ему не оправдывает нарушение snapshot.

### 1286 — Airbnb Insight Miner

Совпадение темы и архитектурной идеи полное. Reference значительно подробнее короткого announcement, с Extract/Embed/Cluster списком, понятными определениями и выходными артефактами. Source L3 автор **Wren Dougherty**; L5 **100 000 conversations — вводный мысленный пример**, не замер объёма запуска Insight Miner. L7 метод как продукт/harness для framing/evidence/decisions. L11 подготовка AI support assistant в 2025 и редкие рискованные события. L13–15 ручные исследования по несколько месяцев, новые языки/регионы/продукты почти еженедельно. L19 extract/embed/cluster + prompt tuning, hard-example mining, contrastive labeling, unsupervised→classification; L21 вопрос в chat, разные текстовые источники. L23 месяцы→дни — сообщённый результат этой команды, не универсальная гарантия. L25 больше человеческого judgment на сложных случаях. L29 через год **десятки команд / сотни типов исследований**, нетехнических пользователей больше технических; не путать команды/людей/число завершённых исследований. L35 отдельные agentic systems помогают обновлять инструкции/ловить ошибки, не полностью автономное изменение production.

### 838 — CARE-X

Заголовок совпадает, но **нынешний source существенным образом уточняет старый авторский reference**. Reference приписывает CARE-X одновременно генерацию, calibrated heads и детерминированные измерения. Snapshot L11,127,191 явно отделяет measurement experiment **Qwen3-VL-4B-Instruct** от **CARE-X**. Повторение старого смешения здесь было бы фактической ошибкой относительно предоставленного источника.

Source L1/195: research model, не medical device/product, нет regulatory clearance, не intended/validated для клинического применения. L73/77–81: dual inference даёт autoregressive + structured auxiliary output; CARE-X = SigLIP2-so400M + Phi-4-mini-instruct 3.8B, совместно обученные головы, три supervised стадии и DAPO RL. L89: improvements в **percentage points**, не относительных процентах. L115–121: лидирующие метрики ограничены reported comparison set, ReXVQA **94% overall / +6 pp / 41 007 QA / as of Aug 2026**, не клиническая точность на любых пациентах. L127–149: отдельный tool experiment, средняя F1 gain **43.6 pp по пяти состояниям**. L157–171: CARE-X 1 047 снимков, prevalence 2.6–5.2%, highest sensitivity в трёх из пяти условий. L175–187: другой положительный CT-confirmed cohort **122 cases**, recall94.26%, +10.65pp; связанное исследование 40/43 vs5/43 mild dilation; без negatives нельзя вывести specificity/clinical utility. Source сам подчёркивает это ограничение. Скрининг — потенциальное направление будущих исследований, не рекомендованное применение.

### 9 — Enterprise-Managed Authorisation

Reference совпадает по stable EMA и централизованному login, но «выписать токен сразу на скоуп MCP серверов» упрощает протокол. Source L5: **ID-JAG обменивается на access token auth-сервером конкретного MCP server**, а не один bearer-token для всех. L7: connection/client/server/scope policy, **после выдачи токена MCP traffic не проверяется**, per-action runtime authorization не предоставляется. L15: нужны совместимые IdP и MCP server; Okta Cross App Access первый названный путь, остальные нуждаются в fallback. L23: Claude/Code/Cowork/VSCode и перечисленные серверы поддерживают, Slack **in progress**. L27 InfoQ author Matt Saunders. Старый reference говорит team часть Anthropic, snapshot этого не утверждает (называет Anthropic adopter): не принимать такую организационную атрибуцию на веру.

### 442 — Terraform tfpolicy

Совпадают тема и главные возможности. Source L1: **public beta within HCP Terraform**. L3/9 HCL-based policy-as-code, знакомый Terraform workflow. L7–15 resource relationships, external data source lookups, provider/module download control, post-deploy actual state. L17 **Sentinel поддерживается**, tfpolicy preferred + agent generation/testing/conversion. L19 **full features only HCP**, standalone CLI primarily validation/local testing, не полный managed enforcement. L21 Sentinel native HCP integration и собственный DSL, OPA vendor-neutral100+integrations. Reference дополнительно перечисляет конкретные IAM/AMI примеры и тройку init/plan/apply: full snapshot поддерживает download-control и общие этапы, но точные пример/названия стадий здесь не приведены. L25 author Sergio De Simone. Следить, чтобы «tfpolicy раньше/после deploy» не стало «Sentinel вообще не умеет такой интеграции».

## Результаты

Каждый результат ниже прочитан после `settled`; `article_snapshot.text` проверен на точное равенство source file, включая конечный newline. Метрики результата — `evaluate_outputs.evaluate`, слова после заголовка с source line. Source/ref analysis выполнен до этих результатов.

### 916 — факты подтверждены, локальные определения неполны

Run **8c5652cc-fce5-407a-83ac-d92473984cbb**, label `final-03-916-candidate-04-review`. Stop, complete JSON, flags пусты; **227 body words / 8 bold fragments**, quote segments отсутствуют. Review-only cost **$0.027966136**.

Проверены все утверждения: определение Monte Carlo из frozen verified glossary; Remediation автоматически после завершённого Troubleshooting (L5/19); diagnosis/evidence/checklist (L1); трудность маршрутизации/передачи работы (L9–11); classification/confidence и четыре действия/hand-off (L19–21); hosted MCP, состав плана и работа coding agent в своём окружении (L23/37); Tuning Agent выводит настройку по alert history (L27); account permissions/no shared credentials/confirmation/code boundary (L29). **Нового definite factual defect нет.** Формулировка последнего абзаца «платформа только предлагает план» читается с явно названным выше применением monitor tuning и ограничением «не трогает код»; без этого контекста она была бы чрезмерно общей.

Verifier действительно исправил draft «маршрутизация и подтверждение — основная трата времени» на утверждение про маршрутизацию и hand-off, которое source поддерживает. Он также убрал полную расшифровку MCP, отсутствующую в snapshot, и дал функциональное описание сервера. Это не потеря подтверждённого факта: полное название в этом исходнике не дано.

**Style:** source/бизнес Monte Carlo, TL;DR, понятные agent roles, bold сущностей и список четырёх действий есть. Reference короче (183 слова всего), разговорнее, но главная план/исполнение мысль сохранена без приписывания личного мнения. Есть повторение содержания плана в TL;DR и далее, сложный длинный абзац про передачу coding-агенту. **Локальность не полностью выполнена:** TL;DR содержит голый MCP-сервер; последний отдельный абзац тоже MCP; PR нигде не объясняется. Расшифровку по памяти считать обязательной нельзя, но можно было убрать/понятно заменить аббревиатуры. Состав full plan из четырёх элементов опять спрятан в строке вместо требуемого списка. Ни JSON, ни фактический pass не засчитывают эти style gaps как исполненные.


### 409 — факты подтверждены; маркетинговый акцент сильнее эталона

Run **8a37e549-4fe3-4301-b40a-ef78bc03df0b**, `final-03-409-candidate-04-review`. Stop, complete JSON, no flags/quotes, **104 body words / 10 bold fragments**; review-only cost **$0.022288816**. Snapshot точь-в-точь равен source file.

PeerSpot awards основаны на отзывах production practitioners (L9–19); перечисленные четыре продукта верны; пять клиентских приоритетов перечислены именно в L23. SQL Server 2025 native vectors/RAG (L79), Azure HorizonDB **public preview** и AI для PostgreSQL (L89), Cosmos vector/hybrid search (L93) переданы без расширения GA/availability. Не придумана должность автора, отсутствующая в snapshot. **Definite factual errors не найдены.** Verifier заменил предположительное определение PeerSpot platform на строгое описание наград и уточнил native vector support; это не новый результат исследования.

**Style/selection:** исходник и TL;DR корректно названы, RAG локально раскрыт, центральные сущности bold. Но TL;DR состоит из трёх предложений и снова содержит длинные inline перечни четырёх продуктов и пяти качеств, несмотря на требование 3+ параллельных items → список. Reference почти весь посвящён четырём техническим направлениям; output тратит около половины объёма на награды/абстрактные качества, а технические различия сжимает в одну строку. Убраны auto-tuning/Hyperscale, Oracle migration, Cosmos failover/change feed — допустимая фактическая компрессия, но явно более слабое совпадение авторского отбора. Объём 104 немного выше ориентира 40–90 для маркетингового обзора; это редакторское замечание, не провал по формальному лимиту.


### 602 — rollout и типы ботов сохранены, Trace исчез при отборе

Run **d82d58d9-a16a-4ec1-91e6-8e360a8441d3**, `final-03-602-candidate-04-review`; stop, complete JSON, no flags/quotes, **182 body words / 10 bold fragments**, review cost **$0.027745696**. Snapshot совпадает с сохранённым full source.

Проверены continuous client-side Precursor (L34–38), mid-session/human-agent switches (L50–52), Risk/Trust distinction (L11–13), BotBase включая ненадёжных/unverified (L26–30), JS через CDN/cost для имитации (L34–40). **206 млн** привязаны именно к **оценкам Precursor за 24 h**, не пользователям/blocked bots (L44); число zones опущено без изменения знаменателя. Adaptive Intelligence **готовится**, не объявлен GA (L61–69); advanced mitigations **анонсированы**: random responses, Labyrinth и очереди для good agents (L79–91). «Кратковременная вероятность» несколько категоричнее source `often ephemeral`, но не установлено существенного нового quantifier claim о безопасности. **Definite factual error не найден.**

Verifier исправил draft `Поэтому Cloudflare готовит Adaptive Intelligence` на `Кроме того`, убрав неустановленную причинность. Заголовок списка `меры против вредоносных ботов` стал `продвинутые меры для ботов`: очередь для легитимных агентов больше не помещена под ошибочную общую категорию. Уточнён статус `unverified`. Определение бизнеса Cloudflare удалено как не данное в snapshot; обычный источник остался.

**Style/coverage:** source/TL;DR/bold есть, Risk/Trust и BotBase объяснены, три mitigation items — список. Сохраняется центральная полезная мысль continuous assessment; меньше каталога, чем в reference. Но reference особенно разъяснял Precursor vs **Precursor Trace**, а demo полностью потеряна. CDN остаётся голым в отдельном абзаце. Один длинный абзац объединяет Risk/Trust/BotBase/Precursor, и TL;DR → абзац → статистический абзац трижды повторяют continuous/mid-session мысль. Это редакторские недостатки, не factual failure.

### 1286 — главный механизм и результат верны; последовательность снова стрелками

Run **df97894d-d67e-4eb8-a3ee-65fe337df100**, `final-03-1286-candidate-04-review`; stop, complete JSON, no flags/quotes, **209 body words / 11 bold fragments**, review cost **$0.019606576**. Snapshot exact-match.

Источник/имя Wren Dougherty (L3), harness definition framing→decisions и reproducibility (L7), preparation support assistant/rare cases (L11), месяцы ручной работы и nearly-weekly переносы в новые языки/географии/LLM products (L13–15), shared methods (L19), agent roles chat (L21), человеческое внимание неоднозначным данным (L25), months→days (L23), nontechnical users > technical после года (L29) и maintenance agents (L35) подтверждены. **Definite factual error не найден.** 100 000 из мысленного примера не превращены в реальный объём внедрения; dozens teams/hundreds investigation types опущены без подмены метрик.

Verifier исправил draft `еженедельные запуски` → **почти еженедельное повторение одного исследования** и `пользователей больше, чем дата-сайентистов` → **больше, чем пользователей с техническими ролями**: второе важно, потому что исходник сравнивает broader roles, не конкретную профессию. «Отдельные агентные системы поддерживают» не утверждает отсутствия людей/полностью автономного deployment; source L35 says `help us`, это условие не следует считать отвергнутым.

**Style:** author English original, TL;DR/ПРОБЛЕМА/РЕШЕНИЕ/РЕЗУЛЬТАТ, понятный harness и результаты сохранены. Текст короче 303-word reference, но удерживает его основную методологическую мысль, не выдаёт личное мнение. Методы сведены в bullets. Однако сам последовательный core process остался **«извлечение → векторное представление → кластеризация»**, вопреки требованию numbered steps, а `contrastive labeling — контрастная разметка` только переводит название, не объясняет работу. LLM в самостоятельном абзаце не раскрыт/не заменён понятным названием. В последнем предложении maintenance три задачи снова inline. Factual pass не снимает эти style mismatches.


### 838 — отдельный эксперимент и research-only граница сохранены

Run **de2a1762-f1c3-4744-996e-a8cc308889de**, `final-03-838-candidate-04-review`; stop, complete JSON, no flags/quotes, **228 body words / 10 bold fragments**, review cost **$0.023297296**. Snapshot точь-в-точь совпадает с full UTF-8 source; исходный узкий пробел в title сохранён.

**Фактическая проверка:** CARE-X сочетает free text и structured auxiliary predictions (L49–81), RL DAPO с task-specific clinical rewards (L79–81). SigLIP2-so400M, Phi-4-mini-instruct3.8B и adapter, co-training и shared representations (L77/85) верны. Dual inference/adjustable threshold (L73/97) и генеративная grounding parity с detection head (L93) не превращены в превосходство на любых задачах. **94%** названо overall accuracy именно **ReXVQA** (L121), не клинической точностью на произвольных пациентах.

**Отдельно от CARE-X** указан Qwen3-VL-4B-Instruct measurement pipeline (L127–131); **43,6 п.п. среднего прироста F1**, не относительные проценты (L135–149). Кардиомегалия/средостение/аортальные находки покрывают пять исходных условий, хотя число пять опущено. **94,26% recall** привязан к CT-confirmed retrospective Narayana Health data (L175–177), и прямо оговорено отсутствие негативной когорты/одномерность recall (L185–187). 122 cases и +10.65pp baseline опущены, не заменены другим знаменателем. Теоретический пример 100% recall при маркировке всех положительными взят из L187 и не выдан за реальный benchmark. Не clinical device/use (L1/195) сохранено. **Definite factual defect не найден.**

Verifier сделал точнее «средний F1 вырос» → «средний прирост F1 составил», связал CT-confirmed и Narayana в один cohort. Он не исправлял смешение CARE-X/Qwen: правильное разделение было уже в draft. Отсутствие этой ошибки нельзя приписывать только второй стадии.

**Style:** заголовок/source/TL;DR корректны, название основной модели объяснено, research limitation разумно оставлена. Output подробнее reference (137 полных слов), со сложной новой ML-идеей это допустимый режим, однако плотность терминов явно выше. VLM введён в TL;DR, затем отдельно голый; DAPO объяснён как RL-algorithm, но позднее без локального пояснения; **CT, F1** нигде не раскрыты, `overall accuracy`, `detection head`, `auxiliary heads` перемешаны с русским. Уточнить функции можно было без выдуманной полной расшифровки. Narayana Health впервые не bold, хотя это центральный источник clinical data (draft был bold). Два длинных абзаца содержат механизмы и результаты вперемешку; список позволил бы яснее отделить модель от measurement experiment. Точность выше старого reference по разделению экспериментов, но стилевую неразличимость это не доказывает.


### 9 — точный scope EMA, но тяжёлые термины и потеря определения MCP

Run **a5bb4f1b-2d09-4964-9905-9128a6c33bb1**, `final-03-9-candidate-04-review`; stop, complete JSON, no flags/quotes, **196 body words / 8 bold fragments**, review cost **$0.025393456**. Snapshot exact-match.

Источник InfoQ/author Matt Saunders (L27), stable extension/zero-touch login (L1–3), previous per-user/per-server OAuth and onboarding/policy/personal-work confusion (L13–15), ID-JAG token exchange auth server (L5), **connection-level control, not per-action runtime authorization** (L7/15), adoption by named organisations (L3), Okta first named identity provider/Cross App Access first supported path and two-sided support requirement/fallback (L15) верны. **Definite factual errors не найдены.** Reference упрощает до одного токена для scope серверов; output это не воспроизвёл. Также не приписана организационная принадлежность MCP-team Anthropic, которой snapshot не доказывает.

Verifier заменил «реализовали поддержку» на более точное **приняли/adopted**, уточнил статус Okta/Cross App Access; не делает из first-mentioned provider единственного навсегда. Удалил функциональное определение MCP из draft как неявное для snapshot. Exact full forms MCP/EMA/ID-JAG присутствуют в source и сохранены; JWT внутри длинного expansion source не раскрыт, поэтому его нельзя расшифровывать наугад.

**Style:** source/type/author-original/TL;DR есть; security distinction полезнее короткого reference и оправдывает некоторую дополнительную длину. Но 196 vs70 полных слов заметно меняет характер короткой заметки: подробности onboarding, ID-JAG и catalogue adoption можно было отобрать строже. **MCP теперь только имя**, без назначения протокола; EMA только английское полное название + функция, а следующий самостоятельный абзац содержит MCP/OAuth/JWT, последний EMA/MCP без локального понятного раскрытия. Identity provider, consent, scope, fallback остаются инженерным jargon без краткого пояснения. Наименование InfoQ не bold. Группа трёх adopters опять inline вместо списка. Эти style gaps не считаются выполненными лишь потому, что expansions технически присутствуют где-то выше.


### 442 — утверждения не ложны, но HCL раскрыт вне разрешённого основания

Run **c2ee7bb6-e95e-41b2-9a38-8dea01c18eca**, `final-03-442-candidate-04-review`; stop, complete JSON, no flags/quotes, **166 body words / 11 bold fragments**, review cost **$0.012239656**. Snapshot exact-match.

InfoQ/author Sergio De Simone (L25), HashiCorp/tfpolicy/HCP и public beta (L1), policy-as-code/HCL familiarity (L3/9), before/after and actual-state validation (L11/15), relationships/external data source/download controls (L13), HCP full capability vs standalone CLI primarily local validation/testing (L19), Sentinel custom DSL and Open Policy Agent role (L21) переданы без новой подтверждённой ложной числовой/причинной связи. Verifier исправил `запретить непроверенные зависимости` на **предотвратить случайное использование неодобренных**, что соответствует L13; `CLI ограничен` на **в основном поддерживает**, убрав необоснованную абсолютность; нормализовал planning/deployment/post-deployment scope.

**Подтверждённый source-grounding gap:** output добавляет **HCL (HashiCorp Configuration Language)**, но полного имени нет ни в полном article snapshot, ни в frozen verified glossary. Это известное корректное раскрытие, а не установленная ложь; однако правило «если точной расшифровки нет в article/проверенном контексте, не угадывай» явно не выполнено. Verifier оставил это из draft. Не следует смешивать отсутствие ложного факта с полным исполнением ограниченного source-only контракта.

**Caution:** Sentinel назван «прежним», а source L17 явно говорит о продолжающейся поддержке. Output не заявляет снятие с поддержки, поэтому это не definite falsehood, но важная reference-оговорка потеряна и слово провоцирует неверное впечатление. Нельзя превращать эту оценку в утверждение, что модель прямо объявила Sentinel discontinued.

**Style:** хороший трёхпунктный список, source/type/person-original и бизнес HashiCorp, назначение Terraform/tfpolicy присутствуют; текст короче reference253 без полного каталога. Но HashiCorp не bold, HCL в новом абзаце уже без локального раскрытия, CLI голый, HCP описан как облачный сервис без полного имени (которого snapshot не даёт). DSL функционально переведён, однако точное английское `domain-specific language` из source не сохранено. Полезное объяснение data source было вырезано verifier, осталось неочевидное имя механизма. Такой trade-off source conservatism/понятность нужно показать автору, не объявлять достигнутой неразличимостью.

## Сводка и воспроизводимость

| ID | Draft → review body words | Review result |
|---|---:|---|
| 916 | 221 → 227 | Роли/права/code boundary верны; повторения и голые MCP/PR |
| 409 | 89 → 104 | Preview/awards/status верны; награды вместо технического отбора, inline lists |
| 602 | 184 → 182 | Исправлены causal link и категория bot mitigations; отсутствует Trace, CDN jargon |
| 1286 | 195 → 209 | Исправлены cadence и population comparison; pipeline стрелками, слабое labeling definition |
| 838 | 231 → 228 | Отдельные эксперименты/pp/recall/research-only верны; плотные термины, CT/F1 |
| 9 | 197 → 196 | Connection vs per-action и token flow верны; MCP function удалена, длинный auth jargon |
| 442 | 166 → 166 | Dependency/CLI scope уточнены; source-grounding gap HCL, Sentinel continuation omission |

Все review calls — Pro/low/16 384, finish `stop`; все final quote segments отсутствуют, поэтому генерация точных цитат этим набором **не проверена**. Вычисления `body_words` сделаны непосредственно по `response.json` через `evaluate_outputs.evaluate`; для draft передан exact `draft_summary` того же request. Никаких stale rendered.md, иных labels или «подправленных» responses при оценке не использовалось.

**Расходы из settled ledger:** семь review calls **$0.158537632**, их семь draft + семь review вместе **$0.303773712**. Это только эта группа, не бюджет всего исследования и не дополнительный расход аудита.

Идентификаторы draft для восстановления пар:

- 916: `dd3aa586-4ced-4b61-90b6-b1a991cb1a22`
- 409: `545a1a64-3a9f-4661-9e2c-52ad772662a1`
- 602: `5cd133a2-ec7e-4edc-b1c2-e34ea8f24cbd`
- 1286: `c25fea9b-54dc-4634-a258-30ea336f3a7b`
- 838: `2ae34f29-b6be-4c79-a86f-f1bd7e1c4719`
- 9: `a761e07f-ab96-4e66-840b-e2d9376d37de`
- 442: `6d0c7aa3-e51d-4727-aeed-7396df811e62`

**Вывод в границах наблюдения:** на этих семи новых источниках вторая стадия исправляет некоторые смысловые детали, не внося найденных существенных ложных утверждений; она одновременно иногда удаляет полезные определения или расширяет текст. 7 примеров недостаточно для оценки частоты ошибок на всех статьях. Source-only исполнение не полное (#442), авторский стиль не принят этим аудитом. Никакие результаты/refs этой группы не использованы для последующей настройки в рамках данного задания.
