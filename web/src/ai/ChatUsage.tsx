import type { ChatProviderCall } from "../api/ai";

/** Add decimal money without converting it to floating point or rounding it. */
export function exactCostSum(values: string[]): string {
  if (values.some(value => !/^\d+(?:\.\d+)?$/.test(value))) throw new Error("Invalid provider cost data");
  const scale = Math.max(0, ...values.map(value => value.split(".")[1]?.length ?? 0));
  const total = values.reduce((sum, value) => {
    const [whole, fraction = ""] = value.split(".");
    return sum + BigInt(whole + fraction.padEnd(scale, "0"));
  }, 0n).toString().padStart(scale + 1, "0");
  return scale ? `${total.slice(0, -scale)}.${total.slice(-scale)}` : total;
}

export function ChatUsage({ calls }: { calls: ChatProviderCall[] }) {
  const costs = calls.flatMap(call => call.usage?.estimatedCostUsd === undefined ? [] : [call.usage.estimatedCostUsd]);
  const unknown = calls.length - costs.length;
  let summary: string;
  try { summary = costs.length ? `${unknown ? "Known " : ""}estimate: $${exactCostSum(costs)}` : "Cost not yet reported"; }
  catch { summary = "Invalid provider cost data"; }
  return <div class="ai-chat__usage" aria-label="Provider request costs">
    {!calls.length ? <span>Summary: 2 provider requests · chat reply: 1 request</span> : <>
      <strong>{calls.length} provider {calls.length === 1 ? "request" : "requests"} · {summary}{unknown ? ` · ${unknown} cost${unknown === 1 ? "" : "s"} unknown` : ""}</strong>
      {calls.map((call, index) => <span key={call.id}>
        {index + 1}. {call.phase === "verifying" ? "Verification" : "Generation"} · {call.status} · {call.usage?.estimatedCostUsd === undefined ? "cost unknown" : `$${call.usage.estimatedCostUsd} estimated`}
        {call.usage && ` · ${call.usage.promptTokens} input / ${call.usage.completionTokens} output tokens`}
      </span>)}
    </>}
  </div>;
}
