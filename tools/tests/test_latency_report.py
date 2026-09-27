import importlib.util
import pathlib
import unittest
spec = importlib.util.spec_from_file_location("latency", pathlib.Path(__file__).parents[1] / "latency_report.py")
latency = importlib.util.module_from_spec(spec)
spec.loader.exec_module(latency)
class LatencyReport(unittest.TestCase):
    def test_sql_pool_and_api_distributions_do_not_expose_payload(self):
        events = [
            {"target":"sqlx::query", "message":"SELECT secret elapsed=1.5ms"},
            {"target":"sqlx::pool::acquire", "message":"acquired connection aquired_after_secs=0.001"},
            {"target":"reader_server::api_request", "message":"api_request operation=/api/articles/{id} elapsed_ms=2"},
            {"target":"reader_stage", "message":"stage=translation_provider outcome=ok elapsed_us=10"},
            {"target":"ai", "message":"ai_claim class=chat job_id=private queue_wait_us=5"},
        ]
        report = latency.report(events)
        self.assertEqual(report["postgres:statement"]["p95_upper_ms"], 2.048)
        self.assertEqual(report["postgres:acquire"]["count"], 1)
        self.assertNotIn("secret", str(report))
        self.assertNotIn("private", str(report))
    def test_percentile_and_zero(self):
        data = [{"target":"reader_stage", "message":f"stage=chat_provider elapsed_us={v}"} for v in range(100)]
        self.assertEqual(latency.report(data)["stage:chat_provider"]["p95_upper_ms"], .128)
