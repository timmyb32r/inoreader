import importlib.util
import pathlib
import unittest
spec = importlib.util.spec_from_file_location("contracts", pathlib.Path(__file__).parents[1] / "generate_api_contracts.py")
contracts = importlib.util.module_from_spec(spec)
spec.loader.exec_module(contracts)

class ContractVocabulary(unittest.TestCase):
    def test_unknown_constraint_fails_generation(self):
        for keyword in ("pattern", "minLength", "uniqueItems", "unevaluatedProperties"):
            with self.assertRaises(ValueError):
                contracts.validate_schema({"type": "object", "properties": {"value": {keyword: 1}}})
    def test_supported_nested_schema(self):
        contracts.validate_schema({"type": "array", "items": {"type": "integer", "minimum": 0}})
    def test_external_refs_fail(self):
        with self.assertRaises(ValueError):
            contracts.validate_schema({"$ref": "https://example.test/schema"})
