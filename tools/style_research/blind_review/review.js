(() => {
  "use strict";
  const data = JSON.parse(document.getElementById("review-data").textContent);
  const ids = data.items.map(item => item.id);
  const storageKey = `blind-author-review:v1:${data.evaluation_id}`;
  const identity = JSON.stringify(data);
  const get = id => document.getElementById(id);
  const issues = { facts: "Факты", numbers: "Числа", quotes: "Цитаты" };
  let answers = Object.fromEntries(ids.map(id => [id, {
    verdict: null, issues: { facts: false, numbers: false, quotes: false },
    comment: "", updated_at: null,
  }]));
  let current = 0;
  let rawMode = false;
  let persistedRaw = null;
  let recoverableRaw = null;
  let persistenceWarning = "";
  let canPersist = true;
  let exporting = false;

  // This artifact cannot import the app's component runtime. All editable
  // controls use this one offline primitive with the same autofill protections.
  function field(tag, type) {
    const element = document.createElement(tag);
    if (type) element.type = type;
    element.setAttribute("autocomplete", "none");
    element.name = `f_${crypto.randomUUID()}`;
    element.setAttribute("data-1p-ignore", "true");
    element.setAttribute("data-lpignore", "true");
    element.setAttribute("data-bwignore", "true");
    element.setAttribute("data-form-type", "other");
    return element;
  }

  function exactKeys(value, keys) {
    return value && typeof value === "object" && !Array.isArray(value)
      && Object.keys(value).length === keys.length
      && keys.every(key => Object.hasOwn(value, key));
  }

  function timestamp(value) {
    return typeof value === "string" && Number.isFinite(Date.parse(value))
      && new Date(value).toISOString() === value;
  }

  function validDraft(value) {
    if (!exactKeys(value, ["schema_version", "manifest", "answers", "current_index", "saved_at"])
      || value.schema_version !== 1 || JSON.stringify(value.manifest) !== identity
      || !exactKeys(value.answers, ids) || !Number.isInteger(value.current_index)
      || value.current_index < 0 || value.current_index >= ids.length
      || !timestamp(value.saved_at)) return false;
    return ids.every(id => {
      const answer = value.answers[id];
      return exactKeys(answer, ["verdict", "issues", "comment", "updated_at"])
        && [null, "like", "unlike"].includes(answer.verdict)
        && exactKeys(answer.issues, Object.keys(issues))
        && Object.keys(issues).every(key => typeof answer.issues[key] === "boolean")
        && typeof answer.comment === "string"
        && (answer.updated_at === null || timestamp(answer.updated_at));
    });
  }

  function status(message = "Оценка не выбрана заранее. Можно возвращаться к любому материалу.") {
    get("status").textContent = persistenceWarning || message;
    get("status").classList.toggle("warning", Boolean(persistenceWarning));
    get("recover-draft").hidden = recoverableRaw === null;
  }

  function blockPersistence(message, raw = null) {
    canPersist = false;
    persistenceWarning = message;
    recoverableRaw = raw;
    status();
  }

  try {
    persistedRaw = localStorage.getItem(storageKey);
    if (persistedRaw !== null) {
      let saved;
      try { saved = JSON.parse(persistedRaw); } catch { /* Preserve invalid bytes below. */ }
      if (validDraft(saved)) {
        answers = saved.answers;
        current = saved.current_index;
      } else {
        blockPersistence("Прежний черновик не совпадает с этой проверкой или повреждён. Он не изменён. Новые оценки сохраняйте экспортом.", persistedRaw);
      }
    }
  } catch {
    blockPersistence("Браузер не разрешил сохранить черновик. Оценки останутся на странице до её закрытия — используйте экспорт.");
  }

  function persist() {
    if (!canPersist) { status(); return; }
    try {
      const existing = localStorage.getItem(storageKey);
      if (existing !== persistedRaw) {
        blockPersistence("Черновик изменён в другой вкладке. Мы его не перезаписываем; сохраните свои текущие оценки экспортом.", existing);
        return;
      }
      const next = JSON.stringify({
        schema_version: 1, manifest: data, answers, current_index: current,
        saved_at: new Date().toISOString(),
      });
      localStorage.setItem(storageKey, next);
      persistedRaw = next;
      status("Черновик сохранён в этом браузере. Экспорт нужен для передачи оценок.");
    } catch {
      blockPersistence("Не удалось сохранить черновик. Текущие оценки не потеряны на странице — сохраните их экспортом.", persistedRaw);
    }
  }

  function safeUrl(value) {
    if (/[\s\\\u0000-\u001f\u007f]/u.test(value)) return false;
    try {
      const url = new URL(value);
      return ["https:", "http:"].includes(url.protocol) && !url.username && !url.password;
    } catch { return false; }
  }

  // The safe subset is deliberately DOM-only: no HTML parser, innerHTML, external
  // renderer, image loading, or executable URL. Unrecognized markup stays text.
  function inline(parent, text) {
    const token = /\*\*([^*\n]+)\*\*|__([^_\n]+)__|`([^`\n]+)`|\[([^\]\n]+)\]\(([^)\n]+)\)|\*([^*\n]+)\*/g;
    let offset = 0;
    for (const match of text.matchAll(token)) {
      parent.append(document.createTextNode(text.slice(offset, match.index)));
      let node;
      if (match[1] || match[2]) {
        node = document.createElement("strong"); node.textContent = match[1] || match[2];
      } else if (match[3]) {
        node = document.createElement("code"); node.textContent = match[3];
      } else if (match[4] && safeUrl(match[5])) {
        node = document.createElement("a"); node.textContent = match[4];
        node.setAttribute("href", match[5]); node.target = "_blank";
        node.rel = "noopener noreferrer"; node.referrerPolicy = "no-referrer";
      } else if (match[6]) {
        node = document.createElement("em"); node.textContent = match[6];
      } else {
        node = document.createTextNode(match[0]);
      }
      parent.append(node);
      offset = match.index + match[0].length;
    }
    parent.append(document.createTextNode(text.slice(offset)));
  }

  function renderSummary() {
    const summary = get("summary");
    const text = data.items[current].text;
    summary.replaceChildren();
    summary.classList.toggle("raw", rawMode);
    if (rawMode) summary.textContent = text;
    else {
      for (const line of text.split(/\r\n|\n|\r/)) {
        const heading = /^(#{1,6}) (.*)$/.exec(line);
        const node = document.createElement(heading ? "h2" : "div");
        node.className = heading ? "summary-heading" : "summary-line";
        inline(node, heading ? heading[2] : line);
        summary.append(node);
      }
    }
    summary.scrollTop = 0;
    get("raw-toggle").setAttribute("aria-pressed", String(rawMode));
  }

  const comment = field("textarea");
  comment.id = "comment";
  comment.setAttribute("aria-label", "Комментарий к стилю или ошибкам");
  get("comment-mount").append(comment);
  const issueFields = {};
  for (const [key, label] of Object.entries(issues)) {
    const wrapper = document.createElement("label"); wrapper.className = "issue";
    const input = field("input", "checkbox"); input.id = `issue-${key}`;
    wrapper.append(input, document.createTextNode(label)); get("issues").append(wrapper);
    issueFields[key] = input;
    input.addEventListener("change", () => update(answer => { answer.issues[key] = input.checked; }));
  }
  comment.addEventListener("input", () => update(answer => { answer.comment = comment.value; }));

  const itemButtons = data.items.map((item, index) => {
    const button = document.createElement("button");
    button.type = "button"; button.className = "item-button"; button.textContent = index + 1;
    button.addEventListener("click", () => navigate(index));
    get("item-nav").append(button);
    return button;
  });

  function progress() {
    const completed = ids.filter(id => answers[id].verdict !== null).length;
    get("progress-text").textContent = `Оценено ${completed} из ${ids.length}`;
    get("progress").max = ids.length; get("progress").value = completed;
    data.items.forEach((item, index) => {
      const button = itemButtons[index];
      const rated = answers[item.id].verdict !== null;
      button.dataset.rated = String(rated);
      button.setAttribute("aria-label", `Материал ${index + 1}: ${rated ? "оценён" : "без оценки"}`);
      if (index === current) button.setAttribute("aria-current", "step");
      else button.removeAttribute("aria-current");
    });
    get("like").setAttribute("aria-pressed", String(answers[ids[current]].verdict === "like"));
    get("unlike").setAttribute("aria-pressed", String(answers[ids[current]].verdict === "unlike"));
  }

  function update(mutate) {
    const answer = answers[ids[current]];
    mutate(answer);
    answer.updated_at = new Date().toISOString();
    progress(); // Synchronous visual feedback, before persistence.
    persist();
  }

  function navigate(index) {
    if (index < 0 || index >= ids.length) return;
    current = index;
    const item = data.items[current];
    get("item-number").textContent = `Материал ${current + 1} из ${ids.length}`;
    if (!safeUrl(item.url)) throw new Error("Manifest contains an invalid source URL");
    get("source-link").setAttribute("href", item.url);
    get("source-link").title = item.title;
    get("source-link").setAttribute("aria-label", `Открыть оригинал: ${item.title}`);
    const answer = answers[item.id];
    comment.value = answer.comment;
    Object.keys(issues).forEach(key => { issueFields[key].checked = answer.issues[key]; });
    get("previous").disabled = current === 0;
    get("next").disabled = current === ids.length - 1;
    renderSummary(); progress(); persist();
  }

  function download(text, name) {
    const url = URL.createObjectURL(new Blob([text], { type: "application/json;charset=utf-8" }));
    const link = document.createElement("a"); link.href = url; link.download = name;
    document.body.append(link); link.click(); link.remove();
    // Let the browser consume the object URL before releasing its backing bytes.
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  function exportPayload() {
    const completed = ids.filter(id => answers[id].verdict !== null).length;
    return {
      schema_version: 1, evaluation_id: data.evaluation_id,
      status: completed === ids.length ? "complete" : "partial",
      total_items: ids.length, completed_items: completed,
      exported_at: new Date().toISOString(),
      items: data.items.map(item => ({ ...item, ...answers[item.id] })),
    };
  }

  get("like").addEventListener("click", () => update(answer => { answer.verdict = "like"; }));
  get("unlike").addEventListener("click", () => update(answer => { answer.verdict = "unlike"; }));
  get("previous").addEventListener("click", () => navigate(current - 1));
  get("next").addEventListener("click", () => navigate(current + 1));
  get("raw-toggle").addEventListener("click", () => { rawMode = !rawMode; renderSummary(); });
  get("recover-draft").addEventListener("click", () => {
    if (recoverableRaw !== null) download(recoverableRaw, `${data.evaluation_id}-previous-draft.json`);
  });
  get("export").addEventListener("click", async () => {
    if (exporting) return;
    exporting = true;
    get("export").disabled = true;
    get("export").setAttribute("aria-busy", "true");
    status("Подготовка файла…");
    try {
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const payload = exportPayload();
      download(JSON.stringify(payload, null, 2), `${data.evaluation_id}-${payload.status}.json`);
      status(payload.status === "complete"
        ? "Экспортированы все оценки. Сохраните скачанный JSON-файл."
        : `Экспорт неполный: оценено ${payload.completed_items} из ${payload.total_items}. Остальные оценки не подставлены.`);
    } catch {
      status("Не удалось подготовить экспорт. Ваши оценки остаются на странице; попробуйте ещё раз.");
    } finally {
      exporting = false;
      get("export").disabled = false;
      get("export").setAttribute("aria-busy", "false");
    }
  });
  window.addEventListener("storage", event => {
    if (event.key === storageKey && event.newValue !== persistedRaw)
      blockPersistence("Черновик изменён в другой вкладке. Мы его не перезаписываем; сохраните свои текущие оценки экспортом.", event.newValue);
  });
  navigate(current);
})();
