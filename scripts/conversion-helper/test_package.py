"""组件归档必须携带运行时和所有随包依赖的许可。"""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location("component_package", Path(__file__).with_name("package.py"))
component_package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(component_package)


class LicenseTests(unittest.TestCase):
    def test_british_license_names_and_native_runtime_notices_are_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            component_package.copy_licenses(root)
            dependencies = json.loads((root / "DEPENDENCIES.json").read_text(encoding="utf-8"))
            by_name = {item["name"].lower(): item for item in dependencies}
            for name in ("openpyxl", "et_xmlfile", "pyinstaller", "pypdfium2"):
                self.assertTrue(by_name[name]["licenseFiles"], name)
            self.assertTrue((root / "licenses/Python-LICENSE.txt").is_file())
            self.assertTrue(any("BUILD_LICENSES" in value for value in by_name["pypdfium2"]["licenseFiles"]))
            self.assertTrue(all(item["licenseFiles"] for item in dependencies))


if __name__ == "__main__":
    unittest.main()
