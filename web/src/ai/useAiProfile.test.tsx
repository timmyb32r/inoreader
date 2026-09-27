import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/preact";
import { AiClient, type AiProfile } from "../api/ai";
import type { Transport } from "../api/client";
import { DeepSeekProfile } from "./DeepSeekProfile";
import { useAiProfile } from "./useAiProfile";

const enabled: AiProfile = { configured: true, enabled: true };
const disabled: AiProfile = { configured: false, enabled: false };
function Harness({
  client,
  account = "owner",
  visible = true,
}: {
  client: AiClient;
  account?: string;
  visible?: boolean;
}) {
  const state = useAiProfile(client, account);
  return (
    <>
      <output data-testid="availability">
        {String(state.profile?.enabled ?? false)}
      </output>
      <button onClick={() => state.update(account, enabled)}>
        Apply mutation
      </button>
      {visible && (
        <DeepSeekProfile
          key={account}
          client={client}
          onProfile={(value) => state.update(account, value)}
          completedGeneration={0}
        />
      )}
    </>
  );
}

it.each([false, true])(
  "publishes saved credentials after the pane closes only for its original account (switch=%s)",
  async (switchAccount) => {
    let finish!: (profile: AiProfile) => void;
    const transport: Transport = async <T,>(
      _path: string,
      init?: RequestInit,
    ) => {
      if (init?.method === "PUT")
        return (await new Promise<AiProfile>((resolve) => {
          finish = resolve;
        })) as T;
      return disabled as T;
    };
    const client = new AiClient(transport),
      view = render(<Harness client={client} />);
    await waitFor(() =>
      expect(screen.getByLabelText("DeepSeek API key")).toBeEnabled(),
    );
    fireEvent.input(screen.getByLabelText("DeepSeek API key"), {
      target: { value: "fixture-key" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save API key" }));
    await waitFor(() => expect(finish).toBeDefined());
    view.rerender(
      <Harness
        client={client}
        account={switchAccount ? "another-owner" : "owner"}
        visible={false}
      />,
    );
    await act(async () => {
      finish(enabled);
    });
    await waitFor(() =>
      expect(screen.getByTestId("availability")).toHaveTextContent(
        switchAccount ? "false" : "true",
      ),
    );
  },
);

it("does not let an older initial profile read overwrite a completed mutation", async () => {
  let finish!: (profile: AiProfile) => void;
  const transport: Transport = async <T,>() =>
    (await new Promise<AiProfile>((resolve) => {
      finish = resolve;
    })) as T;
  render(<Harness client={new AiClient(transport)} visible={false} />);
  await waitFor(() => expect(finish).toBeDefined());
  fireEvent.click(screen.getByRole("button", { name: "Apply mutation" }));
  expect(screen.getByTestId("availability")).toHaveTextContent("true");
  await act(async () => {
    finish(disabled);
  });
  expect(screen.getByTestId("availability")).toHaveTextContent("true");
});
