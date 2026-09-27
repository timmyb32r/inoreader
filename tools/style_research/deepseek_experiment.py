"""Budgeted, reproducible research calls. Never used by the production application.

Each call reserves the *entire documented model context* at cache-miss peak rates,
plus bounded output. This intentionally avoids treating character heuristics as
proven token counts. Unknown provider outcomes retain their reservation. Twenty
Flash final-evaluation calls are initially protected before exploratory spending.
An explicit, recorded amendment can instead target twenty sequential final calls
under the same cap, without guaranteeing that all twenty can be funded.
"""
from __future__ import annotations
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
from decimal import Decimal
import fcntl
import json
import os
from pathlib import Path
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
import uuid

ROOT = Path(__file__).resolve().parents[2]
STATE = ROOT / '.inoreader-state/style-research/experiments'
# Official pricing verified 2026-09-26. Cache-miss peak rates are the upper rates.
PRICES = {
    'deepseek-flash': {'input': '0.3', 'output': '1.2', 'cached': '0.006'},
    'deepseek-v4-pro': {'input': '1.32', 'output': '3.96', 'cached': '0.044'},
}
CONTEXT_BOUND = 1_048_576
FINAL_OUTPUT_BOUND = 4096
SOURCE = 'https://api-docs.deepseek.com/quick_start/pricing/'


def now():
    return datetime.now(timezone.utc).isoformat()


def maximum_cost(model, max_tokens):
    if model not in PRICES or type(max_tokens) is not int or not 1 <= max_tokens <= 393216:
        raise ValueError('Unknown model or invalid output bound')
    rates = PRICES[model]
    return (Decimal(CONTEXT_BOUND) * Decimal(rates['input']) + Decimal(max_tokens) * Decimal(rates['output'])) / Decimal(1_000_000)


def usage_cost(model, usage):
    prompt = usage['prompt_tokens']
    output = usage['completion_tokens']
    cached = usage.get('prompt_cache_hit_tokens', 0)
    if any(type(x) is not int or x < 0 for x in (prompt, output, cached)) or cached > prompt:
        raise ValueError('Invalid provider token accounting')
    rates = PRICES[model]
    return (Decimal(prompt - cached) * Decimal(rates['input']) + Decimal(cached) * Decimal(rates['cached']) + Decimal(output) * Decimal(rates['output'])) / Decimal(1_000_000)


def write_json(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + '.tmp')
    with temporary.open('w') as handle:
        json.dump(data, handle, ensure_ascii=False, indent=2)
        handle.flush()
        os.fsync(handle.fileno())
    temporary.replace(path)


class Budget:
    def __init__(self, directory=STATE):
        self.directory = Path(directory)
        self.directory.mkdir(parents=True, exist_ok=True)
        self.path = self.directory / 'budget.json'

    @contextmanager
    def locked(self):
        with (self.directory / 'budget.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            data = json.loads(self.path.read_text()) if self.path.exists() else {
                'cap_usd': '10', 'protected_final_calls': 20, 'prices': PRICES,
                'pricing_source': SOURCE, 'prices_verified_at': '2026-09-26', 'entries': []}
            if data['prices'] != PRICES or Decimal(data['cap_usd']) != Decimal('10'):
                raise ValueError('Budget/pricing contract changed; explicit reconciliation required')
            try:
                yield data
            finally:
                write_json(self.path, data)
                fcntl.flock(lock, fcntl.LOCK_UN)

    def reserve(self, model, max_tokens, phase, label):
        upper = maximum_cost(model, max_tokens)
        with self.locked() as data:
            if data.get('halted'):
                raise ValueError('Research accounting requires explicit reconciliation before further calls')
            final_remaining = data['protected_final_calls']
            plan = data.get('final_plan')
            plan_call = None
            if phase == 'final':
                if plan is None:
                    if model != 'deepseek-flash' or max_tokens > FINAL_OUTPUT_BOUND or final_remaining < 1:
                        raise ValueError('Final evaluation differs from its reserved plan')
                    final_remaining -= 1
                else:
                    if plan['reservation_mode'] != 'sequential_per_call' or final_remaining != 0:
                        raise ValueError('Final budget plan is inconsistent; explicit reconciliation required')
                    if model != plan['model'] or max_tokens > plan['max_tokens'] or plan['reserved_calls'] >= plan['calls']:
                        raise ValueError('Final evaluation differs from its declared sequential plan')
                    if any(e['phase'] == 'final' and e['status'] == 'reserved' for e in data['entries']):
                        raise ValueError('A sequential final call is still in flight')
                    plan_call = plan['reserved_calls'] + 1
            committed = sum((Decimal(entry.get('cost_usd', entry['reserved_usd'])) for entry in data['entries']), Decimal(0))
            protected = final_remaining * maximum_cost('deepseek-flash', FINAL_OUTPUT_BOUND)
            if committed + upper + protected > Decimal(data['cap_usd']):
                raise ValueError('Research budget cannot safely cover this call and any protected final reservations')
            operation = str(uuid.uuid4())
            data['protected_final_calls'] = final_remaining
            entry = {'id': operation, 'phase': phase, 'label': label,
                'model': model, 'max_tokens': max_tokens, 'reserved_usd': str(upper),
                'status': 'reserved', 'created_at': now()}
            if plan_call is not None:
                entry.update(final_plan_label=plan['label'], final_call_number=plan_call)
                plan['reserved_calls'] = plan_call
            data['entries'].append(entry)
            return operation

    def amend_final_plan_to_sequential(self, label, model, max_tokens, reason):
        """Explicitly replace twenty untouched Flash slots with a named plan.

        Only the unused future batch protection is released. Past settled costs
        and unknown charges are immutable. The new plan targets twenty attempts,
        with at most one final request in flight; each reserves its full context
        and output bound. Funding all twenty is explicitly not guaranteed.
        Call only after joining exploratory workers; this method never calls a
        provider and refuses any still-reserved operation from any phase.
        """
        if not isinstance(label, str) or not label.strip() or not isinstance(reason, str) or not reason.strip():
            raise ValueError('A named final plan and explicit amendment reason are required')
        if type(max_tokens) is not int:
            raise ValueError('Final output bound must be an integer')
        upper = maximum_cost(model, max_tokens)
        with self.locked() as data:
            if data.get('halted'):
                raise ValueError('Research accounting requires explicit reconciliation before amending the final plan')
            amendments = data.get('final_plan_amendments', [])
            if data.get('final_plan') is not None or any(a['label'] == label for a in amendments):
                raise ValueError('A final-plan amendment already exists')
            if data['protected_final_calls'] != 20:
                raise ValueError('Only an untouched twenty-call final batch can be amended')
            if any(entry['status'] == 'reserved' for entry in data['entries']):
                raise ValueError('Join all in-flight research calls before amending the final plan')
            committed = sum((Decimal(e.get('cost_usd', e['reserved_usd'])) for e in data['entries']), Decimal(0))
            if committed + upper > Decimal(data['cap_usd']):
                raise ValueError('Cannot safely fund even one call under the amended final plan')
            batches = data.get('renewed_final_batches', [])
            previous = {'label': batches[-1]['label'] if batches else None,
                'reservation_mode': 'protected_batch', 'model': 'deepseek-flash',
                'max_tokens': FINAL_OUTPUT_BOUND, 'remaining_calls': 20,
                'reserved_upper_usd': str(20 * maximum_cost('deepseek-flash', FINAL_OUTPUT_BOUND))}
            plan = {'label': label, 'reservation_mode': 'sequential_per_call',
                'model': model, 'max_tokens': max_tokens, 'calls': 20,
                'reserved_calls': 0, 'created_at': now()}
            amendment = {'label': label, 'reason': reason, 'created_at': plan['created_at'],
                'previous_plan': previous, 'new_plan': dict(plan),
                'committed_usd_at_amendment': str(committed),
                'per_call_upper_usd': str(upper), 'completion_guaranteed': False}
            data['final_plan_amendments'] = [*amendments, amendment]
            data['final_plan'] = plan
            data['protected_final_calls'] = 0

    def declare_next_sequential_final_plan(self, label, model, max_tokens, reason, *, calls):
        """Declare an explicitly sized arm after the prior arm has finished.

        The caller freezes its scientific protocol before this explicit ledger
        amendment. All previous slots must have terminal outcomes; an
        uncertain outcome remains fully charged and is never replaced. Archive
        the completed plan and append amendment history without modifying past
        entries. Calls counts provider requests, not source articles: twenty
        two-stage summaries require forty calls. Only one next call must fit the
        original cap: completing the arm remains conditional. The existing
        per-call concurrency guard applies, regardless of the declared count.
        """
        if not isinstance(label, str) or not label.strip() or not isinstance(reason, str) or not reason.strip():
            raise ValueError('A named final plan and explicit amendment reason are required')
        if type(calls) is not int or calls < 1:
            raise ValueError('An explicit positive provider-call count is required')
        upper = maximum_cost(model, max_tokens)
        with self.locked() as data:
            if data.get('halted'):
                raise ValueError('Research accounting requires explicit reconciliation before declaring a final plan')
            previous = data.get('final_plan')
            if previous is None or previous['reservation_mode'] != 'sequential_per_call' or data['protected_final_calls'] != 0:
                raise ValueError('A previous sequential final plan is required')
            if any(entry['status'] == 'reserved' for entry in data['entries']):
                raise ValueError('Join all in-flight research calls before declaring the next final plan')
            entries = [e for e in data['entries'] if e.get('final_plan_label') == previous['label']]
            previous_calls = previous['calls']
            if (type(previous_calls) is not int or previous_calls < 1
                    or previous['reserved_calls'] != previous_calls or len(entries) != previous_calls
                    or {e.get('final_call_number') for e in entries} != set(range(1, previous_calls + 1))
                    or any(e['phase'] != 'final' or e['status'] not in ('settled', 'uncertain') for e in entries)):
                raise ValueError('All previous final attempts must have recorded terminal outcomes')
            history = data.get('final_plan_history', [])
            amendments = data.get('final_plan_amendments', [])
            named_plans = [previous, *history, *amendments, *data.get('renewed_final_batches', [])]
            if any(plan['label'] == label for plan in named_plans):
                raise ValueError('Final plan label already exists')
            committed = sum((Decimal(e.get('cost_usd', e['reserved_usd'])) for e in data['entries']), Decimal(0))
            if committed + upper > Decimal(data['cap_usd']):
                raise ValueError('Cannot safely fund even one call under the next final plan')
            plan = {'label': label, 'reservation_mode': 'sequential_per_call',
                'model': model, 'max_tokens': max_tokens, 'calls': calls,
                'reserved_calls': 0, 'created_at': now()}
            amendment = {'label': label, 'reason': reason, 'created_at': plan['created_at'],
                'previous_plan': dict(previous), 'new_plan': dict(plan),
                'committed_usd_at_amendment': str(committed),
                'per_call_upper_usd': str(upper), 'completion_guaranteed': False}
            data['final_plan_history'] = [*history, dict(previous)]
            data['final_plan_amendments'] = [*amendments, amendment]
            data['final_plan'] = plan

    def protect_next_final_batch(self, label):
        """Reserve another declared twenty-case test without erasing failed tests.

        Used only after a previous closed set has become tuning data. Unknown
        charges remain committed, and both fresh evaluation and all past calls
        must still fit the original ten-dollar ceiling.
        """
        if not isinstance(label, str) or not label.strip():
            raise ValueError('A named new evaluation batch is required')
        with self.locked() as data:
            if data.get('halted') or data['protected_final_calls'] != 0 or data.get('final_plan') is not None:
                raise ValueError('Existing reservations or accounting need resolution')
            batches = data.get('renewed_final_batches', [])
            if any(batch['label'] == label for batch in batches):
                raise ValueError('Evaluation batch already exists')
            committed = sum((Decimal(e.get('cost_usd', e['reserved_usd'])) for e in data['entries']), Decimal(0))
            upper = 20 * maximum_cost('deepseek-flash', FINAL_OUTPUT_BOUND)
            if committed + upper > Decimal(data['cap_usd']):
                raise ValueError('Cannot safely reserve another complete final evaluation')
            data['protected_final_calls'] = 20
            data['renewed_final_batches'] = [*batches, {'label': label, 'created_at': now(),
                'calls': 20, 'model': 'deepseek-flash', 'max_tokens': FINAL_OUTPUT_BOUND,
                'reserved_upper_usd': str(upper)}]

    def settle(self, operation, usage, outcome):
        with self.locked() as data:
            entry = next(e for e in data['entries'] if e['id'] == operation)
            if entry['status'] != 'reserved':
                raise ValueError('Attempt already resolved')
            try:
                cost = usage_cost(entry['model'], usage)
                violation = usage['prompt_tokens'] > CONTEXT_BOUND or usage['completion_tokens'] > entry['max_tokens'] or cost > Decimal(entry['reserved_usd'])
            except (KeyError, TypeError, ValueError):
                violation = True
            if violation:
                entry['status'] = 'accounting_violation'
                data['halted'] = True
                raise ValueError('Provider exceeded verified accounting contract; stop research')
            entry.update(status='settled', cost_usd=str(cost), usage=usage, outcome=outcome, finished_at=now())

    def uncertain(self, operation, reason):
        with self.locked() as data:
            entry = next(e for e in data['entries'] if e['id'] == operation)
            if entry['status'] == 'reserved':
                entry.update(status='uncertain', outcome=reason, finished_at=now())


def prepare_payload(payload, model, max_tokens, thinking_effort='none'):
    if set(payload) != {'messages'} or not payload['messages']:
        raise ValueError('Input must contain only a nonempty messages array')
    for message in payload['messages']:
        if set(message) != {'role', 'content'} or message['role'] not in ('system', 'user', 'assistant') or not isinstance(message['content'], str):
            raise ValueError('Invalid text-only message')
    maximum_cost(model, max_tokens)
    if thinking_effort not in ('none', 'low', 'high', 'max'):
        raise ValueError('Unsupported explicit thinking effort')
    payload = {**payload, 'model': model, 'max_tokens': max_tokens,
               'response_format': {'type': 'json_object'}, 'stream': False}
    if thinking_effort == 'none':
        payload.update(thinking={'type': 'disabled'}, temperature=0.3)
    else:
        # Official API ignores temperature in thinking mode. Do not misreport it
        # as an effective parameter. max_tokens also bounds reasoning tokens.
        payload.update(thinking={'type': 'enabled'}, reasoning_effort=thinking_effort)
    return payload


def run(input_path, model, max_tokens, phase, label, directory=STATE, timeout=180, thinking_effort='none'):
    payload = prepare_payload(json.loads(Path(input_path).read_text()), model, max_tokens, thinking_effort)
    key = (Path.home() / '.deepseek').read_text().strip()
    if not key or '\n' in key or '\r' in key:
        raise ValueError('Credential file must contain one token')
    budget = Budget(directory)
    operation = budget.reserve(model, max_tokens, phase, label)
    run_dir = budget.directory / operation
    run_dir.mkdir()
    write_json(run_dir / 'request.json', payload)
    started = time.monotonic()
    request = Request('https://api.deepseek.com/chat/completions', data=json.dumps(payload, ensure_ascii=False).encode(),
        headers={'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json'}, method='POST')
    try:
        with urlopen(request, timeout=timeout) as response:
            result = json.load(response)
        write_json(run_dir / 'response.json', result)
        outcome = result['choices'][0]['finish_reason']
        budget.settle(operation, result['usage'], outcome)
        write_json(run_dir / 'metadata.json', {'elapsed_seconds': time.monotonic() - started, 'completed_at': now(), 'label': label})
        # Print only identifiers and accounting, never prompts, keys or provider errors.
        return {'id': operation, 'finish_reason': outcome, 'usage': result['usage'], 'elapsed_seconds': round(time.monotonic() - started, 3)}
    except HTTPError as error:
        budget.uncertain(operation, f'http_{error.code}')
        return {'id': operation, 'error': f'http_{error.code}', 'reservation_retained': True}
    except (URLError, TimeoutError):
        budget.uncertain(operation, 'transport_failure')
        return {'id': operation, 'error': 'transport_failure', 'reservation_retained': True}
    except Exception:
        # No raw exception logging: an opaque provider error could reproduce input.
        budget.uncertain(operation, 'response_or_accounting_failure')
        return {'id': operation, 'error': 'response_or_accounting_failure', 'reservation_retained': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('input', type=Path)
    parser.add_argument('--model', choices=PRICES, default='deepseek-flash')
    parser.add_argument('--max-tokens', type=int, default=FINAL_OUTPUT_BOUND)
    parser.add_argument('--phase', choices=['development', 'validation', 'final', 'smoke'], default='development')
    parser.add_argument('--label', required=True)
    parser.add_argument('--thinking-effort', choices=['none', 'low', 'high', 'max'], default='none')
    args = parser.parse_args()
    print(json.dumps(run(args.input, args.model, args.max_tokens, args.phase, args.label, thinking_effort=args.thinking_effort)))
