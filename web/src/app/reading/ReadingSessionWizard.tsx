import { useRef, useState } from "preact/hooks";
import { ModalDialog } from "../../ui/ModalDialog";
import { AutofillResistantField } from "../../ui/fields";
import { StatusRegion } from "../../ui/StatusRegion";
import { readingPlan, sessionUrl } from "./readingSession";
import "./reading-session.css";
export function ReadingSessionWizard({
  workspace,
  onClose,
  onStart,
}: {
  workspace: string;
  onClose: () => void;
  onStart: (url: string) => boolean;
}) {
  const [smart, setSmart] = useState("45"),
    [random, setRandom] = useState("15");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  return (
    <ModalDialog
      title="Сессия чтения"
      onClose={onClose}
      closeDisabled={busy}
      width="480px"
    >
      <form
        class="reading-session-wizard"
        onSubmit={(event) => {
          event.preventDefault();
          if (lock.current) return;
          try {
            const plan = readingPlan(smart, random);
            lock.current = true;
            setBusy(true);
            if (!onStart(sessionUrl(workspace, plan))) {
              lock.current = false;
              setBusy(false);
            }
          } catch (cause) {
            setError((cause as Error).message);
          }
        }}
      >
        <p>
          Сначала самые интересные статьи, затем та же лента в случайном
          порядке. Ноль минут пропускает этап.
        </p>
        <label>
          Умная лента · минуты
          <AutofillResistantField
            inputMode="numeric"
            value={smart}
            disabled={busy}
            onInput={(event) => setSmart(event.currentTarget.value)}
          />
        </label>
        <label>
          Случайная лента · минуты
          <AutofillResistantField
            inputMode="numeric"
            value={random}
            disabled={busy}
            onInput={(event) => setRandom(event.currentTarget.value)}
          />
        </label>
        <p>
          Смена режима — после текущей статьи. Скрытая вкладка не расходует
          время.
        </p>
        <StatusRegion class="reading-session-wizard__status">
          {error}
        </StatusRegion>
        <div class="reading-reason__actions">
          <button
            type="button"
            class="secondary-button"
            disabled={busy}
            onClick={onClose}
          >
            Отмена
          </button>
          <button
            type="submit"
            class="primary-button"
            disabled={busy}
            aria-busy={busy}
          >
            Начать чтение
          </button>
        </div>
      </form>
    </ModalDialog>
  );
}
