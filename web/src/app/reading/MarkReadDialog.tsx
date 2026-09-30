import { useState } from "preact/hooks";
import type { ApiClient } from "../../api/client";
import type { CompleteReading } from "../../api/focusedReading";
import { ModalDialog } from "../../ui/ModalDialog";
import { ArticleFeedback } from "./ArticleFeedback";

export function MarkReadDialog({
  onClose,
  ...props
}: {
  client: ApiClient;
  owner: string;
  workspace: string;
  article: string;
  onClose: () => void;
  onSave: (command: CompleteReading) => Promise<void>;
  onSkip: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  return (
    <ModalDialog
      title="Оценить статью"
      description="Оценка и пояснение сохранятся вместе с отметкой прочитано."
      onClose={onClose}
      closeDisabled={busy}
    >
      <ArticleFeedback {...props} onSaved={onClose} onBusy={setBusy} />
    </ModalDialog>
  );
}
