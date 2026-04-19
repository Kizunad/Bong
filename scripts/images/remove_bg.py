#!/usr/bin/env python3
"""
aiddup.com 去背工具
用法:
    python remove_bg.py <输入图片> [输出路径]
    python remove_bg.py *.png            # 批量处理

需要先设置环境变量:
    export AIDDUP_SESSION="<__session cookie 的值>"

从浏览器 DevTools → Application → Cookies → aiddup.com → __session 复制。
"""

import os
import sys
import mimetypes
import urllib.request
import urllib.parse
import json
from pathlib import Path


SESSION_COOKIE = os.environ.get("AIDDUP_SESSION", "")
BASE_URL = "https://aiddup.com"


def _json_post(url: str, payload: dict, extra_headers: dict | None = None) -> dict:
    body = json.dumps(payload).encode()
    headers = {
        "Content-Type": "application/json",
        "Cookie": f"__session={SESSION_COOKIE}",
        "User-Agent": "Mozilla/5.0",
        "Origin": BASE_URL,
        "Referer": f"{BASE_URL}/playground",
    }
    if extra_headers:
        headers.update(extra_headers)
    req = urllib.request.Request(url, data=body, headers=headers, method="POST")
    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read())


def get_presigned(file_name: str, file_size: int, content_type: str) -> dict:
    return _json_post(
        f"{BASE_URL}/api/agent/presigned",
        {"fileName": file_name, "fileSize": file_size, "contentType": content_type},
    )


def upload_to_oss(upload_url: str, oss_headers: dict, image_bytes: bytes, content_type: str) -> None:
    headers = {k: v for k, v in oss_headers.items()}
    headers["Content-Type"] = content_type
    req = urllib.request.Request(upload_url, data=image_bytes, headers=headers, method="PUT")
    with urllib.request.urlopen(req) as resp:
        if resp.status not in (200, 204):
            raise RuntimeError(f"OSS upload failed: {resp.status}")


def remove_background(source_url: str, file_name: str, content_type: str) -> str:
    result = _json_post(
        f"{BASE_URL}/api/remove-background",
        {"sourceUrl": source_url, "fileName": file_name, "contentType": content_type},
    )
    return result["resultUrl"]


def download(url: str, dest: Path) -> None:
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req) as resp:
        dest.write_bytes(resp.read())


def process(input_path: str, output_path: str | None = None) -> Path:
    if not SESSION_COOKIE:
        print("错误: 未设置 AIDDUP_SESSION 环境变量")
        print("  export AIDDUP_SESSION='<从浏览器 Cookies 复制的值>'")
        sys.exit(1)

    src = Path(input_path)
    if not src.exists():
        raise FileNotFoundError(src)

    content_type = mimetypes.guess_type(src.name)[0] or "image/png"
    image_bytes = src.read_bytes()

    # 输出路径默认: input_nobg.png
    if output_path:
        out = Path(output_path)
    else:
        out = src.with_stem(src.stem + "_nobg").with_suffix(".png")

    print(f"[1/3] 获取上传凭证 ({src.name}, {len(image_bytes)//1024} KB)...")
    presigned = get_presigned(src.name, len(image_bytes), content_type)

    print(f"[2/3] 上传到 OSS...")
    upload_to_oss(presigned["uploadUrl"], presigned["headers"], image_bytes, content_type)

    print(f"[3/3] 去背处理中...")
    result_url = remove_background(presigned["finalUrl"], src.name, content_type)

    print(f"      下载结果 → {out}")
    download(result_url, out)

    print(f"完成: {out}")
    return out


def main():
    args = sys.argv[1:]
    if not args or args[0] in ("-h", "--help"):
        print(__doc__)
        sys.exit(0)

    # 批量模式: 多个文件
    paths = args if len(args) > 1 else [args[0]]
    for p in paths:
        try:
            process(p)
        except Exception as e:
            print(f"  失败 ({p}): {e}")


if __name__ == "__main__":
    main()
