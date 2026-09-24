"""发布约束隔离验证，不打包、不联网。"""
import unittest
from check_alpha_release import validate


class AlphaReleaseTests(unittest.TestCase):
    def config(self):
        return {'version': '0.1.0-alpha.1', 'bundle': {
            'createUpdaterArtifacts': False, 'macOS': {'signingIdentity': '-'}}, 'plugins': {}}

    def test_alpha(self):
        self.assertEqual(validate('v0.1.0-alpha.1', self.config()), [])

    def test_stable_and_mismatched_tags(self):
        for tag in ['v0.1.0', 'v0.1.0-beta.1', 'v0.1.0-alpha.2', 'v0.1.0-alpha.0']:
            self.assertTrue(validate(tag, self.config()))

    def test_rejects_updater_channel_and_artifacts(self):
        config = self.config()
        config['plugins']['updater'] = {}
        self.assertTrue(validate('v0.1.0-alpha.1', config))
        config = self.config()
        config['bundle']['createUpdaterArtifacts'] = True
        self.assertTrue(validate('v0.1.0-alpha.1', config))

    def test_missing_ad_hoc_identity(self):
        config = self.config()
        config['bundle']['macOS'] = {}
        self.assertTrue(validate('v0.1.0-alpha.1', config))
