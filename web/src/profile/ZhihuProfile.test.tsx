import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import { ZhihuProfile } from "./ZhihuProfile";
import { ZhihuClient } from "../api/zhihu";
import type { Transport } from "../api/client";
import type { ResponseContract, ResponseValue } from "../api/decode";
it("preserves pasted cookies, shows pending immediately, prevents duplicates and clears secrets on save", async () => {
  let resolve!: (value: unknown) => void;
  const mutations: RequestInit[] = [];
  const client = new ZhihuClient((async <C extends ResponseContract>(
    _path: string,
    init?: RequestInit,
  ) => {
    if (init?.method === "PUT") {
      mutations.push(init);
      return (await new Promise<unknown>(
        (r) => (resolve = r),
      )) as ResponseValue<C>;
    }
    return { available: true, configured: false } as ResponseValue<C>;
  }) as Transport);
  render(<ZhihuProfile client={client} />);
  const field = screen.getByLabelText("Zhihu cookies");
  await waitFor(() => expect(field).toBeEnabled());
  const raw = "z_c0\tEXACT==\t.zhihu.com\t/\t1\t2\t3\t4\t5\t6\t7\t8";
  fireEvent.input(field, { target: { value: raw } });
  const save = screen.getByRole("button", { name: "Save" });
  fireEvent.click(save);
  fireEvent.click(save);
  expect(save).toHaveAttribute("aria-busy", "true");
  expect(save).toBeDisabled();
  expect(mutations).toHaveLength(1);
  expect(JSON.parse(mutations[0].body as string).cookies).toBe(raw);
  resolve({ available: true, configured: true });
  await waitFor(() => expect(save).toHaveAttribute("aria-busy", "false"));
  expect(field).toHaveValue("");
  expect(screen.getByText("Session saved")).toBeInTheDocument();
});
it("requires explicit removal confirmation and preserves credentials on failed checks", async () => {
  const methods: string[] = [];
  const client = new ZhihuClient((async <C extends ResponseContract>(
    _path: string,
    init?: RequestInit,
  ) => {
    if (init?.method) {
      methods.push(init.method);
      if (init.method === "POST") throw new Error("Session rejected");
    }
    return {
      available: true,
      configured: init?.method !== "DELETE",
    } as ResponseValue<C>;
  }) as Transport);
  render(<ZhihuProfile client={client} />);
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Check" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Check" }));
  await screen.findByText("Session rejected");
  expect(screen.getByText("Session saved")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Remove" }));
  expect(methods).toEqual(["POST"]);
  fireEvent.click(screen.getByRole("button", { name: "Confirm removal" }));
  await screen.findByText("Session removed. Collected articles kept.");
  expect(methods).toEqual(["POST", "DELETE"]);
});
