"""Tests for assets.py: URL extraction, host-auth guard, download."""

import os
import unittest
from unittest.mock import MagicMock

from active_collab.assets import (
    Asset,
    download_asset,
    extract_asset_urls,
    should_send_token,
)


class TestExtractAssetUrls(unittest.TestCase):
    def _task(self, body: str = "", attachments: list | None = None) -> dict:
        result: dict = {"body": body}
        if attachments is not None:
            result["attachments"] = attachments
        return result

    def test_img_tag_in_body_extracted_as_image(self) -> None:
        task = self._task(body='<img src="https://example.com/photo.jpg"/>')
        assets = extract_asset_urls(task, [])
        self.assertEqual(len(assets), 1)
        self.assertEqual(assets[0].url, "https://example.com/photo.jpg")
        self.assertEqual(assets[0].kind, "image")

    def test_href_tag_in_body_extracted_as_link(self) -> None:
        task = self._task(body='<a href="https://example.com/doc.pdf">doc</a>')
        assets = extract_asset_urls(task, [])
        self.assertEqual(len(assets), 1)
        self.assertEqual(assets[0].kind, "link")

    def test_href_with_image_extension_classified_as_image(self) -> None:
        task = self._task(
            body='<a href="https://example.com/banner.png">img</a>'
        )
        assets = extract_asset_urls(task, [])
        self.assertEqual(assets[0].kind, "image")

    def test_comment_body_parsed_for_assets(self) -> None:
        task = self._task()
        comments = [{"body": '<img src="https://cdn.example.com/thumb.gif"/>'}]
        assets = extract_asset_urls(task, comments)
        self.assertEqual(len(assets), 1)
        self.assertEqual(assets[0].url, "https://cdn.example.com/thumb.gif")
        self.assertEqual(assets[0].kind, "image")

    def test_attachments_list_included(self) -> None:
        task = self._task(
            attachments=[{
                "name": "report.pdf",
                "url": "https://example.com/files/report.pdf",
            }]
        )
        assets = extract_asset_urls(task, [])
        self.assertEqual(len(assets), 1)
        self.assertEqual(assets[0].name, "report.pdf")
        self.assertEqual(assets[0].kind, "attachment")

    def test_attachment_with_image_extension_is_image(self) -> None:
        task = self._task(
            attachments=[{
                "name": "screenshot.png",
                "url": "https://example.com/files/screenshot.png",
            }]
        )
        assets = extract_asset_urls(task, [])
        self.assertEqual(assets[0].kind, "image")

    def test_deduplication_preserves_first_seen_order(self) -> None:
        url = "https://example.com/image.jpg"
        task = self._task(
            body=f'<img src="{url}"/><img src="{url}"/>',
            attachments=[{"name": "image.jpg", "url": url}],
        )
        assets = extract_asset_urls(task, [])
        urls = [a.url for a in assets]
        self.assertEqual(urls.count(url), 1)
        self.assertEqual(urls[0], url)

    def test_empty_body_and_no_comments_returns_empty(self) -> None:
        task = self._task()
        self.assertEqual(extract_asset_urls(task, []), [])

    def test_missing_body_field_tolerated(self) -> None:
        assets = extract_asset_urls({}, [])
        self.assertEqual(assets, [])

    def test_missing_attachments_field_tolerated(self) -> None:
        task = self._task(body='<img src="https://example.com/x.jpg"/>')
        assets = extract_asset_urls(task, [])
        self.assertEqual(len(assets), 1)

    def test_empty_comments_list_tolerated(self) -> None:
        task = self._task(
            body='<a href="https://example.com/file.zip">file</a>'
        )
        assets = extract_asset_urls(task, [])
        self.assertEqual(len(assets), 1)

    def test_assets_from_multiple_comments_combined(self) -> None:
        task = self._task()
        comments = [
            {"body": '<img src="https://a.com/1.jpg"/>'},
            {"body": '<img src="https://b.com/2.jpg"/>'},
        ]
        assets = extract_asset_urls(task, comments)
        self.assertEqual(len(assets), 2)

    def test_body_and_comments_and_attachments_combined(self) -> None:
        task = self._task(
            body='<img src="https://example.com/body.jpg"/>',
            attachments=[{
                "name": "file.pdf",
                "url": "https://example.com/file.pdf",
            }],
        )
        comments = [{
            "body": '<a href="https://example.com/linked.html">link</a>',
        }]
        assets = extract_asset_urls(task, comments)
        urls = [a.url for a in assets]
        self.assertIn("https://example.com/body.jpg", urls)
        self.assertIn("https://example.com/file.pdf", urls)
        self.assertIn("https://example.com/linked.html", urls)

    def test_body_parsed_before_stripping_retains_html_urls(self) -> None:
        task = self._task(
            body='<p>See <img src="https://example.com/raw.png"/></p>'
        )
        assets = extract_asset_urls(task, [])
        self.assertEqual(len(assets), 1)
        self.assertEqual(assets[0].url, "https://example.com/raw.png")

    def test_none_comments_tolerated(self) -> None:
        task = self._task(body='<img src="https://example.com/img.jpg"/>')
        assets = extract_asset_urls(task, None)
        self.assertEqual(len(assets), 1)


class TestShouldSendToken(unittest.TestCase):
    BASE = "https://collab.myorg.com"

    def test_same_scheme_and_host_returns_true(self) -> None:
        url = "https://collab.myorg.com/api/v1/file"
        self.assertTrue(should_send_token(url, self.BASE))

    def test_same_host_with_path_returns_true(self) -> None:
        url = "https://collab.myorg.com/projects/1/tasks/2"
        self.assertTrue(should_send_token(url, self.BASE))

    def test_different_host_returns_false(self) -> None:
        self.assertFalse(
            should_send_token("https://cdn.example.com/image.jpg", self.BASE)
        )

    def test_different_scheme_returns_false(self) -> None:
        self.assertFalse(
            should_send_token("http://collab.myorg.com/file", self.BASE)
        )

    def test_subdomain_of_base_host_returns_false(self) -> None:
        url = "https://assets.collab.myorg.com/img.jpg"
        self.assertFalse(should_send_token(url, self.BASE))

    def test_base_as_subdomain_of_asset_returns_false(self) -> None:
        self.assertFalse(
            should_send_token(
                "https://myorg.com/file.jpg", "https://collab.myorg.com"
            )
        )

    def test_host_match_is_case_insensitive(self) -> None:
        self.assertTrue(
            should_send_token(
                "https://COLLAB.MYORG.COM/file", "https://collab.myorg.com"
            )
        )


class TestDownloadAsset(unittest.TestCase):
    def _make_http(
        self, status: int = 200, body: bytes = b"data"
    ) -> MagicMock:
        http = MagicMock()
        http.get.return_value = (status, body)
        return http

    def test_downloads_file_and_returns_path(self, tmp_path=None) -> None:
        import tempfile
        with tempfile.TemporaryDirectory() as tmp_dir:
            asset = Asset(
                name="photo.jpg",
                url="https://collab.myorg.com/photo.jpg",
                kind="image",
            )
            http = self._make_http(body=b"imgdata")
            path = download_asset(
                http, asset, "https://collab.myorg.com", "token123", tmp_dir
            )
            self.assertEqual(path, os.path.join(tmp_dir, "photo.jpg"))
            with open(path, "rb") as fh:
                self.assertEqual(fh.read(), b"imgdata")

    def test_token_attached_when_same_host(self) -> None:
        import tempfile
        with tempfile.TemporaryDirectory() as tmp_dir:
            asset = Asset(
                name="file.pdf",
                url="https://collab.myorg.com/file.pdf",
                kind="attachment",
            )
            http = self._make_http()
            download_asset(
                http, asset,
                "https://collab.myorg.com", "secret_token", tmp_dir,
            )
            pos_args = http.get.call_args[0]
            headers = (
                pos_args[1]
                if len(pos_args) > 1
                else http.get.call_args[1].get("headers", {})
            )
            self.assertIn("X-Angie-AuthApiToken", headers)
            self.assertEqual(
                headers["X-Angie-AuthApiToken"], "secret_token"
            )

    def test_token_not_attached_for_foreign_host(self) -> None:
        import tempfile
        with tempfile.TemporaryDirectory() as tmp_dir:
            asset = Asset(
                name="image.png",
                url="https://cdn.thirdparty.com/image.png",
                kind="image",
            )
            http = self._make_http()
            download_asset(
                http, asset,
                "https://collab.myorg.com", "secret_token", tmp_dir,
            )
            pos_args = http.get.call_args[0]
            headers = pos_args[1] if len(pos_args) > 1 else {}
            self.assertNotIn("X-Angie-AuthApiToken", headers)

    def test_raises_on_non_200_status(self) -> None:
        import tempfile
        with tempfile.TemporaryDirectory() as tmp_dir:
            asset = Asset(
                name="missing.jpg",
                url="https://collab.myorg.com/missing.jpg",
                kind="image",
            )
            http = self._make_http(status=404)
            with self.assertRaises(RuntimeError):
                download_asset(
                    http, asset, "https://collab.myorg.com", "token", tmp_dir
                )
