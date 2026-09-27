# Третья свежая проверка: исходники до открытия авторских эталонов

Сбор завершён **2026-09-26 22:52 UTC**. Подготовлено **20 полных первоисточников**: все 19 оставшихся article-like групп исходного final split и одна восстановленная ранее недоступная статья. Новые авторские reference bodies при подборе, извлечении и аудите не открывались; платные модельные вызовы не выполнялись. Это проверка идентичности источника по заголовку и URL, а не подтверждение смыслового соответствия авторскому пересказу.

## Зафиксированный выбор и восстановление

До получения страниц записан `.inoreader-state/style-research/final-03-selection.json` (22:41:56 UTC). Сохранён относительный порядок оставшихся групп из `partitions.json/selection_order.final`. #927 исключён заранее по указанию исследователя: его ссылка ведёт на профиль организации Harperfast в GitHub, а не на подтверждённую статью. Из иных split новые примеры не добавлялись.

Исходный пул уже не мог поддержать прежний равномерный обход шести strata. Итоговые 20: September-long 6, August-long 5, September-short 4, July-short 4, July-long 1; August-short отсутствует. Выборка не объявляется сбалансированной или репрезентативной для всего канала.

Для двадцатой пары до повторных запросов записан `final-03-recovery-selection.json`: прежние retrieval failures cohort02 в исходном порядке. Основания допустимой свежести — `decisions-02.json` с `author_reference_bodies_opened=false`, прежний [отчёт получения](cohort-02-sources.md), перечни прочитанных успешных 20 в трёх paired-audit отчётах, подтверждённая история чтения root/collector и отсутствие всех 17 recovery-ID в model labels всех 209 settled experiments. Это документированные свидетельства, а не доказательство абсолютного отсутствия любой экспозиции. Источники уже искались раньше; «нетронутый retrieval» для recovery не заявляется.

Повторно обработаны последовательно:

| ID | Результат |
|---|---|
| 621 | Direct снова HTTP 403; Jina возвращает `Just a moment` challenge. Полного текста нет. |
| 1470 | Повторный поиск характерного заголовка находит общий отчёт Kunpeng, но не точную первичную статью. URL не угадан. |
| 1316 | Для редакционного заголовка `Streamhouse - state on 15.sep.2026` нет точного источника или явной ссылки; пара не выдумана. |
| 126 | Найдена полная статья на официальном `airbnb.tech` с точным заголовком. Ранее явная Medium-ссылка была недоступна. Обе URL и история попыток сохранены; новое имя источника не подставлено молча. После двадцатой готовой пары recovery остановлен. |

## Готовые источники

Порядок ниже является порядком готовых записей в `pairs/final-03.jsonl`; неудачи 621/1470/1316 также присутствуют в этом JSONL. Число символов — полный сохранённый текст без единственного служебного terminal newline.

| ID | Первичный заголовок | Символов | Получение |
|---|---|---:|---|
| 1304 | Writing Parquet That VertiPaq Likes | 9 554 | direct |
| 610 | Uno Platform 6.6 Adds Native AOT, Vulkan Rendering, and Broader Accessibility Support | 3 890 | Jina |
| 1229 | Netflix Conductor : The Next Chapter | 30 720 | Jina |
| 1215 | How Sony LIV built real-time video streaming analytics with AWS | 8 938 | direct |
| 286 | Client-Side Load Balancing at a Million Requests Per Second | 37 220 | direct |
| 651 | Comprehension as an Architectural Characteristic: A System That Is Not Understood Cannot Evolve Safely | 17 879 | Jina |
| 998 | Continuous Delivery for Foundational Platforms | 48 127 | Jina |
| 916 | Root Cause is Half the Job; Remediation Agent Ensures Every Analysis Ends in a Plan | 4 789 | direct |
| 409 | What customers value most in Microsoft Databases—from reliability to AI readiness | 9 725 | direct |
| 602 | Unveiling good and bad behaviors on the Agentic Internet | 12 253 | direct |
| 1286 | Beyond the model: Engineering AI infra with scientific judgement | 7 016 | direct |
| 838 | Introducing CARE-X: Towards Clinically Useful Radiology VLMs with Auxiliary Supervision, Reward-Aligned Learning, and Tool-Augmented Measurement | 19 784 | direct |
| 9 | AI Model Context Protocol Adds Centralised Auth for Enterprise | 4 729 | Jina |
| 442 | Terraform Introduces tfpolicy, an HCL-based Policy-as-Code Framework | 3 725 | Jina |
| 1170 | How to Stop Prompt Drift From Wrecking Your AI Outputs | 7 580 | direct |
| 1021 | Specification-driven composition for flexible data workflows | 14 382 | direct |
| 273 | Enabling Data Intelligence: Data Profiling Framework at Halodoc | 19 070 | direct |
| 747 | How AI is transforming analytics at Grab | 16 043 | direct |
| 901 | Powering agentic AI with real-time streaming data on AWS | 16 649 | direct |
| 126 | From weeks to a day: how we made LLM evaluation fast enough to iterate on | 14 901 | direct |

Итого **306 974 символа основной выборки**, **306 994 символа в точных `.txt` файлах** с 20 terminal newlines. Все 20 имеют точное совпадение заголовка с h1/OG на первичном издателе. Для входного заголовка выбран точный видимый h1, если он единственный; брендинговый OG не заменяет заголовок. В JSONL сохранены без изменений NBSP в #1304 и narrow NBSP в #838; таблица выше служит читаемой навигацией, не источником prompt-input.

## Границы, кодировка и ограничения

- У Zalando #286 общий `main` включал рекомендуемые статьи. Новая производная выбирает единственную статью `content-post` с `blogpost`, включая её заголовок, вводный абзац, byline и весь основной текст; чужие карточки исключены.
- У Halodoc #273 `main` тоже захватывал рекомендации. Выбран полный `js-post-content`; заключение, сведения о компании и tags остаются, recommended articles вне контейнера исключены.
- У Microsoft #838 HTTP явно объявляет UTF-8, но HTML не содержит charset; первоначальное byte-parsing давало mojibake. Новая производная декодирует исходные bytes строгим UTF-8 согласно сохранённому HTTP header. Исходный HTML, прежняя неудачно декодированная производная и новые файлы сохранены отдельно. Потерь/замены символов нет.
- #998 — полный опубликованный transcript и Q&A; видео независимо не расшифровывалось. Другие тексты также получены из опубликованного article/main-body, не из поисковых snippets.
- Все начала и окончания контейнеров проверены. Несколько издательских footer-блоков — авторы, legal, hiring, CTA, share/tags — оставлены и явно описаны в `boundary_review`; они не выдаются за новый смысл статьи.
- Изображения, формулы и графики сохранены в raw HTML/selected HTML, но OCR пикселей не выполнялся. Оценка не должна предполагать факты, доступные только на диаграмме.
- Это современные снимки живых страниц, не архивы состояния на дату Telegram-поста. Title-match не доказывает неизменность содержания или точное соответствие будущему открытому авторскому пересказу.
- 14 источников получены напрямую, 6 — через публичный Jina HTML transport. Для proxy сохранены исходный requested URL, transport/effective URL, HTTP headers, время и representation. Proxy-снимки нельзя называть оригинальными wire bytes издателя.

## Артефакты и воспроизводимость

Все сырые данные находятся в игнорируемом `.inoreader-state/style-research/`:

- `pairs/final-03.jsonl` — 20 готовых и 3 недоступных записи; точный title/URL, source metadata/text/raw paths, selector, evidence, boundaries, limitations.
- `decisions-03.json` — решения, #927 exclusion, **31** сохранённая fetch/derivative запись: 24 extraction success (включая промежуточные производные), 6 fetch failures, 1 extraction failure. Это не 31 готовая статья.
- `sources/final03-*` — immutable raw HTML, headers, selected HTML, полный текст и metadata; предыдущие `final02-*` не изменены.
- `search-final03-*` — сохранённые результаты поиска/переходов; `final03-fetch-*.json` — использованные batch URL. Поисковый текст служит доказательством обнаружения, не модельным article input.
- `assemble-final03.py` — точная metadata-only сборка/валидация manifest. Для повторного запуска нужен отдельный каталог состояния: скрипт намеренно отказывается перезаписывать готовые пары/решения.

Перед сохранением проверены 20 text-length/metadata совпадений, наличие точных title matches, отсутствие known group/title/URL overlap с `development`, `validation`, `final` и `final-02`. При сравнении URL использованы только сравнимые host/path (www, trailing slash, AWS `/ru/blogs` alias); записанные URL не переписаны. Тематическое сходство и неизвестные aliases этим не исключаются. #126 явно проверен как другой первичный publisher URL той же ранее недоступной публикации.

Ни prompt, ни production-код, ни бюджет, ни прежние результаты/ответы не изменены. Semantic reference/source alignment и factual evaluation должны выполняться только после freeze нового кандидата.
