# Frontend ownership

The app shell coordinates account/workspace state, article navigation and popup
history. Feature modules should not import the shell or own its navigation state.

- `SubscriptionsPage` owns modal focus, Escape/close handling and keeps the catalog
  mounted while details are open, preserving filters and selection on Back.
- `SubscriptionCatalog` owns table interactions and bulk actions.
- `subscriptionCatalogModel` owns column definitions, workspace-local preferences,
  sorting and the shared attention predicate used by both filters and indicators.
  Needs attention is a mandatory column, also inserted into saved layouts; its
  ascending order puts problematic sources first, with name as a stable tie-break.
  A transient raw error alone does not qualify a source as needing attention.
- `SubscriptionDetails` owns source editing and lazy detail-tab requests.
- `LatestArticles` is a stateless list; navigation is supplied by the shell. Its
  hover, focus and pressed feedback change colors, never row geometry.
- `PublicationHistory` owns its calendar, scales and history query.
- `AdvancedSettings` owns workspace/account management and OPML controls.
- `ui/formatArticleDate` is the shared display formatter; no feature imports App
  merely to reuse date formatting.

This refactor changes no API, database schema, request count or ownership boundary.
Existing catalog/detail/popup tests exercise behavior across the module boundaries.
The new browser checks cover attention sorting, row target stability and the
full-width sidebar toggle. The shell still owns article pagination and mutations;
backend storage/fetch workers were not refactored as part of this UI task.

The favicon is an embedded public asset, generated from the Reader RSS brand mark
by `python3 tools/generate_favicon.py` (Pillow). Vite copies `web/public/favicon.ico`
to the build and reader-server-ui embeds it with the ICO MIME type.
