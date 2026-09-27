import importlib.util
from pathlib import Path
import tempfile
import unittest
from decimal import Decimal
from concurrent.futures import ThreadPoolExecutor
import copy
import threading

spec = importlib.util.spec_from_file_location('experiment', Path(__file__).parents[1] / 'deepseek_experiment.py')
experiment = importlib.util.module_from_spec(spec)
spec.loader.exec_module(experiment)

class BudgetTests(unittest.TestCase):
    def test_thinking_parameters_are_explicit_and_do_not_mutate_input(self):
        original = {'messages': [{'role': 'user', 'content': 'example'}]}
        regular = experiment.prepare_payload(original, 'deepseek-flash', 4096)
        self.assertEqual(regular['thinking'], {'type': 'disabled'})
        self.assertEqual(regular['temperature'], 0.3)
        thinking = experiment.prepare_payload(original, 'deepseek-flash', 4096, 'high')
        self.assertEqual(thinking['thinking'], {'type': 'enabled'})
        self.assertEqual(thinking['reasoning_effort'], 'high')
        self.assertNotIn('temperature', thinking)
        self.assertEqual(set(original), {'messages'})
        with self.assertRaises(ValueError):
            experiment.prepare_payload(original, 'deepseek-flash', 4096, 'unknown')

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.budget = experiment.Budget(self.tmp.name)

    def test_reserves_final_twenty_and_refuses_overspend(self):
        for _ in range(2): self.budget.reserve('deepseek-v4-pro', 4096, 'development', 'dev')
        with self.assertRaises(ValueError): self.budget.reserve('deepseek-v4-pro', 4096, 'development', 'over')
        for _ in range(20): self.budget.reserve('deepseek-flash', 4096, 'final', 'heldout')
        with self.assertRaises(ValueError): self.budget.reserve('deepseek-flash', 4096, 'final', 'extra')
        with self.budget.locked() as data:
            self.assertLessEqual(sum(Decimal(e['reserved_usd']) for e in data['entries']), Decimal('10'))

    def test_exact_accounting_releases_only_known_remainder(self):
        op = self.budget.reserve('deepseek-flash', 4096, 'development', 'test')
        self.budget.settle(op, {'prompt_tokens': 1000, 'prompt_cache_hit_tokens': 500, 'completion_tokens': 100}, 'stop')
        with self.budget.locked() as data:
            self.assertEqual(data['entries'][0]['cost_usd'], '0.000273')
        with self.assertRaises(ValueError): self.budget.settle(op, {'prompt_tokens': 1, 'completion_tokens': 1}, 'stop')

    def test_uncertain_call_keeps_full_reservation(self):
        op = self.budget.reserve('deepseek-flash', 4096, 'development', 'test')
        self.budget.uncertain(op, 'timeout')
        with self.budget.locked() as data:
            self.assertNotIn('cost_usd', data['entries'][0])
            self.assertEqual(data['entries'][0]['status'], 'uncertain')

    def test_bad_usage_is_not_silently_coerced(self):
        for usage in [{'prompt_tokens': -1, 'completion_tokens': 2}, {'prompt_tokens': 1, 'completion_tokens': 0.5}, {'prompt_tokens': 1, 'completion_tokens': 2, 'prompt_cache_hit_tokens': 3}]:
            with self.assertRaises(ValueError): experiment.usage_cost('deepseek-flash', usage)

    def test_accounting_violation_halts_calls_and_survives_error_handler(self):
        for usage in ({'prompt_tokens': 1, 'completion_tokens': 4097}, {'prompt_tokens': -1, 'completion_tokens': 2}, {}):
            with tempfile.TemporaryDirectory() as directory:
                budget = experiment.Budget(directory)
                op = budget.reserve('deepseek-flash', 4096, 'development', 'bad')
                with self.assertRaises(ValueError): budget.settle(op, usage, 'stop')
                budget.uncertain(op, 'response_or_accounting_failure')
                with budget.locked() as data:
                    self.assertTrue(data['halted'])
                    self.assertEqual(data['entries'][0]['status'], 'accounting_violation')
                with self.assertRaises(ValueError): budget.reserve('deepseek-flash', 4096, 'development', 'forbidden')

    def test_later_disk_error_does_not_erase_settled_accounting(self):
        op = self.budget.reserve('deepseek-flash', 4096, 'development', 'settled')
        self.budget.settle(op, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        self.budget.uncertain(op, 'metadata_write_failed')
        with self.budget.locked() as data:
            self.assertEqual(data['entries'][0]['status'], 'settled')

    def test_new_closed_batch_preserves_old_results_and_original_cap(self):
        with self.assertRaises(ValueError): self.budget.protect_next_final_batch('second')
        for index in range(20):
            op = self.budget.reserve('deepseek-flash', 4096, 'final', str(index))
            self.budget.settle(op, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        self.budget.protect_next_final_batch('second')
        with self.budget.locked() as data:
            self.assertEqual(len(data['entries']), 20)
            self.assertTrue(all(e['status'] == 'settled' for e in data['entries']))
            self.assertEqual(data['protected_final_calls'], 20)
            self.assertEqual(data['cap_usd'], '10')
        with self.assertRaises(ValueError): self.budget.protect_next_final_batch('third')

    def test_new_closed_batch_cannot_hide_unknown_charges(self):
        for index in range(20):
            op = self.budget.reserve('deepseek-flash', 4096, 'final', str(index))
            self.budget.uncertain(op, 'timeout')
        with self.assertRaises(ValueError): self.budget.protect_next_final_batch('second')
        with self.budget.locked() as data:
            self.assertEqual(data['protected_final_calls'], 0)

    def sequential_plan(self, label='candidate02-pro'):
        self.budget.amend_final_plan_to_sequential(
            label, 'deepseek-v4-pro', 8192,
            'Preregister twenty target cases, stopping if a full per-call reservation cannot fit')

    def test_explicit_amendment_preserves_ledger_prices_and_prior_failed_batch(self):
        for index in range(20):
            op = self.budget.reserve('deepseek-flash', 4096, 'final', str(index))
            self.budget.settle(op, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        self.budget.protect_next_final_batch('candidate02-flash')
        unknown = self.budget.reserve('deepseek-v4-pro', 4096, 'development', 'unknown-diagnostic')
        self.budget.uncertain(unknown, 'transport_failure')
        with self.budget.locked() as data:
            previous = copy.deepcopy(data)
        self.sequential_plan()
        with self.budget.locked() as data:
            self.assertEqual(data['entries'], previous['entries'])
            self.assertEqual(data['prices'], previous['prices'])
            self.assertEqual(data['cap_usd'], '10')
            self.assertEqual(data['renewed_final_batches'], previous['renewed_final_batches'])
            self.assertEqual(data['protected_final_calls'], 0)
            self.assertEqual(data['final_plan']['calls'], 20)
            self.assertEqual(data['final_plan']['reserved_calls'], 0)
            amendment = data['final_plan_amendments'][0]
            self.assertEqual(amendment['previous_plan']['label'], 'candidate02-flash')
            self.assertFalse(amendment['completion_guaranteed'])
            self.assertEqual(amendment['per_call_upper_usd'], '1.41656064')
            self.assertEqual(Decimal(amendment['committed_usd_at_amendment']),
                sum((Decimal(e.get('cost_usd', e['reserved_usd'])) for e in previous['entries']), Decimal(0)))

    def test_amendment_requires_untouched_batch_no_inflight_and_unique_plan(self):
        development = self.budget.reserve('deepseek-flash', 4096, 'development', 'running')
        with self.assertRaises(ValueError): self.sequential_plan()
        self.budget.settle(development, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        final = self.budget.reserve('deepseek-flash', 4096, 'final', 'started-final')
        with self.assertRaises(ValueError): self.sequential_plan()
        self.budget.settle(final, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        with self.assertRaises(ValueError): self.sequential_plan()
        with self.budget.locked() as data:
            self.assertNotIn('final_plan', data)
            self.assertEqual(data['protected_final_calls'], 19)

        with tempfile.TemporaryDirectory() as directory:
            budget = experiment.Budget(directory)
            budget.amend_final_plan_to_sequential('plan', 'deepseek-v4-pro', 8192, 'reason')
            with budget.locked() as data: previous = copy.deepcopy(data)
            with self.assertRaises(ValueError):
                budget.amend_final_plan_to_sequential('plan', 'deepseek-v4-pro', 8192, 'duplicate')
            with self.assertRaises(ValueError):
                budget.amend_final_plan_to_sequential('different', 'deepseek-flash', 4096, 'silent replacement')
            with self.assertRaises(ValueError): budget.protect_next_final_batch('bypass')
            with budget.locked() as data: self.assertEqual(data, previous)

    def test_sequential_plan_enforces_model_output_slots_and_monotonic_counts(self):
        self.sequential_plan()
        for model, tokens in [('deepseek-flash', 8192), ('deepseek-v4-pro', 8193),
                              ('deepseek-v4-pro', 1.5), ('deepseek-v4-pro', True)]:
            with self.assertRaises(ValueError): self.budget.reserve(model, tokens, 'final', 'invalid')
        for index in range(1, 21):
            operation = self.budget.reserve('deepseek-v4-pro', 8192, 'final', f'case-{index}')
            with self.budget.locked() as data:
                self.assertEqual(data['final_plan']['reserved_calls'], index)
                self.assertEqual(data['entries'][-1]['final_plan_label'], 'candidate02-pro')
                self.assertEqual(data['entries'][-1]['final_call_number'], index)
            self.budget.settle(operation, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
            with self.budget.locked() as data:
                self.assertEqual(data['final_plan']['reserved_calls'], index)
        with self.assertRaises(ValueError): self.budget.reserve('deepseek-v4-pro', 8192, 'final', 'extra')
        with self.budget.locked() as data:
            self.assertEqual(len(data['entries']), 20)
            self.assertEqual(data['final_plan_amendments'][0]['new_plan']['reserved_calls'], 0)

    def test_started_renewed_batch_cannot_be_relabelled_and_accounting_halt_stays_halted(self):
        for index in range(20):
            op = self.budget.reserve('deepseek-flash', 4096, 'final', str(index))
            self.budget.settle(op, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        self.budget.protect_next_final_batch('candidate02-flash')
        first = self.budget.reserve('deepseek-flash', 4096, 'final', 'candidate02-first')
        self.budget.settle(first, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        with self.assertRaises(ValueError): self.sequential_plan()
        with self.budget.locked() as data: self.assertEqual(data, previous)
        with tempfile.TemporaryDirectory() as directory:
            budget = experiment.Budget(directory)
            op = budget.reserve('deepseek-flash', 4096, 'development', 'bad-accounting')
            with self.assertRaises(ValueError):
                budget.settle(op, {'prompt_tokens': 1, 'completion_tokens': 4097}, 'stop')
            with self.assertRaises(ValueError):
                budget.amend_final_plan_to_sequential('plan', 'deepseek-v4-pro', 8192, 'reason')
            with budget.locked() as data:
                self.assertTrue(data['halted'])
                self.assertNotIn('final_plan', data)
                self.assertEqual(data['entries'][0]['status'], 'accounting_violation')

    def test_sequential_unknown_calls_retain_full_bound_until_cap_exhaustion(self):
        self.sequential_plan()
        for index in range(7):
            operation = self.budget.reserve('deepseek-v4-pro', 8192, 'final', str(index))
            self.budget.uncertain(operation, 'transport_failure')
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        with self.assertRaises(ValueError): self.budget.reserve('deepseek-v4-pro', 8192, 'final', 'over-budget')
        with self.budget.locked() as data:
            self.assertEqual(data, previous)
            self.assertEqual(data['final_plan']['reserved_calls'], 7)
            self.assertTrue(all(e['status'] == 'uncertain' and 'cost_usd' not in e for e in data['entries']))
            committed = sum((Decimal(e['reserved_usd']) for e in data['entries']), Decimal(0))
            self.assertLessEqual(committed, Decimal('10'))
            self.assertGreater(committed + experiment.maximum_cost('deepseek-v4-pro', 8192), Decimal('10'))

    def test_sequential_final_concurrency_is_guarded_under_the_ledger_lock(self):
        self.sequential_plan()
        barrier = threading.Barrier(4)
        def attempt(index):
            budget = experiment.Budget(self.tmp.name)
            barrier.wait()
            try:
                return budget.reserve('deepseek-v4-pro', 8192, 'final', str(index))
            except ValueError:
                return None
        with ThreadPoolExecutor(max_workers=4) as executor:
            operations = list(executor.map(attempt, range(4)))
        accepted = [operation for operation in operations if operation is not None]
        self.assertEqual(len(accepted), 1)
        with self.budget.locked() as data:
            self.assertEqual(len(data['entries']), 1)
            self.assertEqual(data['final_plan']['reserved_calls'], 1)
        self.budget.settle(accepted[0], {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        self.budget.reserve('deepseek-v4-pro', 8192, 'final', 'second')

    def finish_sequential_slots(self, count=20, unknown_first=False):
        for index in range(count):
            operation = self.budget.reserve('deepseek-v4-pro', 8192, 'final', f'case-{index}')
            if unknown_first and index == 0:
                self.budget.uncertain(operation, 'transport_failure')
            else:
                self.budget.settle(operation, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')

    def next_sequential_plan(self, label='candidate03-feedback'):
        self.budget.declare_next_sequential_final_plan(
            label, 'deepseek-v4-pro', 8192, 'Paired arm frozen before opening either answer set', calls=20)

    def test_two_stage_plan_accounts_for_forty_requests_and_preserves_prior_history(self):
        self.sequential_plan()
        self.finish_sequential_slots()
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        self.budget.declare_next_sequential_final_plan(
            'two-stage', 'deepseek-v4-pro', 16384, 'Twenty articles, draft and review', calls=40)
        for index in range(40):
            operation = self.budget.reserve('deepseek-v4-pro', 16384, 'final', f'two-stage-{index}')
            with self.assertRaises(ValueError):
                self.budget.reserve('deepseek-v4-pro', 16384, 'final', 'concurrent')
            self.budget.settle(operation, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
            if index == 19:
                with self.assertRaises(ValueError): self.next_sequential_plan('premature-next')
        with self.assertRaises(ValueError):
            self.budget.reserve('deepseek-v4-pro', 16384, 'final', 'forty-first')
        with self.budget.locked() as data:
            self.assertEqual(data['entries'][:20], previous['entries'])
            self.assertEqual(data['final_plan']['reserved_calls'], 40)
            self.assertEqual(data['cap_usd'], '10')
        self.next_sequential_plan('after-two-stage')
        with self.budget.locked() as data:
            self.assertEqual([p['calls'] for p in data['final_plan_history']], [20, 40])

    def test_invalid_call_count_is_rejected_without_changing_the_ledger(self):
        self.sequential_plan()
        self.finish_sequential_slots()
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        for calls in (0, -1, True, 20.0, '40', None):
            with self.assertRaises(ValueError):
                self.budget.declare_next_sequential_final_plan(
                    'invalid-size', 'deepseek-v4-pro', 16384, 'reason', calls=calls)
        with self.budget.locked() as data: self.assertEqual(data, previous)

    def test_next_plan_preserves_completed_arm_unknown_costs_and_amendment_history(self):
        self.sequential_plan()
        self.finish_sequential_slots(unknown_first=True)
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        self.next_sequential_plan()
        with self.budget.locked() as data:
            self.assertEqual(data['entries'], previous['entries'])
            self.assertEqual(data['prices'], previous['prices'])
            self.assertEqual(data['cap_usd'], '10')
            self.assertEqual(data['protected_final_calls'], 0)
            self.assertEqual(data['final_plan_history'], [previous['final_plan']])
            self.assertEqual(data['final_plan_amendments'][:-1], previous['final_plan_amendments'])
            amendment = data['final_plan_amendments'][-1]
            committed = sum((Decimal(e.get('cost_usd', e['reserved_usd'])) for e in previous['entries']), Decimal(0))
            self.assertEqual(Decimal(amendment['committed_usd_at_amendment']), committed)
            self.assertEqual(amendment['previous_plan'], previous['final_plan'])
            self.assertFalse(amendment['completion_guaranteed'])
            self.assertEqual(data['final_plan']['reserved_calls'], 0)
            self.assertLessEqual(committed + experiment.maximum_cost('deepseek-v4-pro', 8192), Decimal('10'))
            self.assertGreater(committed + 20 * experiment.maximum_cost('deepseek-v4-pro', 8192), Decimal('10'))
        operation = self.budget.reserve('deepseek-v4-pro', 8192, 'final', 'paired-first')
        with self.assertRaises(ValueError): self.budget.reserve('deepseek-v4-pro', 8192, 'final', 'concurrent')
        self.budget.settle(operation, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        with self.budget.locked() as data:
            self.assertEqual(data['final_plan_history'][0]['reserved_calls'], 20)
            self.assertEqual(data['final_plan']['reserved_calls'], 1)
            self.assertEqual(data['final_plan_amendments'][-1]['new_plan']['reserved_calls'], 0)
            self.assertEqual(data['entries'][:-1], previous['entries'])
            self.assertEqual(data['entries'][-1]['final_plan_label'], 'candidate03-feedback')
            self.assertEqual(data['entries'][-1]['final_call_number'], 1)
            self.assertNotIn('cost_usd', data['entries'][0])

    def test_next_plan_requires_previous_twenty_terminal_slots_and_no_inflight_calls(self):
        with self.assertRaises(ValueError): self.next_sequential_plan()
        self.sequential_plan()
        for count in (0, 19):
            if count: self.finish_sequential_slots(count)
            with self.budget.locked() as data: previous = copy.deepcopy(data)
            with self.assertRaises(ValueError): self.next_sequential_plan()
            with self.budget.locked() as data: self.assertEqual(data, previous)
        last = self.budget.reserve('deepseek-v4-pro', 8192, 'final', 'last-running')
        with self.assertRaises(ValueError): self.next_sequential_plan()
        self.budget.settle(last, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        development = self.budget.reserve('deepseek-flash', 4096, 'development', 'still-running')
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        with self.assertRaises(ValueError): self.next_sequential_plan()
        with self.budget.locked() as data: self.assertEqual(data, previous)
        self.budget.settle(development, {'prompt_tokens': 1000, 'completion_tokens': 100}, 'stop')
        self.next_sequential_plan()

    def test_next_plan_rejects_duplicate_labels_across_archived_plans(self):
        self.sequential_plan()
        self.finish_sequential_slots()
        with self.assertRaises(ValueError): self.next_sequential_plan('candidate02-pro')
        self.next_sequential_plan()
        with self.assertRaises(ValueError): self.next_sequential_plan()
        self.finish_sequential_slots()
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        for label in ('candidate02-pro', 'candidate03-feedback'):
            with self.assertRaises(ValueError): self.next_sequential_plan(label)
        with self.budget.locked() as data: self.assertEqual(data, previous)
        self.next_sequential_plan('candidate04-explicit')
        with self.budget.locked() as data:
            self.assertEqual([p['reserved_calls'] for p in data['final_plan_history']], [20, 20])
            self.assertEqual(len(data['final_plan_amendments']), 3)
            self.assertEqual(data['entries'], previous['entries'])

    def test_next_plan_refuses_exhausted_cap_without_releasing_unknown_charges(self):
        self.sequential_plan()
        self.finish_sequential_slots()
        for index in range(7):
            operation = self.budget.reserve('deepseek-v4-pro', 8192, 'development', f'unknown-{index}')
            self.budget.uncertain(operation, 'timeout')
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        with self.assertRaises(ValueError): self.next_sequential_plan()
        with self.budget.locked() as data:
            self.assertEqual(data, previous)
            self.assertNotIn('final_plan_history', data)
            committed = sum((Decimal(e.get('cost_usd', e['reserved_usd'])) for e in data['entries']), Decimal(0))
            self.assertLessEqual(committed, Decimal('10'))
            self.assertGreater(committed + experiment.maximum_cost('deepseek-v4-pro', 8192), Decimal('10'))

    def test_next_plan_rejects_invalid_metadata_accounting_halt_and_inconsistent_history(self):
        self.sequential_plan()
        self.finish_sequential_slots()
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        for label, model, output, reason in [('', 'deepseek-v4-pro', 8192, 'reason'),
                ('next', 'deepseek-v4-pro', 8192, ''), ('next', 'unknown', 8192, 'reason'),
                ('next', 'deepseek-v4-pro', 0, 'reason'), ('next', 'deepseek-v4-pro', True, 'reason')]:
            with self.assertRaises(ValueError):
                self.budget.declare_next_sequential_final_plan(label, model, output, reason, calls=20)
        with self.budget.locked() as data:
            self.assertEqual(data, previous)
            data['entries'][-1]['final_call_number'] = 19
        with self.assertRaises(ValueError): self.next_sequential_plan()
        with self.budget.locked() as data:
            data['entries'][-1]['final_call_number'] = 20
        bad = self.budget.reserve('deepseek-flash', 4096, 'development', 'bad-accounting')
        with self.assertRaises(ValueError): self.budget.settle(bad, {'prompt_tokens': 1, 'completion_tokens': 4097}, 'stop')
        with self.budget.locked() as data: previous = copy.deepcopy(data)
        with self.assertRaises(ValueError): self.next_sequential_plan()
        with self.budget.locked() as data: self.assertEqual(data, previous)

    def test_bad_amendment_metadata_never_changes_policy(self):
        for label, model, output, reason in [('', 'deepseek-v4-pro', 8192, 'reason'),
                ('plan', 'deepseek-v4-pro', 8192, ''), ('plan', 'unknown', 8192, 'reason'),
                ('plan', 'deepseek-v4-pro', 0, 'reason'), ('plan', 'deepseek-v4-pro', True, 'reason')]:
            with self.assertRaises(ValueError):
                self.budget.amend_final_plan_to_sequential(label, model, output, reason)
        with self.budget.locked() as data:
            self.assertNotIn('final_plan', data)
            self.assertEqual(data['protected_final_calls'], 20)

if __name__ == '__main__': unittest.main()
