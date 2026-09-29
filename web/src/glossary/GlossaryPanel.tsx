import { usePanelDock } from "../ui/PanelDock";
import { useEffect, useRef, useState } from "preact/hooks";
import { CopyButton } from "../ui/CopyButton";
import type {
  DefinitionsView,
  EntityDefinition,
  KnownDefinition,
} from "../api/glossary";
import { AsyncButton } from "../ui/AsyncButton";
import { FloatingPanel } from "../ui/FloatingPanel";
import { Icon } from "../ui/Icon";
import { StatusRegion } from "../ui/StatusRegion";
import { useFloatingPanel } from "../ui/useFloatingPanel";
import { StyledParagraph } from "./StyledParagraph";
import "./glossary.css";
import type { useGlossary } from "./useGlossary";

export function GlossaryPanel({
  controller,
  embedded = false,
}: {
  controller: ReturnType<typeof useGlossary>;
  embedded?: boolean;
}) {
  const [actionError, setActionError] = useState("");
  const dock = usePanelDock();
  const floating = useFloatingPanel(false, !embedded);
  const [shown, setShown] = useState<DefinitionsView | null>(controller.view);
  const latest = useRef(controller.view),
    pressed = useRef(false),
    scrolling = useRef(false),
    timer = useRef<number>();
  const body = useRef<HTMLDivElement>(null),
    newBlock = useRef<HTMLDivElement>(null);
  latest.current = controller.view;
  const flush = () => {
    const selection = window.getSelection();
    if (
      !pressed.current &&
      !scrolling.current &&
      !(
        selection &&
        !selection.isCollapsed &&
        body.current?.contains(selection.anchorNode)
      )
    )
      setShown(latest.current);
  };
  useEffect(() => {
    flush();
  }, [controller.view]);
  useEffect(() => {
    const release = () => {
      pressed.current = false;
      flush();
    };
    window.addEventListener("pointerup", release);
    window.addEventListener("pointercancel", release);
    document.addEventListener("selectionchange", flush);
    floating.element.current
      ?.querySelector<HTMLElement>(".glossary-close")
      ?.focus();
    return () => {
      clearTimeout(timer.current);
      window.removeEventListener("pointerup", release);
      window.removeEventListener("pointercancel", release);
      document.removeEventListener("selectionchange", flush);
    };
  }, []);
  const entities =
    shown?.job?.status === "completed" ? shown.job.result.entities : [];
  const known = shown?.known ?? [],
    names = new Set(known.map((k) => k.definition.term)),
    fresh = entities.filter((e) => !names.has(e.name));
  const status = controller.busy
    ? "Извлекаем термины…"
    : controller.error ||
      actionError ||
      (shown?.job?.status === "failed"
        ? shown.job.error
        : shown && !shown.channel.indexReady
          ? "Сначала импортируйте историю канала."
          : shown && !shown.channel.generationAllowed
            ? "DeepSeek недоступен для этого аккаунта."
            : "");
  const Shell = embedded ? EmbeddedGlossary : FloatingPanel;
  return (
    <Shell
      position={floating}
      label="Термины статьи"
      onClose={controller.close}
      class="glossary-panel"
      onPointerDown={() => {
        pressed.current = true;
      }}
    >
      <header>
        <div
          class="glossary-handle"
          role={embedded ? undefined : "button"}
          tabIndex={embedded ? undefined : 0}
          aria-label="Переместить окно терминов"
          {...(embedded ? {} : (dock ?? floating.handle))}
        >
          <small>Термины</small>
          <strong>{controller.target?.title}</strong>
        </div>
        <button
          class="icon-button glossary-close"
          aria-label="Закрыть термины"
          onClick={controller.close}
        >
          <Icon name="close" />
        </button>
      </header>
      <StatusRegion class="glossary-status" busy={controller.busy}>
        {controller.busy && <span class="spinner" />}
        <span>{status}</span>
      </StatusRegion>
      <div
        ref={body}
        class="glossary-content"
        onScroll={() => {
          scrolling.current = true;
          clearTimeout(timer.current);
          timer.current = window.setTimeout(() => {
            scrolling.current = false;
            flush();
          }, 180);
        }}
      >
        <section>
          <h3>
            Новые определения <span>{fresh.length}</span>
          </h3>
          <div ref={newBlock}>
            {fresh.map((e) => (
              <NewDefinition key={e.name} entity={e} />
            ))}
          </div>
          {shown?.job?.status === "completed" && !fresh.length && (
            <p class="glossary-muted">Новых определений нет.</p>
          )}
        </section>
        <hr />
        <section>
          <h3>
            Уже объяснено в канале <span>{known.length}</span>
          </h3>
          {known.map((k) => (
            <Known key={k.definition.term} value={k} />
          ))}
          {shown?.job?.status === "completed" && !known.length && (
            <p class="glossary-muted">Совпадений по точному названию нет.</p>
          )}
        </section>
      </div>
      <footer>
        <span class="glossary-muted">
          {shown?.channel.posts ?? "—"} постов ·{" "}
          {shown?.channel.configured ? "бот подключён" : "бот не подключён"}
          <small>Публичная история может быть неполной</small>
        </span>
        <AsyncButton
          class="icon-button"
          aria-label="Обновить определения"
          title="Обновить определения · новый запрос DeepSeek"
          disabled={
            controller.busy ||
            !shown?.channel.generationAllowed ||
            !shown.channel.indexReady
          }
          onPress={async () => {
            setActionError("");
            await controller.retry();
          }}
          onError={(error) =>
            setActionError(
              error instanceof Error ? error.message : "Request failed",
            )
          }
        >
          <Icon name="refresh" />
        </AsyncButton>
        <CopyButton
          label="Скопировать все новые определения"
          text={fresh
            .map((e) => `**${e.name}** — ${e.explanation}`)
            .join("\n\n")}
          html={() =>
            Array.from(
              newBlock.current?.querySelectorAll(".glossary-paragraph") ?? [],
            )
              .map((p) => `<p>${p.innerHTML}</p>`)
              .join("")
          }
        />
      </footer>
    </Shell>
  );
}
function NewDefinition({ entity: e }: { entity: EntityDefinition }) {
  const paragraph = useRef<HTMLParagraphElement>(null);
  return (
    <article class="glossary-entry">
      <p class="glossary-paragraph" ref={paragraph}>
        <strong>{e.name}</strong> — {e.explanation}
      </p>
      {e.insufficientContext && (
        <small class="glossary-muted">
          Недостаточно контекста для уверенного определения
        </small>
      )}
      <CopyButton
        label={`Скопировать ${e.name}`}
        text={`**${e.name}** — ${e.explanation}`}
        html={() => `<p>${paragraph.current?.innerHTML ?? ""}</p>`}
      />
    </article>
  );
}
function Known({ value: k }: { value: KnownDefinition }) {
  const paragraph = useRef<HTMLDivElement>(null);
  return (
    <article class="glossary-entry">
      <div ref={paragraph}>
        <StyledParagraph value={k.definition.paragraph} />
      </div>
      <a href={k.permalink} target="_blank" rel="noopener noreferrer">
        Пост в канале ↗
      </a>
      {k.stale && (
        <small class="glossary-muted">
          Есть конфликт обновления — показана сохранённая версия
        </small>
      )}
      <CopyButton
        label={`Скопировать известное определение ${k.definition.term}`}
        text={k.definition.paragraph.text}
        html={() => paragraph.current?.innerHTML ?? ""}
      />
    </article>
  );
}

function EmbeddedGlossary({
  children,
  onPointerDown,
}: import("preact").ComponentProps<typeof FloatingPanel>) {
  return (
    <section
      class="glossary-panel glossary-panel--embedded"
      aria-label="Article terms"
      onPointerDown={onPointerDown}
    >
      {children}
    </section>
  );
}
