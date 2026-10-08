import { useLayoutEffect, useRef, useState } from "preact/hooks";
import { ApiError, type ApiClient } from "../../api/client";
import type { CompleteReading, ReadingState } from "../../api/focusedReading";
import { assertWire } from "../../api/decode";
import { AutofillResistantTextarea } from "../../ui/fields";
import { StatusRegion } from "../../ui/StatusRegion";
import "./article-feedback.css";

type Draft = {
  score: number | null;
  abstain?: boolean;
  reason: string;
  pending?: CompleteReading;
};
/** Explicit single-article read action. A pending receipt is retained before sending,
 * so a lost response can only replay the same rating and exact explanation. */
export function ArticleFeedback({
  client,
  owner,
  workspace,
  article,
  onSaved,
  onBusy,
  inline = false,
  onSave,
  onSkip,
  ratingOptional = false,
}: {
  client: ApiClient;
  owner: string;
  workspace: string;
  article: string;
  onSaved: () => void;
  onBusy?: (busy: boolean) => void;
  inline?: boolean;
  ratingOptional?: boolean;
  onSave: (command: CompleteReading) => Promise<void>;
  onSkip?: () => Promise<void>;
}) {
  const key = `mark-read-feedback:${owner}:${workspace}:${article}`;
  const [draft, setDraft] = useState<Draft>({ score: null, reason: "" });
  const [state, setState] = useState<ReadingState | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const lock = useRef(false);
  const [completed, setCompleted] = useState(false);
  useLayoutEffect(() => {
    let active = true;
    void (async () => {
      try {
        const stored = sessionStorage.getItem(key);
        if (stored) {
          const value = JSON.parse(stored) as Draft;
          if (
            typeof value.reason !== "string" ||
            (value.abstain !== undefined &&
              typeof value.abstain !== "boolean") ||
            !(
              value.score === null ||
              (Number.isInteger(value.score) &&
                value.score >= 1 &&
                value.score <= 10)
            )
          )
            throw Error(
              "Invalid saved rating draft. It has not been overwritten.",
            );
          if (value.pending) assertWire("CompleteReading", value.pending);
          setDraft(value);
        }
        const result = await client.reading.state(workspace, article);
        if (active) setState(result);
      } catch (cause) {
        if (active) setError((cause as Error).message);
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [client, key]);
  const remember = (value: Draft) => {
    sessionStorage.setItem(key, JSON.stringify(value));
    setDraft(value);
  };
  const edit = (value: Draft) => {
    setDraft(value);
    try {
      remember(value);
      setError("");
    } catch {
      setError(
        "Browser storage unavailable. Keep this window open to retain your explanation.",
      );
    }
  };
  const save = async (skip: boolean) => {
    if (lock.current || loading || !state || draft.reason.includes("\0"))
      return;
    if (!skip && draft.score === null && !draft.abstain && !ratingOptional)
      return;
    lock.current = true;
    setBusy(true);
    onBusy?.(true);
    setError("");
    try {
      if (skip) await onSkip?.();
      else {
        const command = draft.pending ?? {
          operationId: crypto.randomUUID(),
          expectedRevision: state.revision,
          rating: draft.score,
          reason: draft.reason === "" ? null : draft.reason,
        };
        remember({ ...draft, pending: command });
        await onSave(command);
      }
      sessionStorage.removeItem(key);
      setCompleted(true);
      onSaved();
    } catch (cause) {
      setError((cause as Error).message);
      if (cause instanceof ApiError && cause.status < 500) {
        try {
          remember({
            score: draft.score,
            reason: draft.reason,
            abstain: draft.abstain,
          });
        } catch {
          /* Keep the pending receipt in memory/storage for explicit recovery. */
        }
      }
    } finally {
      lock.current = false;
      setBusy(false);
      onBusy?.(false);
    }
  };
  const invalid = draft.reason.includes("\0");
  return (
    <div
      class={`reading-reason ${inline ? "reading-reason--inline" : ""}`}
      aria-busy={loading || busy}
    >
      <div class="mark-read-rating" role="group" aria-label="Article quality">
        {Array.from({ length: 10 }, (_, i) => i + 1).map((value) => (
          <button
            type="button"
            aria-label={`Rate ${value} out of 10`}
            aria-pressed={draft.score === value}
            disabled={
              busy || !!draft.pending || completed || (inline && !!state?.read)
            }
            onClick={() =>
              edit({
                ...draft,
                abstain: false,
                score: draft.score === value ? null : value,
              })
            }
          >
            {value}
          </button>
        ))}
        <button
          type="button"
          class="mark-read-rating__unknown"
          aria-pressed={!!draft.abstain}
          disabled={
            busy || !!draft.pending || completed || (inline && !!state?.read)
          }
          onClick={() =>
            edit({ ...draft, score: null, abstain: !draft.abstain })
          }
        >
          Не знаю
        </button>
      </div>
      <label
        style={{
          visibility:
            draft.score === null && !draft.abstain ? "hidden" : "visible",
        }}
      >
        <span class="reading-reason__label">
          Почему такая оценка? <span>Необязательно</span>
        </span>
        <AutofillResistantTextarea
          autoFocus={!inline}
          aria-label="Пояснение к оценке"
          value={draft.reason}
          disabled={
            busy || !!draft.pending || completed || (inline && !!state?.read)
          }
          aria-invalid={invalid}
          placeholder="Что было полезно, чего не хватило…"
          onInput={(event) =>
            edit({ ...draft, reason: event.currentTarget.value })
          }
        />
      </label>
      <StatusRegion class="reading-reason__status" busy={busy || loading}>
        {completed
          ? "Прочитано · оценка сохранена"
          : busy
            ? "Сохраняем…"
            : loading
              ? ""
              : invalid
                ? "Explanation cannot contain U+0000."
                : error ||
                  (draft.pending
                    ? "Повтор подтвердит прежнее сохранение."
                    : state?.read
                      ? inline
                        ? "Уже прочитано"
                        : "Статья уже прочитана. Закройте окно и обновите список."
                      : "")}
      </StatusRegion>
      <div class="reading-reason__actions">
        {onSkip && (
          <button
            class="secondary-button"
            disabled={
              loading ||
              busy ||
              !!draft.pending ||
              draft.score !== null ||
              !!draft.abstain ||
              draft.reason !== "" ||
              !state
            }
            onClick={() => void save(true)}
          >
            Без оценки
          </button>
        )}
        <button
          class="primary-button reading-reason__save"
          aria-busy={busy}
          disabled={
            completed ||
            loading ||
            busy ||
            !state ||
            (state.read && !draft.pending) ||
            (draft.score === null && !draft.abstain && !ratingOptional) ||
            invalid
          }
          onClick={() => void save(false)}
        >
          <span>
            {completed
              ? "Прочитано ✓"
              : draft.pending && !busy
                ? "Повторить"
                : inline
                  ? "Прочитано"
                  : "Сохранить"}
          </span>
          {busy && (
            <span class="reading-reason__pending">
              <span class="spinner" />
            </span>
          )}
        </button>
      </div>
    </div>
  );
}
