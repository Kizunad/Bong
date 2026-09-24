"""Regression for image gateways returning a signed CDN URL."""

import io
import json
import unittest
from unittest.mock import patch

import gen


class ImageDownloadTest(unittest.TestCase):
    def test_signed_asset_download_does_not_receive_generation_credentials(self):
        def response(body):
            result = io.BytesIO(body)
            result.status = 200
            return result

        def open_url(request, **kwargs):
            if request.full_url.endswith("/v1/images/generations"):
                self.assertEqual("Bearer test-api-key", request.get_header("Authorization"))
                return response(json.dumps({"data": [{"url": "https://cdn.example/asset.png?signature=test"}]}).encode())
            self.assertEqual("https://cdn.example/asset.png?signature=test", request.full_url)
            self.assertIsNone(request.get_header("Authorization"),
                              "CDN rejects API Bearer headers and must not receive the credential")
            return response(b"png-bytes")

        with patch.object(gen.urllib.request, "urlopen", side_effect=open_url):
            images = gen.generate_cliproxy_images("icon", "test-api-key", "https://api.example",
                                                  "gpt-image-2", "1024x1024", "high", "png",
                                                  "transparent", 1, "auto")
        self.assertEqual([b"png-bytes"], images)
