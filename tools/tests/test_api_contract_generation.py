import importlib.util
import pathlib
import unittest
spec=importlib.util.spec_from_file_location('contracts',pathlib.Path(__file__).resolve().parents[1]/'generate_api_contracts.py')
contracts=importlib.util.module_from_spec(spec)
spec.loader.exec_module(contracts)
class ContractGenerationTests(unittest.TestCase):
    def test_flattened_state_retains_identity(self):
        result=contracts.ts({'type':'object','properties':{'id':{'type':'string'}},'required':['id'],'oneOf':[{'type':'object','properties':{'status':{'const':'queued'}}}]})
        self.assertIn('"id": string',result)
        self.assertIn(' & ',result)
        self.assertIn('"queued"',result)
    def test_unsupported_schema_fails_closed(self):
        with self.assertRaises(ValueError): contracts.ts({'mystery':True})
