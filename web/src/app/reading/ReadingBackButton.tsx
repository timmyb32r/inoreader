import "./reading-navigation.css";

/** Same control and coordinates in both reading surfaces; the caller owns the destination. */
export function ReadingBackButton({
  onClick,
  disabled = false,
}: {
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      class="secondary-button reading-back"
      disabled={disabled}
      onClick={onClick}
    >
      ← Back
    </button>
  );
}
