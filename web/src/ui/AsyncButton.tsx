import type { ComponentChildren, JSX } from "preact";
import { useRef, useState } from "preact/hooks";
type Props = Omit<
  JSX.ButtonHTMLAttributes<HTMLButtonElement>,
  "onClick" | "children"
> & {
  children: ComponentChildren;
  onPress: () => Promise<unknown>;
  onError: (error: unknown) => void;
};
/** The lock is synchronous, before Promise creation; labels and hit boxes never
 * change while pending. Errors belong in the caller's reserved status region. */
export function AsyncButton({
  children,
  onPress,
  onError,
  disabled,
  ...props
}: Props) {
  const locked = useRef(false);
  const [busy, setBusy] = useState(false);
  return (
    <button
      {...props}
      disabled={!!disabled || busy}
      aria-busy={busy}
      onClick={() => {
        if (locked.current || disabled) return;
        locked.current = true;
        setBusy(true);
        Promise.resolve()
          .then(onPress)
          .catch(onError)
          .finally(() => {
            locked.current = false;
            setBusy(false);
          });
      }}
    >
      <span style={{ visibility: busy ? "hidden" : undefined }}>
        {children}
      </span>
      {busy && (
        <span class="async-button__pending" aria-hidden="true">
          <span class="spinner" />
        </span>
      )}
    </button>
  );
}
