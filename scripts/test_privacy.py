"""Privacy checks must distinguish public attribution from private metadata."""
import unittest

from check_privacy import findings


class PrivacyTests(unittest.TestCase):
    def test_exact_contributor_no_reply_is_public(self):
        public = b"MX-Goliath <98145946+MX-Goliath@users.noreply.github.com>"
        self.assertEqual(findings("README.md", public), set())
        self.assertEqual(findings("commit", b"author " + public + b" 0 +0000\n"), set())

    def test_historical_service_identity_is_scoped_to_attribution(self):
        service = b"T3 Code <t3code@" + b"users.noreply.github.com>"
        self.assertEqual(findings(".mailmap", service), set())
        commit = b"author " + service + b" 0 +0000\ncommitter " + service + b" 0 +0000\n\nMessage\n"
        self.assertEqual(findings("commit", commit), set())
        self.assertIn("email address", findings("README.md", service))
        self.assertIn("email address", findings("commit", b"\n\nMessage mentions " + service))
        self.assertIn("email address", findings("commit", b"\n\nMessage\nauthor " + service))

    def test_other_addresses_are_still_rejected(self):
        for address in [b"other@" + b"users.noreply.github.com",
                        b"person@" + b"private.test"]:
            for path in ["README.md", ".mailmap", "commit"]:
                with self.subTest(path=path, address=address):
                    self.assertIn("email address", findings(path, b"author <" + address + b">"))

    def test_paths_credentials_and_unreviewed_designs_are_rejected(self):
        home_path = b"C:/" + b"Users/PrivatePerson/design"
        self.assertIn("personal home directory", findings("README.md", home_path))
        self.assertIn("personal home directory", findings("fixture.json", home_path.decode().encode("utf-16-le")))
        self.assertIn("possible credential", findings("README.md", b"ghp_" + b"x" * 30))
        self.assertIn("local credentials file", findings(".env", b""))
        self.assertIn("design outside reviewed synthetic fixtures", findings("private-projects/test.SchDoc", b""))


if __name__ == "__main__":
    unittest.main()
