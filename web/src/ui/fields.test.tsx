import { fireEvent, render, screen } from "@testing-library/preact";
import { AuthField, AutofillResistantField, AutofillResistantSelect, AutofillResistantTextarea } from "./fields";

describe("field contracts", () => {
  it("stamps opaque protection after caller props", () => {
    render(<AutofillResistantField aria-label="Feed URL" data-1p-ignore="false" />);
    const field = screen.getByLabelText("Feed URL");
    expect(field).toHaveAttribute("autocomplete", "none");
    expect(field).toHaveAttribute("data-1p-ignore", "true");
    expect(field.getAttribute("name")).toMatch(/^field-/);
    expect(field).not.toHaveAttribute("name", "Feed URL");
  });

  it("protects textareas and selects", () => {
    render(<><AutofillResistantTextarea aria-label="Reason"/><AutofillResistantSelect aria-label="Workspace"><option>One</option></AutofillResistantSelect></>);
    for (const field of [screen.getByLabelText("Reason"), screen.getByLabelText("Workspace")]) {
      expect(field).toHaveAttribute("autocomplete", "none");
      expect(field).toHaveAttribute("data-lpignore", "true");
    }
  });

  it.each([
    ["username", "text", "username", "inoreader-username"],
    ["current-password", "password", "current-password", "inoreader-current-password"],
    ["new-password", "password", "new-password", "inoreader-new-password"],
  ] as const)("maps the %s auth role", (role, type, autocomplete, name) => {
    render(<AuthField authRole={role} aria-label={role}/>);
    const field = screen.getByLabelText(role);
    expect(field).toHaveAttribute("type", type);
    expect(field).toHaveAttribute("autocomplete", autocomplete);
    expect(field).toHaveAttribute("name", name);
    expect(field).not.toHaveAttribute("data-1p-ignore");
  });

  it("retains entered content", () => {
    render(<AutofillResistantField aria-label="Name" />);
    fireEvent.input(screen.getByLabelText("Name"), { target: { value: "финансы" } });
    expect(screen.getByLabelText("Name")).toHaveValue("финансы");
  });

  it("gives password confirmation a stable distinct name",()=>{
    render(<><AuthField authRole="new-password" aria-label="New password"/><AuthField authRole="new-password" purpose="confirmation" aria-label="Confirmation"/></>);
    expect(screen.getByLabelText("New password")).toHaveAttribute("name","inoreader-new-password");
    expect(screen.getByLabelText("Confirmation")).toHaveAttribute("name","inoreader-new-password-confirmation");
    expect(screen.getByLabelText("Confirmation")).toHaveAttribute("autocomplete","new-password");
  });
});
