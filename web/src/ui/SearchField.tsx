import { AutofillResistantField } from "../ui/fields";
import { Icon } from "../ui/Icon";
/** The shared search entry keeps input and submit coordinates stable while busy. */
export function SearchField({
  value,
  onInput,
  onSubmit,
  busy = false,
  label = "Search news and wiki",
}: {
  value: string;
  onInput: (v: string) => void;
  onSubmit: () => void;
  busy?: boolean;
  label?: string;
}) {
  return (
    <form
      class="unified-search-field"
      onSubmit={(e) => {
        e.preventDefault();
        if (!busy) onSubmit();
      }}
    >
      <Icon name="search" />
      <AutofillResistantField
        type="search"
        aria-label={label}
        placeholder={label}
        value={value}
        onInput={(e) => onInput(e.currentTarget.value)}
      />
      <button type="submit" disabled={busy} aria-busy={busy}>
        <span style={{ opacity: busy ? 0 : 1 }}>Search</span>
        {busy && (
          <span class="async-button__pending">
            <span class="spinner" />
          </span>
        )}
      </button>
    </form>
  );
}
