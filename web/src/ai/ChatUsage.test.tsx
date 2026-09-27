import { render, screen } from "@testing-library/preact";
import type { ChatProviderCall } from "../api/ai";
import { ChatUsage, exactCostSum } from "./ChatUsage";

const call = (id: string, cost?: string): ChatProviderCall => ({ id, assistantId: "summary", phase: id === "1" ? "generating" : "verifying", status: "completed", usage: { promptTokens: 1000, completionTokens: 200, promptCacheHitTokens: 300, promptCacheMissTokens: 700, estimatedCostUsd: cost } });

it("accounts for both requests and failed retry attempts without rounding decimal costs", () => {
  render(<ChatUsage calls={[call("1", "0.000000000000000001"), { ...call("2", "0.100000000000000009"), status: "failed" }, { ...call("3", "0.2"), assistantId: "retry" }]}/>);
  expect(screen.getByLabelText("Provider request costs")).toHaveTextContent("3 provider requests · estimate: $0.300000000000000010");
  expect(screen.getByText(/2\. Verification/)).toHaveTextContent("failed · $0.100000000000000009 estimated · 1000 input / 200 output tokens");
});

it("distinguishes missing cost and usage from a reported zero", () => {
  render(<ChatUsage calls={[call("1", "0.000"), call("2"), { ...call("3"), usage: undefined, status: "started" }]}/>);
  expect(screen.getByLabelText("Provider request costs")).toHaveTextContent("Known estimate: $0.000 · 2 costs unknown");
  expect(screen.getByText(/3\. Verification/)).toHaveTextContent("started · cost unknown");
  expect(screen.getByText(/3\. Verification/)).not.toHaveTextContent("tokens");
});

it("reports invalid money explicitly and never displays it as a zero estimate", () => {
  expect(() => exactCostSum(["1e-5"])).toThrow("Invalid provider cost data");
  render(<ChatUsage calls={[call("1", "NaN")]}/>);
  expect(screen.getByLabelText("Provider request costs")).toHaveTextContent("Invalid provider cost data");
});
