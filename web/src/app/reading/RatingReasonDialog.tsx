import "./article-feedback.css";
import { ModalDialog } from "../../ui/ModalDialog";
import { AutofillResistantTextarea } from "../../ui/fields";
import { StatusRegion } from "../../ui/StatusRegion";

/** An explicit confirmation step: opening/closing never changes article state.
 * Pending saves lock dismissal and input; a lost response replays its exact text. */
export function RatingReasonDialog({
  score,
  reason,
  onReason,
  busy,
  uncertain,
  error,
  onClose,
  onSave,
}: {
  score: number;
  reason: string;
  onReason: (value: string) => void;
  busy: boolean;
  uncertain: boolean;
  error: string;
  onClose: () => void;
  onSave: () => void;
}) {
  const invalid = reason.includes("\0");
  return (
    <ModalDialog
      title={`Почему ${score} из 10?`}
      onClose={onClose}
      closeDisabled={busy}
      width="520px"
    >
      <form
        class="reading-reason"
        aria-busy={busy}
        onSubmit={(event) => {
          event.preventDefault();
          if (!busy && !invalid) onSave();
        }}
      >
        <label>
          <span class="reading-reason__label">
            Что повлияло на оценку? <span>Необязательно</span>
          </span>
          <AutofillResistantTextarea
            autoFocus
            value={reason}
            disabled={busy || uncertain}
            aria-label="Пояснение к оценке"
            aria-invalid={invalid}
            placeholder="Что было полезно, что нового узнал, чего не хватило…"
            onInput={(event) => onReason(event.currentTarget.value)}
          />
        </label>
        <StatusRegion class="reading-reason__status" busy={busy}>
          <span class={error || invalid ? "ai-error" : ""}>
            {invalid
              ? "Explanation cannot contain U+0000."
              : busy
                ? "Сохраняем оценку и пояснение…"
                : error || ""}
          </span>
        </StatusRegion>
        <div class="reading-reason__actions">
          <button
            type="button"
            class="secondary-button"
            disabled={busy}
            onClick={onClose}
          >
            Назад к статье
          </button>
          <button
            type="submit"
            class="primary-button reading-reason__save"
            disabled={busy || invalid}
            aria-busy={busy}
          >
            <span>{uncertain ? "Retry saving" : "Save & next →"}</span>
            {busy && (
              <span class="reading-reason__pending">
                <span class="spinner" />
              </span>
            )}
          </button>
        </div>
      </form>
    </ModalDialog>
  );
}
