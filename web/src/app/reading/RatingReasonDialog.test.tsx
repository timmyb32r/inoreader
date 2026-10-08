import { fireEvent, render, screen } from "@testing-library/preact";
import { RatingReasonDialog } from "./RatingReasonDialog";
it("supports reselecting a score while retaining the explanation and locks ratings during pending/uncertain saves", () => {
  const onScore = vi.fn(),
    onReason = vi.fn();
  const props = {
    score: 3,
    reason: "Useful details",
    onScore,
    onReason,
    busy: false,
    uncertain: false,
    error: "",
    onClose: vi.fn(),
    onSave: vi.fn(),
  };
  const { rerender } = render(<RatingReasonDialog {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "Rate 9 out of 10" }));
  expect(onScore).toHaveBeenCalledWith(9);
  expect(screen.getByLabelText("Пояснение к оценке")).toHaveValue(
    "Useful details",
  );
  rerender(<RatingReasonDialog {...props} score={9} busy />);
  expect(
    screen.getByRole("button", { name: "Rate 9 out of 10" }),
  ).toBeDisabled();
  rerender(<RatingReasonDialog {...props} uncertain />);
  expect(screen.getByRole("button", { name: "Не знаю" })).toBeDisabled();
});
