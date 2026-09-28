# Импорт источников Transferia — 28 сентября 2026

Исходный файл: `~/Downloads/transferia-missing-subscriptions-2026-09-27.md`. Разрешённый объём: основные источники, видеоканалы и архивы; без активных дублей.

## Результат

- Рассмотрены 300 позиций: 213 основных, 61 видеоканал, 11 архивных и 15 DataFun.
- Добавлено 237 активных подписок; активных подписок стало 448 вместо 211.
- Одна позиция использует уже добавленный тот же канал EMQX/eKuiper.
- 62 позиции не подключены: причины приведены ниже. Это не утверждение, что все они недоступны вообще; часть требует отдельного адаптера или авторизации.
- На момент последней проверки новые активные подписки содержат 7960 статей; 188 источников уже непустые. Первичная загрузка остальных продолжается.

Добавление выполнено тремя атомарными импортами (113 + 103 + 21). Временная пустая HTML-подписка Cyclr заменена рабочей RSS-подпиской; старая запись архивирована, данные статей не удалялись. Существующие пользовательские подписки не удалялись.

Для HTML-источников проверены заголовки в предварительном просмотре и результат фонового сбора. Уточнены правила, исключающие повторные витрины. DataChannel подключён к основной вкладке последних статей (12 на момент проверки), Domo — к основным последним публикациям (3); полный исторический импорт всех страниц сайтов не выполнялся. Portable использует явный XPath-выбор одной непустой ссылки на URL. В Rclone/Ora2Pg подключены официальные GitHub-ленты релизов, ссылки на репозитории подтверждены исходными страницами.

## Не подключены

| Источник | Причина |
|---|---|
| [Yandex Cloud — Telegram](https://t.me/yandexcloudnews) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [Keboola — newsletter](https://www.keboola.com/newsletter) | Почтовая рассылка: форма регистрации, открытая лента выпусков не найдена. |
| [Fivetran — changelog](https://fivetran.com/docs/changelog) | Проверка сервера: remote server returned HTTP 406 |
| [Hevo — Data Builders newsletter](https://hevodata.com/data-builders-newsletter/) | Почтовая рассылка: форма регистрации, открытая лента выпусков не найдена. |
| [Pentaho — блог](https://pentaho.com/insights/blogs/) | Проверка сервера: remote server returned HTTP 403 |
| [Ab Initio — LinkedIn](https://www.linkedin.com/company/ab-initio/) | LinkedIn: открытая страница компании не предоставляет подтверждённую ленту постов. |
| [SAP Community — Technology Blogs](https://community.sap.com/t5/technology-blogs-by-sap/bg-p/technology-blog-sap) | Проверка сервера: remote server returned HTTP 403 |
| [SAS Data Management — блог](https://blogs.sas.com/content/datamanagement/) | Проверка сервера: outbound request was rejected: transport failed: request_timeout |
| [Alteryx — блог](https://www.alteryx.com/blog) | Проверка сервера: outbound request was rejected: transport failed: request_timeout |
| [FME / Safe Software — блог](https://fme.safe.com/blog/) | Проверка сервера: outbound request was rejected: transport failed: request_timeout |
| [Qlik — Blog](https://www.qlik.com/blog) | Проверка сервера: remote server returned HTTP 404 |
| [Quest — блог, включая SharePlex](https://blog.quest.com/) | Проверка сервера: remote server returned HTTP 403 |
| [Syniti — блог](https://blog.syniti.com/) | Тайм-аут получения данных; рабочая лента при проверке не подтверждена. |
| [Redis — блог](https://redis.io/blog/) | Проверка сервера: remote server returned HTTP 403 |
| [MongoDB — блог](https://www.mongodb.com/company/blog) | Проверка сервера: remote server returned HTTP 403 |
| [StreamNative — Medium](https://medium.com/streamnative) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [DeltaStream — блог](https://www.deltastream.io/blogs/) | Проверка сервера: outbound request was rejected: transport failed: request_timeout |
| [Timeplus — блог](https://www.timeplus.com/blog) | Тайм-аут получения данных; рабочая лента при проверке не подтверждена. |
| [HiveMQ — блог](https://www.hivemq.com/blog/) | Проверка сервера: remote server returned HTTP 404 |
| [Solace — Medium](https://medium.com/@solacedotcom) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [Bindplane — блог](https://bindplane.com/blog) | Проверка сервера: remote server returned HTTP 403 |
| [Chronosphere — блог](https://chronosphere.io/blog-hub/) | Проверка сервера: outbound request was rejected: transport failed: request_timeout |
| [Syslog-ng — community blog](https://www.syslog-ng.com/community/b/blog) | Проверка сервера: remote server returned HTTP 403 |
| [Adobe Experience Cloud — блог](https://business.adobe.com/blog/) | Тайм-аут получения данных; рабочая лента при проверке не подтверждена. |
| [SnapLogic — блог](https://www.snaplogic.com/blog) | Проверка сервера: outbound request was rejected: transport failed: request_timeout |
| [TIBCO — блог](https://www.tibco.com/blogs) | Проверка сервера: remote server returned HTTP 403 |
| [Linx — блог](https://linx.software/blog/) | Тайм-аут получения данных; рабочая лента при проверке не подтверждена. |
| [Elastic.io — блог](https://www.elastic.io/blog/) | Проверка сервера: outbound request was rejected: transport failed: request_timeout |
| [Make — блог](https://www.make.com/en/blog) | Проверка сервера: remote server returned HTTP 403 |
| [Zapier — Engineering](https://zapier.com/blog/categories/engineering-insights/) | Сайт возвращает страницу US Sanctions вместо статей. |
| [Zapier — блог](https://zapier.com/blog/) | Сайт возвращает страницу US Sanctions вместо статей. |
| [MuleSoft — Technically Speaking newsletter](https://www.linkedin.com/newsletters/technically-speaking-7140068811264651264/) | LinkedIn newsletter: сервер не получил доступ к публичному архиву. |
| [Albato — Medium](https://medium.com/albato-platform) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [Arenadata — Telegram](https://t.me/arenadata) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [Loginom — Telegram](https://t.me/loginom) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [DATAREON — Telegram](https://t.me/appintegtation) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [Digital Q — Telegram](https://t.me/DiasoftTechno_Channel) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [Neoflex — Telegram](https://t.me/neoflexlive) | Проверка сервера: outbound request was rejected: transport failed: connect |
| [Rsync — новости](https://rsync.samba.org/) | Новости доступны в общей HTML-странице; надёжная лента отдельных записей не настроена. |
| [MySQL — блог](https://blogs.oracle.com/mysql/) | Проверка сервера: remote server returned HTTP 403 |
| [Ray / Anyscale — блог](https://www.anyscale.com/blog) | Динамическая страница: в полученной разметке нет подтверждённой ленты статей. |
| [Funnel — YouTube](https://www.youtube.com/@WeAreFunnel) | Тайм-аут получения данных; рабочая лента при проверке не подтверждена. |
| [Talend Open Studio — статус](https://www.qlik.com/us/blog/talend/a-new-chapter-for-talend-open-studio) | Получение страницы: HTTP Error 404: Not Found |
| [Apache Sqoop — Attic](https://attic.apache.org/projects/sqoop.html) | Статическая страница Apache Attic о закрытии проекта; это не лента публикаций. |
| [Apache Apex — Attic](https://attic.apache.org/projects/apex.html) | Статическая страница Apache Attic о закрытии проекта; это не лента публикаций. |
| [Bytewax — блог](https://bytewax.io/blog) | Получение страницы: <urlopen error [Errno 8] nodename nor servname provided, or not known> |
| [pg_flo — блог](https://www.pgflo.io/blog) | Получение страницы: <urlopen error [Errno 8] nodename nor servname provided, or not known> |
| [Equalum — блог](https://www.equalum.io/blog) | Получение страницы: <urlopen error [Errno 8] nodename nor servname provided, or not known> |
| [Arcion — блог](https://www.arcion.io/blog) | Получение страницы: <urlopen error [SSL: SSLV3_ALERT_HANDSHAKE_FAILURE] ssl/tls alert handshake failure (_ssl.c:1082)> |
| [DataFun — официальный сайт](https://www.datafuntalk.com/) | Динамический сайт XiaoeTech; браузерная проверка завершилась тайм-аутом. |
| WeChat Official Account / 微信公众号: DataFunTalk | WeChat: нет публичного URL ленты; нужен доступ через приложение/аккаунт или ручное вступление. |
| WeChat Official Account / 微信公众号: DataFunSummit | WeChat: нет публичного URL ленты; нужен доступ через приложение/аккаунт или ручное вступление. |
| WeChat Official Account / 微信公众号: 大话数智 | WeChat: нет публичного URL ленты; нужен доступ через приложение/аккаунт или ручное вступление. |
| WeChat Channels / 微信视频号: DataFunTalk | WeChat: нет публичного URL ленты; нужен доступ через приложение/аккаунт или ручное вступление. |
| Тематические и событийные группы WeChat / 微信群 | WeChat: нет публичного URL ленты; нужен доступ через приложение/аккаунт или ручное вступление. |
| [DataFunTalk — организатор на 活动行 / Huodongxing](https://www.huodongxing.com/org/570862189921) | Динамическая страница мероприятий: рабочее правило извлечения не подтверждено. |
| [DataFunTalk — 今日头条 / Toutiao](https://www.toutiao.com/c/user/token/MS4wLjABAAAAkib_WRHS65C7InolW9ERkXdPG6tMQEVnaNeF_3P3gyjZzZ45a-t89upXiao_ZCtG/) | Динамическая страница Toutiao: рабочее правило извлечения не подтверждено. |
| [DataFunTalk — 掘金 / Juejin](https://juejin.cn/user/2295436009547911/posts) | Динамическая страница Juejin: рабочее правило извлечения не подтверждено. |
| [DataFunTalk — колонка Tencent Cloud Developer Community](https://cloud.tencent.com/developer/column/83291) | Динамическая колонка Tencent: рабочее правило извлечения не подтверждено. |
| [DataFun — SegmentFault / 思否](https://segmentfault.com/u/datafun/articles) | Получение страницы: HTTP Error 468:  |
| [DataFunTalk — 墨天轮 / Modb](https://www.modb.pro/u/310618) | Динамическая страница Modb: рабочее правило извлечения не подтверждено. |
| [DataFunTalk — 新浪看点 / Sina](https://k.sina.com.cn/mediaDocList.d.html?uid=2674405451) | Получена общая страница Sina вместо списка публикаций автора. |

## Все позиции исходного списка

| ID | Источник | Результат / подключённый URL |
|---|---|---|
| s001 | Transferia Go — релизы | Добавлен: https://github.com/transferia/transferia/releases.atom |
| s007 | Matillion — блог | Добавлен: https://www.matillion.com/resources/blog |
| s009 | Keboola — блог | Добавлен: https://www.keboola.com/blog |
| s012 | Boomi — блог | Добавлен: https://boomi.com/blog/ |
| s015 | Etleap — блог | Добавлен: https://etleap.com/blog |
| s016 | Portable — блог | Добавлен: https://portable.io/learn |
| s018 | Polytomic — блог | Добавлен: https://www.polytomic.com/blog |
| s020 | TROCCO — English blog | Добавлен: https://global.trocco.io/blog |
| s021 | DataChannel — блог | Добавлен: https://www.datachannel.co/blogs |
| s022 | Y42 — блог | Добавлен: https://www.y42.com/blog |
| s023 | CloudQuery — блог | Добавлен: https://www.cloudquery.io/blog |
| s245 | primeNumber / TROCCO — японский блог | Добавлен: https://primenumber.com/feed/ |
| s247 | Yandex Cloud — Telegram | Не подключён; причина выше |
| s248 | Keboola — Tech Blog | Добавлен: https://500.keboola.com/rss/ |
| s249 | Keboola — changelog | Добавлен: https://changelog.keboola.com/rss/ |
| s250 | Keboola — newsletter | Не подключён; причина выше |
| s251 | Fivetran — changelog | Не подключён; причина выше |
| s252 | Hevo — Data Builders newsletter | Не подключён; причина выше |
| s253 | Hevo — changelog | Добавлен: https://headwayapp.co/hevo-changelog/rss |
| s254 | Matillion — changelog | Добавлен: https://roadmap.matillion.com/rss/changelog.xml |
| s276 | primeNumber — Owned Media | Добавлен: https://lounge.primenumber.com/feed |
| s027 | Pentaho — блог | Не подключён; причина выше |
| s028 | CloverDX — блог | Добавлен: https://www.cloverdx.com/blog/rss.xml |
| s029 | Ab Initio — LinkedIn | Не подключён; причина выше |
| s030 | SAP Community — Technology Blogs | Не подключён; причина выше |
| s032 | Microsoft SSIS — блог | Добавлен: https://techcommunity.microsoft.com/category/sql-server/blog/ssis |
| s033 | SAS Data Management — блог | Не подключён; причина выше |
| s034 | Alteryx — блог | Не подключён; причина выше |
| s035 | FME / Safe Software — блог | Не подключён; причина выше |
| s036 | KNIME — блог | Добавлен: https://www.knime.com/blog/rss.xml |
| s037 | TimeXtender — блог | Добавлен: https://www.timextender.com/blog |
| s038 | WhereScape — блог | Добавлен: https://www.wherescape.com/feed/ |
| s039 | K2view — блог | Добавлен: https://www.k2view.com/blog/rss.xml |
| s255 | IBM StreamSets — community | Добавлен: https://community.ibm.com/community/user/groups/community-home?CommunityKey=971a803b-7763-4ee4-bef3-0194750c4201 |
| s277 | IBM DataStage — Global Data Lifecycle blogs | Добавлен: https://community.ibm.com/community/user/groups/community-home/recent-community-blogs?communitykey=73ce24ad-a006-4d3a-bc1e-7a509a0e2e7d |
| s281 | Qlik — Blog | Не подключён; причина выше |
| s043 | Alibaba DataX — релизы | Добавлен: https://github.com/alibaba/DataX/releases.atom |
| s044 | ChunJun — релизы | Добавлен: https://github.com/DTStack/chunjun/releases.atom |
| s045 | Apache Hop — блог | Добавлен: https://hop.apache.org/blog/index.xml |
| s049 | Bruin — блог | Добавлен: https://getbruin.com/blog/ |
| s050 | Mage — блог | Добавлен: https://www.mage.ai/blog |
| s051 | Embulk — статьи | Добавлен: https://www.embulk.org/articles/ |
| s052 | Singer SDK — релизы | Добавлен: https://github.com/meltano/sdk/releases.atom |
| s053 | Apache Gobblin — релизы | Добавлен: https://github.com/apache/gobblin/releases.atom |
| s058 | Quest — блог, включая SharePlex | Не подключён; причина выше |
| s059 | Syniti — блог | Не подключён; причина выше |
| s061 | PeerDB — блог | Добавлен: https://blog.peerdb.io/rss.xml |
| s062 | Sequin — блог | Добавлен: https://blog.sequinstream.com/rss/ |
| s064 | Artie — блог | Добавлен: https://www.artie.com/blogs |
| s066 | Adiom — блог | Добавлен: https://adiom.io/blog-feed.xml |
| s067 | Xata — блог | Добавлен: https://xata.io/blog |
| s068 | Alibaba Canal — релизы | Добавлен: https://github.com/alibaba/canal/releases.atom |
| s069 | Maxwell — релизы | Добавлен: https://github.com/zendesk/maxwell/releases.atom |
| s256 | Streamkap — Substack | Добавлен: https://streamkap.substack.com/feed |
| s071 | Google Cloud — Databases | Добавлен: https://cloud.google.com/blog/products/databases |
| s075 | Huawei Cloud — Developer blog | Добавлен: https://bbs.huaweicloud.com/blogs |
| s278 | Huawei Cloud — новости | Добавлен: https://www.huaweicloud.com/intl/en-us/news.html |
| s084 | SingleStore — блог | Добавлен: https://www.singlestore.com/blog/feed.xml |
| s085 | Redis — блог | Не подключён; причина выше |
| s086 | Apache Hudi — блог | Добавлен: https://hudi.apache.org/blog/rss.xml |
| s087 | Apache Paimon — релизы | Добавлен: https://github.com/apache/paimon/releases.atom |
| s089 | PostgreSQL — новости | Добавлен: https://www.postgresql.org/news.rss |
| s091 | wal2json — релизы | Добавлен: https://github.com/eulerto/wal2json/releases.atom |
| s092 | pgEdge — блог | Добавлен: https://www.pgedge.com/blog |
| s093 | Bucardo — релизы | Добавлен: https://github.com/bucardo/bucardo/releases.atom |
| s095 | MongoDB — блог | Не подключён; причина выше |
| s097 | Yugabyte — блог | Добавлен: https://www.yugabyte.com/blog/ |
| s098 | Apache Kafka — блог | Добавлен: https://kafka.apache.org/blog/index.xml |
| s101 | Bento — релизы | Добавлен: https://github.com/warpstreamlabs/bento/releases.atom |
| s102 | Apache Pulsar — блог | Добавлен: https://pulsar.apache.org/blog/rss.xml |
| s103 | Apache Camel — блог | Добавлен: https://camel.apache.org/blog/index.xml |
| s104 | Spring — блог | Добавлен: https://spring.io/blog.atom |
| s105 | Apache Pekko Connectors — релизы | Добавлен: https://github.com/apache/pekko-connectors/releases.atom |
| s106 | Akka — блог | Добавлен: https://akka.io/blog/rss.xml |
| s107 | RabbitMQ — блог | Добавлен: https://www.rabbitmq.com/blog/rss.xml |
| s108 | NATS — блог | Добавлен: https://nats.io/blog/ |
| s110 | Decodable — блог | Добавлен: https://www.decodable.co/blog |
| s113 | IBM Event Automation — ресурсы и блоги | Добавлен: https://www.ibm.com/products/event-automation/resources |
| s257 | Aiven — changelog | Добавлен: https://aiven.io/changelog/feed.xml |
| s258 | StreamNative — Medium | Не подключён; причина выше |
| s114 | Apache Beam — блог | Добавлен: https://beam.apache.org/feed.xml |
| s116 | Materialize — блог | Добавлен: https://materialize.com/rss.xml |
| s117 | Feldera — блог | Добавлен: https://www.feldera.com/blog |
| s118 | DeltaStream — блог | Не подключён; причина выше |
| s119 | Timeplus — блог | Не подключён; причина выше |
| s120 | Epsio — блог | Добавлен: https://www.epsio.io/blog |
| s121 | Quix — блог | Добавлен: https://quix.io/blog/ |
| s122 | Pathway — блог | Добавлен: https://pathway.com/blog/ |
| s123 | Hazelcast — блог | Добавлен: https://hazelcast.com/feed/ |
| s124 | Apache Storm — новости | Добавлен: https://storm.apache.org/ |
| s246 | Apache Spark — новости | Добавлен: https://spark.apache.org/news/ |
| s125 | EMQX — блог | Добавлен: https://www.emqx.com/en/blog |
| s126 | HiveMQ — блог | Не подключён; причина выше |
| s127 | eKuiper — блог EMQX | Добавлен: https://www.emqx.com/en/blog/category/kuiper |
| s128 | Node-RED — блог | Добавлен: https://nodered.org/feed.xml |
| s129 | Apache StreamPipes — блог | Добавлен: https://streampipes.apache.org/blog/rss.xml |
| s130 | Solace — блог | Добавлен: https://solace.com/feed/ |
| s259 | Solace — Medium | Не подключён; причина выше |
| s131 | Vector — блог | Добавлен: https://vector.dev/blog/index.xml |
| s132 | Fluent Bit — блог | Добавлен: https://fluentbit.io/blog/index.xml |
| s133 | Fluentd — блог | Добавлен: https://www.fluentd.org/blog/ |
| s134 | Elastic / Logstash — блог | Добавлен: https://www.elastic.co/blog/feed |
| s135 | Cribl — блог | Добавлен: https://cribl.io/blog/ |
| s136 | OpenTelemetry — блог | Добавлен: https://opentelemetry.io/blog/index.xml |
| s137 | Grafana — блог | Добавлен: https://grafana.com/blog/index.xml |
| s138 | Bindplane — блог | Не подключён; причина выше |
| s139 | Mezmo — блог | Добавлен: https://www.mezmo.com/blog |
| s140 | Edge Delta — блог | Добавлен: https://edgedelta.com/feed.xml |
| s141 | Chronosphere — блог | Не подключён; причина выше |
| s142 | Tenzir — блог | Добавлен: https://tenzir.com/blog/feed.xml |
| s143 | Axoflow — блог | Добавлен: https://axoflow.com/blog |
| s144 | OpenSearch — блог | Добавлен: https://opensearch.org/feed/ |
| s145 | Syslog-ng — community blog | Не подключён; причина выше |
| s146 | Rsyslog — блог | Добавлен: https://www.rsyslog.com/feed/ |
| s260 | Vector — релизы | Добавлен: https://vector.dev/releases/ |
| s261 | Fluent Bit — релизы | Добавлен: https://fluentbit.io/announcements/ |
| s262 | Tenzir — changelog | Добавлен: https://tenzir.com/changelog/feed.xml |
| s263 | Tenzir — Product Updates | Добавлен: https://tenzir.com/product/updates/feed.xml |
| s264 | Tenzir — newsletter | Добавлен: https://tenzir.com/newsletter/feed.xml |
| s265 | Syslog-ng — релизы | Добавлен: https://github.com/syslog-ng/syslog-ng/releases.atom |
| s266 | Rainer Gerhards — автор rsyslog | Добавлен: https://rainer.gerhards.net/feed |
| s147 | RudderStack — блог | Добавлен: https://www.rudderstack.com/blog/ |
| s148 | Snowplow — блог | Добавлен: https://snowplow.io/blog |
| s149 | Twilio / Segment — блог | Добавлен: https://www.twilio.com/en-us/blog |
| s150 | Tealium — блог | Добавлен: https://tealium.com/feed/ |
| s151 | Treasure AI — блог | Добавлен: https://www.treasure.ai/blog/rss.xml |
| s152 | Adobe Experience Cloud — блог | Не подключён; причина выше |
| s153 | Hightouch — блог | Добавлен: https://hightouch.com/blog/category/all/page/1 |
| s154 | Multiwoven — релизы | Добавлен: https://github.com/Multiwoven/multiwoven/releases.atom |
| s155 | GrowthLoop — блог | Добавлен: https://www.growthloop.com/resources/blogs |
| s156 | Omnata — блог | Добавлен: https://omnata.com/blog |
| s267 | Hightouch — changelog | Добавлен: https://hightouch.com/docs/changelog/feed.xml |
| s157 | Adverity — блог | Добавлен: https://www.adverity.com/blog/rss.xml |
| s158 | Funnel — блог | Добавлен: https://funnel.io/blog/rss.xml |
| s159 | Improvado — блог | Добавлен: https://improvado.io/blog |
| s160 | Supermetrics — блог | Добавлен: https://supermetrics.com/blog |
| s161 | Windsor.ai — блог | Добавлен: https://windsor.ai/blog/ |
| s162 | Renta — блог | Добавлен: https://renta.im/blog/ |
| s163 | Coupler.io — блог | Добавлен: https://blog.coupler.io/feed/ |
| s164 | Dataslayer — блог | Добавлен: https://www.dataslayer.ai/blog |
| s165 | OWOX — блог | Добавлен: https://www.owox.com/blog |
| s166 | MuleSoft — блог | Добавлен: https://blogs.mulesoft.com/feed/ |
| s167 | SnapLogic — блог | Не подключён; причина выше |
| s168 | Workato — Product Hub | Добавлен: https://www.workato.com/product-hub/feed/ |
| s169 | Workato — The Connector | Добавлен: https://www.workato.com/the-connector/feed/ |
| s170 | Jitterbit — блог | Добавлен: https://www.jitterbit.com/resources/type/blog/ |
| s171 | Celigo — блог | Добавлен: https://www.celigo.com/feed/ |
| s172 | Tray.ai — блог | Добавлен: https://tray.ai/blog/ |
| s173 | n8n — блог | Добавлен: https://blog.n8n.io/rss/ |
| s174 | WSO2 — блог | Добавлен: https://wso2.com/library/ |
| s175 | IBM webMethods — community blogs | Добавлен: https://community.ibm.com/community/user/groups/community-home/recent-community-blogs?communitykey=82b75916-ed06-4a13-8eb6-0190da9f1bfa |
| s176 | TIBCO — блог | Не подключён; причина выше |
| s177 | Frends — Insights | Добавлен: https://frends.com/insights/rss.xml |
| s178 | Digibee — блог | Добавлен: https://www.digibee.com/blog |
| s179 | Flowgear — блог | Добавлен: https://www.flowgear.net/blog/ |
| s180 | Linx — блог | Не подключён; причина выше |
| s181 | Elastic.io — блог | Не подключён; причина выше |
| s182 | Make — блог | Не подключён; причина выше |
| s183 | Zapier — Engineering | Не подключён; причина выше |
| s184 | Zapier — блог | Не подключён; причина выше |
| s185 | Activepieces — блог | Добавлен: https://www.activepieces.com/rss.xml |
| s186 | Pipedream — changelog | Добавлен: https://pipedream.com/docs/changelog/rss.xml |
| s187 | Zoho Flow — блог | Добавлен: https://www.zoho.com/blog/feed |
| s188 | Prismatic — блог | Добавлен: https://prismatic.io/blog/ |
| s189 | Paragon — блог | Добавлен: https://www.useparagon.com/blog |
| s190 | Cyclr — блог | Добавлен: https://cyclr.com/feed/ |
| s191 | Albato — блог | Добавлен: https://albato.com/blog/all |
| s268 | MuleSoft — Technically Speaking newsletter | Не подключён; причина выше |
| s269 | Paragon — changelog | Добавлен: https://docs.useparagon.com/changelog/product-updates |
| s270 | Albato — Medium | Не подключён; причина выше |
| s192 | Arenadata — новости и блог | Добавлен: https://arenadata.tech/ru/news |
| s193 | Loginom — блог | Добавлен: https://loginom.ru/blog |
| s194 | DATAREON — новости | Добавлен: https://datareon.ru/news/feed/ |
| s195 | Diasoft / Digital Q — новости | Добавлен: https://q.diasoft.ru/mediacenter/news/ |
| s196 | Neoflex — Хабр | Добавлен: https://habr.com/ru/rss/companies/neoflex/articles/?fl=ru |
| s271 | Arenadata — Telegram | Не подключён; причина выше |
| s272 | Loginom — Telegram | Не подключён; причина выше |
| s273 | DATAREON — Telegram | Не подключён; причина выше |
| s274 | Digital Q — Telegram | Не подключён; причина выше |
| s279 | Neoflex — Telegram | Не подключён; причина выше |
| s197 | Rclone — changelog | Добавлен: https://github.com/rclone/rclone/releases.atom |
| s198 | Rsync — новости | Не подключён; причина выше |
| s199 | pgloader — релизы | Добавлен: https://github.com/dimitri/pgloader/releases.atom |
| s200 | pgcopydb — релизы | Добавлен: https://github.com/dimitri/pgcopydb/releases.atom |
| s201 | Ora2Pg — новости | Добавлен: https://github.com/darold/ora2pg/releases.atom |
| s202 | mydumper — релизы | Добавлен: https://github.com/mydumper/mydumper/releases.atom |
| s203 | MySQL — блог | Не подключён; причина выше |
| s204 | SQLines — новости | Добавлен: https://sqlines.com/feed.php |
| s205 | AWS Storage — блог | Добавлен: https://aws.amazon.com/blogs/storage/feed/ |
| s206 | Google Cloud — Storage & Data Transfer | Добавлен: https://cloud.google.com/blog/products/storage-data-transfer |
| s207 | Progress MOVEit — блог | Добавлен: https://www.progress.com/blogs/moveit |
| s210 | Prefect — блог | Добавлен: https://www.prefect.io/blog |
| s212 | Apache DolphinScheduler — релизы | Добавлен: https://github.com/apache/dolphinscheduler/releases.atom |
| s213 | Argo — блог | Добавлен: https://blog.argoproj.io/feed |
| s214 | Flyte — блог и ресурсы | Добавлен: https://flyte.org/resources |
| s215 | Luigi — релизы | Добавлен: https://github.com/spotify/luigi/releases.atom |
| s216 | Digdag — релизы | Добавлен: https://github.com/treasure-data/digdag/releases.atom |
| s218 | Apache StreamPark — блог | Добавлен: https://streampark.apache.org/blog/rss.xml |
| s219 | Dinky — блог | Добавлен: https://www.dinky.org.cn/blog/rss.xml |
| s275 | Prefect — changelog | Добавлен: https://www.prefect.io/changelog/feed.xml |
| s220 | Daft — блог | Добавлен: https://www.eventual.ai/blog |
| s221 | Ray / Anyscale — блог | Не подключён; причина выше |
| s222 | Dask — блог | Добавлен: https://blog.dask.org/atom.xml |
| s223 | Polars — блог | Добавлен: https://pola.rs/rss.xml |
| s226 | Tobiko / SQLMesh — блог | Добавлен: https://www.tobikodata.com/blog |
| s227 | Coalesce — блог | Добавлен: https://coalesce.io/resources/ |
| s280 | Coiled — Dask blog | Добавлен: https://docs.coiled.io/blog/atom.xml |
| s228 | Denodo — Data Management Blog | Добавлен: https://www.datamanagementblog.com/feed/ |
| s229 | Dremio — блог | Добавлен: https://www.dremio.com/feed/ |
| s230 | Trino — блог | Добавлен: https://trino.io/blog/feed.xml |
| s231 | Starburst — блог | Добавлен: https://www.starburst.io/blog/ |
| s232 | Palantir — блог | Добавлен: https://blog.palantir.com/feed |
| s233 | Domo — блог | Добавлен: https://www.domo.com/blog/ |
| video-213 | Transferia Go / Yandex Data Transfer — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCjWOzgG0oTFHy4N4BeDmBhg |
| video-214 | Airbyte — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCQ_JWEFzs1_INqdhIO3kmrw |
| video-215 | Estuary Flow — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCJ9JIjh7uaUdjcFR6xTkJXQ |
| video-216 | Integrate.io — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC2ZnX1ePwLx8rM2iXa6tjwQ |
| video-217 | Keboola — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UChlGdJBp2rGJ5t6a062WfDw |
| video-218 | Etlworks — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCdSO6LqaHaQfiJPHtJbnB5Q |
| video-219 | Nexla — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCmUwWQe9J_32akd8oEQjw8g |
| video-220 | Skyvia — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCKyolgdX5CmgZyrreqQ-umA |
| video-221 | Weld — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC7T0gMt-hFmBT9SNzAfRNXA |
| video-222 | TROCCO — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCvgRgbI7FfwC1MCjOqipzOA |
| video-223 | CloudQuery — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCDe9ZMwW5GpM8i4ySOay6BA |
| video-224 | Informatica IDMC / Cloud Data Integration — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCvXtdT5kAsavz662XL7umvg |
| video-225 | WhereScape — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC-4Gkpf-U1gHTCyQaB4OugA |
| video-226 | Apache Hop — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCGlcYslwe03Y2zbZ1W6DAGA |
| video-227 | Meltano — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCe6DegNBlA1LcXmXQ_oR5rw |
| video-228 | dlt — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCmyrIjWrlpSdRwnY4NMVJRA |
| video-229 | ingestr — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UClwfN3abGuEuvPM0joP3sAg |
| video-230 | Debezium — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCk8VviAaxNZkakaL1hPykIg |
| video-231 | Striim — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCduNv6TDK3eNcG_ye0PetqQ |
| video-232 | SymmetricDS — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCSUv1QmSa65YSWQuZmHiCyQ |
| video-233 | dsync, Adiom — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCW1XTyNIO99-J2Xshwz-ixg |
| video-234 | pgstream, Xata — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCyN72GF-Ncow-7ltt5o8BFg |
| video-235 | Azure Database Migration Service — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC0m-80FnNY2Qb7obvTL_2fA |
| video-236 | Apache Hudi Streamer — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCs7AhE0BWaEPZSChrBR-Muw |
| video-237 | pgEdge — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCsyp3qG_p8mJUl2YH9_Kr6Q |
| video-238 | Redpanda Connect, ранее Benthos — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCMrqRNX9Og3wFjuI-qMbKHw |
| video-239 | Akka / Alpakka — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCcxTiKb1h2lY_gpTCDgXn1g |
| video-240 | NATS JetStream mirrors/sources — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCIfWp0bZie8O1hxQyM3uF-A |
| video-241 | Ververica — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCxt7B_fmLplq0OMRVxLiE8w |
| video-242 | Aiven for Apache Kafka Connect — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC4cZf5RyAD5EL0fQf5u8c1g |
| video-243 | Apache Beam — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UChNnb_YO_7B0HlW6FhAXZZQ |
| video-244 | Feldera — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCdZuDkmEebHigQVe2zkbRqA |
| video-245 | Timeplus — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCRQCOw9wOiqHZkm7ftAMdTQ |
| video-246 | EMQX — YouTube | Один и тот же канал уже подключён; активный дубль не создан |
| video-247 | eKuiper — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC5FjR77ErAxvZENEWzQaO5Q |
| video-248 | Cribl Stream — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC1tTDQgiC_o1eqPfRGk4ewA |
| video-249 | Grafana Alloy — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCYCwgQAMm9sTJv0rgwQLCxw |
| video-250 | Edge Delta — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCCecP7pVmJk3tSS3b2p4_DA |
| video-251 | Snowplow — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCeLlVPW8RF0zZNmRY2EWqbA |
| video-252 | Twilio Segment — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCWh3G9LZmZ3q_xWOyPpn8ag |
| video-253 | Treasure AI, ранее Treasure Data — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCzFa1bUqK0m4vIs6wUprHKA |
| video-254 | Hightouch — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCmjKHswj5UPdKqkrHB8RhVw |
| video-255 | GrowthLoop — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC-t1JTtAUUUS-Hk6WacRSXQ |
| video-256 | Adverity — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCeHuw64RddYTY_OFe7lEqag |
| video-257 | Funnel — YouTube | Не подключён; причина выше |
| video-258 | Supermetrics — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCLe4NReGgBmFK_9UAwoJHoQ |
| video-259 | Windsor.ai — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCyYRziXbzhGTJZZqcHRtOGw |
| video-260 | Coupler.io — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCS0ZTW6ynY34OZodixxwb7w |
| video-261 | Dataslayer — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCub0zxljQ_EyG12fDY6l5FA |
| video-262 | OWOX — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCOPsyFJWaxEKToscLXwAzPA |
| video-263 | Celigo — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCI7mNOr2sN3GfmvfCQozPpw |
| video-264 | Tray.ai — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCmwwe5BlB7Y5B5zh_LmPr1w |
| video-265 | Prismatic — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCCPVQjc-DPfnNHrrwWmrGVA |
| video-266 | Albato — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCkcmYx2c6gVgwTOXh4x7VAg |
| video-267 | Arenadata Streaming — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCxEzcAvtm20a2Z9n1BUggxg |
| video-268 | DATAREON Platform — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UC6LsWURBgG7T2LWRktYKGxw |
| video-269 | Apache Airflow — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCSXwxpWZQ7XZ1WL3wqevChA |
| video-270 | Dagster — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCfLnv9X8jyHTe6gJ4hVBo9Q |
| video-271 | Kestra — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCMCsjAEnJXzGsg_IAZF8WHQ |
| video-272 | Flyte — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCKeh7bpt9X9HxBd6TyGzqQg |
| video-273 | dbt — YouTube | Добавлен: https://www.youtube.com/feeds/videos.xml?channel_id=UCVpBwKK-ecMEV75y1dYLE5w |
| s234 | Talend Open Studio — статус | Не подключён; причина выше |
| s235 | Grouparoo — репозиторий | Добавлен: https://github.com/grouparoo/grouparoo/releases.atom |
| s236 | Apache Sqoop — Attic | Не подключён; причина выше |
| s237 | Apache Apex — Attic | Не подключён; причина выше |
| s238 | Apache Samza — блог | Добавлен: https://samza.apache.org/blog/ |
| s239 | Apache Flume — новости | Добавлен: https://flume.apache.org/ |
| s240 | Apache Heron — репозиторий | Добавлен: https://github.com/apache/incubator-heron/releases.atom |
| s241 | Bytewax — блог | Не подключён; причина выше |
| s242 | pg_flo — блог | Не подключён; причина выше |
| s243 | Equalum — блог | Не подключён; причина выше |
| s244 | Arcion — блог | Не подключён; причина выше |
| DF01 | DataFun — официальный сайт | Не подключён; причина выше |
| DF02 | WeChat Official Account / 微信公众号: DataFunTalk | Не подключён; причина выше |
| DF03 | WeChat Official Account / 微信公众号: DataFunSummit | Не подключён; причина выше |
| DF04 | WeChat Official Account / 微信公众号: 大话数智 | Не подключён; причина выше |
| DF05 | WeChat Channels / 微信视频号: DataFunTalk | Не подключён; причина выше |
| DF06 | Тематические и событийные группы WeChat / 微信群 | Не подключён; причина выше |
| DF07 | DataFunTalk — организатор на 活动行 / Huodongxing | Не подключён; причина выше |
| DF08 | DataFunTalk — 今日头条 / Toutiao | Не подключён; причина выше |
| DF09 | DataFunTalk — CSDN / DevPress, `DataFun_Hoh` | Добавлен: https://devpress.csdn.net/user/DataFun_Hoh |
| DF10 | DataFunTalk — 博客园 / CNBlogs | Добавлен: https://feed.cnblogs.com/blog/u/560452/rss/ |
| DF11 | DataFunTalk — 掘金 / Juejin | Не подключён; причина выше |
| DF12 | DataFunTalk — колонка Tencent Cloud Developer Community | Не подключён; причина выше |
| DF13 | DataFun — SegmentFault / 思否 | Не подключён; причина выше |
| DF14 | DataFunTalk — 墨天轮 / Modb | Не подключён; причина выше |
| DF15 | DataFunTalk — 新浪看点 / Sina | Не подключён; причина выше |

## Состояние загрузки на момент проверки

- AWS Storage — блог: outbound_rejected: transport failed: connect
