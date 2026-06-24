import os
import re
import urllib.parse
from dataclasses import dataclass
from typing import Literal

from active_collab.http import HttpClient

_IMG_SRC_PATTERN = re.compile(
    r'<img\b[^>]*\bsrc=["\']([^"\']+)["\']', re.IGNORECASE
)
_HREF_PATTERN = re.compile(
    r'<a\b[^>]*\bhref=["\']([^"\']+)["\']', re.IGNORECASE
)

_IMAGE_EXTENSIONS = frozenset(
    {
        ".jpg", ".jpeg", ".png", ".gif", ".webp",
        ".svg", ".bmp", ".ico", ".tiff", ".avif",
    }
)

AssetKind = Literal["image", "attachment", "link"]


@dataclass(frozen=True)
class Asset:
    name: str
    url: str
    kind: AssetKind


def _classify_url(url: str, from_img_tag: bool) -> AssetKind:
    if from_img_tag:
        return "image"
    ext = os.path.splitext(urllib.parse.urlparse(url).path)[1].lower()
    if ext in _IMAGE_EXTENSIONS:
        return "image"
    return "link"


def _assets_from_html(html: str) -> list[Asset]:
    assets: list[Asset] = []
    for match in _IMG_SRC_PATTERN.finditer(html):
        url = match.group(1)
        name = os.path.basename(urllib.parse.urlparse(url).path) or url
        assets.append(Asset(name=name, url=url, kind="image"))
    for match in _HREF_PATTERN.finditer(html):
        url = match.group(1)
        name = os.path.basename(urllib.parse.urlparse(url).path) or url
        kind = _classify_url(url, from_img_tag=False)
        assets.append(Asset(name=name, url=url, kind=kind))
    return assets


def _assets_from_attachments(attachments: list) -> list[Asset]:
    assets: list[Asset] = []
    for att in attachments:
        url = att.get("url") or att.get("download_url") or ""
        if not url:
            continue
        name = (
            att.get("name")
            or os.path.basename(urllib.parse.urlparse(url).path)
            or url
        )
        ext = os.path.splitext(name)[1].lower()
        kind: AssetKind = "image" if ext in _IMAGE_EXTENSIONS else "attachment"
        assets.append(Asset(name=name, url=url, kind=kind))
    return assets


def extract_asset_urls(task: dict, comments: list) -> list[Asset]:
    """Extract image, attachment, and link assets from a task and comments.

    Parses raw HTML (before any stripping) for <img src> and <a href>,
    plus an optional task['attachments'] list. Deduplicates by URL,
    preserving first-seen order. Tolerates empty or absent fields.
    """
    seen: set[str] = set()
    result: list[Asset] = []

    def add(asset: Asset) -> None:
        if asset.url not in seen:
            seen.add(asset.url)
            result.append(asset)

    body_html = task.get("body") or ""
    for asset in _assets_from_html(body_html):
        add(asset)

    for comment in (comments or []):
        comment_html = comment.get("body") or ""
        for asset in _assets_from_html(comment_html):
            add(asset)

    for asset in _assets_from_attachments(task.get("attachments") or []):
        add(asset)

    return result


def should_send_token(asset_url: str, instance_base_url: str) -> bool:
    """Return True when asset_url's scheme AND host match instance_base_url.

    Any scheme or host mismatch returns False, preventing token leakage
    to third-party asset hosts.
    """
    parsed_asset = urllib.parse.urlparse(asset_url)
    parsed_base = urllib.parse.urlparse(instance_base_url)
    return (
        parsed_asset.scheme.lower() == parsed_base.scheme.lower()
        and parsed_asset.netloc.lower() == parsed_base.netloc.lower()
    )


def download_asset(
    http: HttpClient,
    asset: Asset,
    instance_base_url: str,
    token: str,
    dest_dir: str,
) -> str:
    """Download an asset to dest_dir and return the local file path.

    Attaches X-Angie-AuthApiToken only when the asset host matches the
    instance base URL (host-auth guard). Raises RuntimeError on non-200.
    """
    headers: dict = {}
    if should_send_token(asset.url, instance_base_url):
        headers["X-Angie-AuthApiToken"] = token

    status, body = http.get(asset.url, headers)
    if status != 200:
        raise RuntimeError(f"Download failed for {asset.url!r}: HTTP {status}")

    dest_path = os.path.join(dest_dir, asset.name)
    with open(dest_path, "wb") as fh:
        fh.write(body)
    return dest_path
