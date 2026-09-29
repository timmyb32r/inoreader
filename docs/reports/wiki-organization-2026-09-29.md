# Search and wiki organization

- One global search entry remains in the reader sidebar; keyboard shortcuts remain.
  Opening search focuses the query without stealing focus on subsequent results.
- Pages can select a parent within their namespace, with revision checks, cycle
  rejection and history restoration. Parent/children navigation is available in
  the page organization section.
- Account-private stars, Favorites and Standalone pages are namespace-scoped and
  honor current access. Standalone means neither parent nor children.
- Article W+ creation organizes the new page under the registered Subscriptions
  parent before binding it. Six existing pages serve seven subscription bindings.

Verification: `just check-affected`, `just check-release`, 193 frontend tests,
74 browser scenarios, PostgreSQL upgrade and organization acceptance, concurrency
and cross-namespace/role tests, Chromium and backup/restore acceptance. Browser
coverage checks pending feedback, duplicate activation and stable controls.

Deployment requires the explicit release 9→10 upgrade and a protected wiki backup.
Names and Markdown are checked byte-for-byte during the existing-page organization.

Deployed and verified on 158.160.186.87. Release 10 is active. The registered
Subscriptions page is `bd70ad2d-7824-4d4a-889e-694e780306d6` in namespace
`0a009c26-c44f-489c-9083-58a56bc072f8`. All six source pages were moved without
changing names or Markdown; zero subscription bindings remain unorganized.
Authenticated browser verification confirmed one search entry, focused search,
six children, Favorites and Standalone screens. Both collection endpoints return
HTTP 200; the current namespace has three standalone pages.
