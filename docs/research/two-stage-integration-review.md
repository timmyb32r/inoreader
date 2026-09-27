# Два запроса: независимая проверка интеграции

Проверены текущие `reader-ai/service/worker.rs`, `model.rs`, `config.rs`, `service.rs`, `provider.rs`, `reader-storage-postgres/ai.rs`, `ai/calls.rs` и соответствующие frontend DTO/контроллер/виджет. Backend проверялся чтением; Cargo, provider calls и deploy этот аудит не запускал. Backend в это время дополнял тесты, поэтому это проверка конкретных контрактов и сценариев, а не замена общего release gate.

## Подтверждённый P2, исправлен

**После сбоя первой summary UI предлагал Send, который сервер всегда отклонял.**

`PostgresAiStore::append` разрешает `OperationKind::Message` только при наличии `purpose=Summary` и `status=Complete`. Исходные условия `ArticleChatWidget.canSend` и `useArticleChat.send` проверяли отсутствие активной работы, наличие профиля и текста, но не завершённую проверку. Воспроизведение: первая summary → verification failed/cancelled → написать вопрос → Send был активен → сервер возвращал 409.

Исправление использует общий `hasVerifiedSummary`: Send, submit и Ctrl/⌘+Enter недоступны до готовой проверенной summary. Редактор остаётся доступен, черновик сохраняется; в уже зарезервированной строке композитора объяснено условие отправки. Никакого изменения backend policy, автоматического Retry или дополнительного provider call не добавлено.

Регрессии проверяют disabled Send/keyboard после failed/cancelled summary, отсутствие POST `/messages`, сохранение текста через Retry/Stop и неизменность координат композитора и кнопок. Предыдущая verified summary продолжает разрешать диалог после завершения последующей попытки ответа.

## Других подтверждённых P0/P1/P2 в прочитанном backend не найдено

Проверенные механизмы:

- **Черновик и публикация.** `AttemptTask::Summary` хранит исходный envelope во внутренней записи; `StoredProgress` не пишет generated draft в сообщения. `update_claim` запрещает content до сохранённого draft, а `Completed` требует завершённого verification call. Промежуточный текст verifier имеет незавершённый статус и скрыт UI вместе с Copy; failed/interrupted сообщения не попадают в последующий model context. Ответы обычного follow-up продолжают стримиться.
- **Фазы и повтор.** `calls::begin` проверяет phase текущего task, assistant identity и отсутствие уже зарегистрированного вызова этой фазы. Сохранённый draft переводит task в verification. Retry копирует task: завершённая генерация повторно не оплачивается, проверка получает тот же draft и article snapshot; failed generation остаётся generation retry. Follow-up — `Reply`, один вызов. UI не запускает Retry автоматически при сетевой неопределённости; повтор неполученного acknowledgement сохраняет original operation ID.
- **Отмена и поздний ответ.** Публикация ограждена lease/status/account/workspace проверками. Stop и удаление ключа прерывают активную попытку; старый worker не может опубликовать ответ в новую попытку. `update_call` отдельно допускает позднюю известную usage старого assistant ID, не возобновляя job. Неизвестный результат не превращается в нулевой расход.
- **Стоимость.** Каждая фаза и повтор имеют отдельный call ID. Usage сохраняется до финального parser success, поэтому известные расходы остаются при последующей ошибке ответа. UI показывает все calls текущей версии, сумму известных decimal estimates без float/округления и отдельное число неизвестных расходов. Обрыв до получения provider usage остаётся явно unknown.
- **Фиксация входа.** Запись хранит полный snapshot, оба prompt текста, модели, режимы, output limits и rates; проверки реального input/context идут перед каждым платным вызовом. До первого вызова дополнительно проверяется, что исходник помещается в verification input. После появления draft проверяется его полный фактический размер, без усечения. Источник закрепляется один раз; проверка получает тот же snapshot. Lease configuration учитывает два request deadlines.
- **Изоляция.** Чтение и блокировка записей проверяют владельца и workspace; call update ищет одновременно call ID и assistant ID внутри принадлежащей владельцу записи. Равенство URL не объединяет истории. Просмотр сохранённой беседы не требует нового ключа и не инициирует provider request.

Не делаю вывода, что второй LLM-запрос гарантирует фактическую истинность текста: эта проверка устанавливает исполнение двух стадий и контракт публикации, а качество самого verifier требует отдельного исследования root.

## Проверки frontend

До последнего P2: `npm run typecheck` — passed; `npm test` — **20 файлов, 127 тестов**, 5,81 с; focused Chromium — **7/7**, 5,1 с.

После P2 на итоговом UI:

- `npm test -- src/ai/ArticleChatWidget.test.tsx src/ai/ArticleChatVerification.test.tsx` — **18/18**, 1,15 с.
- `npm run test:e2e -- e2e/deepseek.spec.ts` — **7/7 Chromium**, 5,2 с. Команда также выполнила `tsc --noEmit` и Vite build в webServer startup.

Проверено сохранение координат reader toolbar/Stop/Retry/Send/editor при generation → verification → complete и verification failure → retry → stop; также старые drag, collapse, tiny viewport, profile masking, profile unmount и uncertain acknowledgement. Общий Rust/Docker/browser release gate выполняет backend/root после окончательной интеграции; этот документ не утверждает, что он уже завершён.
