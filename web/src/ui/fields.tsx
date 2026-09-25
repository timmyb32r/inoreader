import { useId } from "preact/hooks";
import type { ComponentChildren, JSX } from "preact";

type InputProps = Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "name" | "autoComplete">;
type TextareaProps = Omit<JSX.TextareaHTMLAttributes<HTMLTextAreaElement>, "name" | "autoComplete">;
type SelectProps = Omit<JSX.SelectHTMLAttributes<HTMLSelectElement>, "name" | "autoComplete"> & { children: ComponentChildren };

export function useOpaqueFieldName(): string {
  const id = useId();
  return `field-${id.replace(/[^a-zA-Z0-9_-]/g, "x")}`;
}

const ignoreManager = {
  "data-1p-ignore": "true",
  "data-lpignore": "true",
  "data-form-type": "other",
} as const;

export function AutofillResistantField(props: InputProps) {
  const name = useOpaqueFieldName();
  return <input {...props} name={name} autoComplete="none" {...ignoreManager} />;
}

export function AutofillResistantTextarea(props: TextareaProps) {
  const name = useOpaqueFieldName();
  return <textarea {...props} name={name} autoComplete="none" {...ignoreManager} />;
}

export function AutofillResistantSelect({ children, ...props }: SelectProps) {
  const name = useOpaqueFieldName();
  return <select {...props} name={name} autoComplete="none" {...ignoreManager}>{children}</select>;
}

export type AuthFieldRole = "username" | "current-password" | "new-password";
const authFields: Record<AuthFieldRole, { type: "text" | "password"; autoComplete: string; name: string }> = {
  username: { type: "text", autoComplete: "username", name: "inoreader-username" },
  "current-password": { type: "password", autoComplete: "current-password", name: "inoreader-current-password" },
  "new-password": { type: "password", autoComplete: "new-password", name: "inoreader-new-password" },
};

type AuthProps = Omit<InputProps, "type"> & ({ authRole: "new-password"; purpose?: "primary" | "confirmation" } | { authRole: "username" | "current-password"; purpose?: never });
export function AuthField({ authRole, purpose, ...props }: AuthProps) {
  const contract = authRole === "new-password" && purpose === "confirmation" ? { ...authFields[authRole], name: "inoreader-new-password-confirmation" } : authFields[authRole];
  return <input {...props} {...contract} />;
}
