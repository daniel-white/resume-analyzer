"""Stub ATS providers.

Each process listens on a private URL. The API worker POSTs the file there.
The stub waits a fuzzy amount of time, then POSTs the signed callback itself.
Nothing in the gateway routes to these ports.

    python3 -m providers --all

    ATS_PROVIDERS=greenline=http://127.0.0.1:8101,levr=http://127.0.0.1:8102,workdaye=http://127.0.0.1:8103,ash=http://127.0.0.1:8104

The request is the plugin's file plus the run the callback has to name.
The callback body is the contract: run id, parse, verdict.
`PLUGIN_SECRET` is the same value the API uses to verify the signature.
`ATS_DELAY=min,max` overrides the fuzzy band, in seconds.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import hmac
import io
import json
import os
import random
import re
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import TypedDict

PDF = "application/pdf"
DOCX = "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
MAX_FILE_BYTES = 5 * 1024 * 1024
BASE = 50

EMAIL = re.compile(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}")
MONTH = re.compile(r"\b(?:0?[1-9]|1[0-2])/(?:19|20)\d{2}\b")
YEAR = re.compile(r"\b(?:19|20)\d{2}\b")
COLS = re.compile(r"<w:cols\b[^>]*\bw:num=\"(\d+)\"")


class Section(TypedDict):
    heading: str
    body: str
    in_table: bool
    in_text_box: bool
    in_header: bool


class Profile(TypedDict):
    name: str | None
    email: str | None
    phone: str | None
    sections: list[Section]


class Layout(TypedDict):
    columns: int
    contact_in_header: bool
    has_tables: bool
    has_text_boxes: bool
    has_images: bool
    bullet_count: int
    dates: str


class ParseInput(TypedDict):
    profile: Profile
    layout: Layout
    layout_confidence: str


class Factor(TypedDict):
    label: str
    delta: int
    advice: str


class Verdict(TypedDict):
    kept: list[str]
    dropped: list[str]
    score: int
    summary: str
    factors: list[Factor]


class CallbackBody(TypedDict):
    run_id: str
    parse: ParseInput
    verdict: Verdict


@dataclass(frozen=True)
class Provider:
    id: str
    name: str
    port: int
    delay: tuple[float, float]


@dataclass(frozen=True)
class Read:
    name: str | None
    email: str | None
    phone: str | None
    sections: list[Section]
    columns: int
    contact_in_header: bool
    has_tables: bool
    has_text_boxes: bool
    has_images: bool
    bullet_count: int
    dates: str
    layout_confidence: str
    unreadable: bool


@dataclass(frozen=True)
class Job:
    run_id: str
    media_type: str
    file: bytes
    callback_url: str


FLEET = (
    Provider("greenline", "Greenline", 8101, (0.30, 1.10)),
    Provider("levr", "Levr", 8102, (0.55, 1.50)),
    Provider("workdaye", "Workdaye", 8103, (0.40, 1.90)),
    Provider("ash", "Ash", 8104, (0.80, 2.20)),
)


def provider(provider_id: str) -> Provider:
    for item in FLEET:
        if item.id == provider_id:
            return item
    raise KeyError(provider_id)


def secret() -> str:
    return os.environ.get("PLUGIN_SECRET", "spine-stub-secret")


def delay_band(item: Provider) -> tuple[float, float]:
    raw = os.environ.get("ATS_DELAY", "").strip()
    if not raw:
        return item.delay
    low, high = raw.split(",", 1)
    return float(low), float(high)


def sign(key: str, timestamp: int, body: bytes) -> str:
    digest = hmac.new(key.encode(), f"{timestamp}.".encode() + body, hashlib.sha256)
    return digest.hexdigest()


def empty_read(confidence: str, unreadable: bool) -> Read:
    return Read(
        name=None,
        email=None,
        phone=None,
        sections=[],
        columns=1,
        contact_in_header=False,
        has_tables=False,
        has_text_boxes=False,
        has_images=False,
        bullet_count=0,
        dates="absent",
        layout_confidence=confidence,
        unreadable=unreadable,
    )


def read_file(media_type: str, data: bytes) -> Read:
    if media_type == DOCX:
        parsed = read_docx(data)
        return parsed if parsed is not None else empty_read("structural", True)
    if media_type == PDF and data.startswith(b"%PDF"):
        return read_pdf(data)
    confidence = "structural" if media_type == DOCX else "inferred"
    return empty_read(confidence, True)


def read_pdf(data: bytes) -> Read:
    text = data.decode("latin-1", errors="ignore")
    parsed = empty_read("inferred", False)
    return Read(
        name=parsed.name,
        email=first_email(text),
        phone=None,
        sections=[],
        columns=1,
        contact_in_header=False,
        has_tables=False,
        has_text_boxes=False,
        has_images=False,
        bullet_count=0,
        dates=date_style(text),
        layout_confidence="inferred",
        unreadable=False,
    )


def read_docx(data: bytes) -> Read | None:
    try:
        archive = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile:
        return None
    body = ""
    header = ""
    for name in archive.namelist():
        xml = archive.read(name).decode("utf-8", errors="ignore")
        if name == "word/document.xml":
            body = xml
        elif name.startswith("word/header") or name.startswith("word/footer"):
            header += xml
    if not body:
        return None
    body_texts = texts(body)
    header_texts = texts(header)
    body_text = " ".join(body_texts)
    header_text = " ".join(header_texts)
    columns = 1
    found = COLS.search(body)
    if found:
        columns = max(1, int(found.group(1)))
    return Read(
        name=plausible_name(header_texts) or plausible_name(body_texts),
        email=first_email(body_text) or first_email(header_text),
        phone=None,
        sections=headings(body),
        columns=columns,
        contact_in_header=first_email(header_text) is not None,
        has_tables="<w:tbl" in body,
        has_text_boxes="<w:txbx" in body,
        has_images="<w:drawing" in body or "<a:blip" in body,
        bullet_count=body.count("<w:numPr"),
        dates=date_style(body_text),
        layout_confidence="structural",
        unreadable=False,
    )


def headings(body: str) -> list[Section]:
    found: list[Section] = []
    for paragraph in body.split("</w:p>"):
        if "Heading" not in paragraph:
            continue
        heading = " ".join(texts(paragraph)).strip()
        if heading:
            found.append(
                Section(
                    heading=heading,
                    body="",
                    in_table=False,
                    in_text_box=False,
                    in_header=False,
                )
            )
    return found


def texts(xml: str) -> list[str]:
    out: list[str] = []
    rest = xml
    while True:
        start = rest.find("<w:t")
        if start < 0:
            break
        open_end = rest.find(">", start)
        if open_end < 0:
            break
        close = rest.find("</w:t>", open_end)
        if close < 0:
            break
        text = rest[open_end + 1 : close].replace("&amp;", "&").strip()
        if text:
            out.append(text)
        rest = rest[close + 6 :]
    return out


def plausible_name(values: list[str]) -> str | None:
    for value in values:
        text = value.strip()
        if (
            " " in text
            and "@" not in text
            and len(text) < 60
            and any(char.isalpha() for char in text)
        ):
            return text
    return None


def first_email(text: str) -> str | None:
    found = EMAIL.search(text)
    return found.group(0) if found else None


def date_style(text: str) -> str:
    if MONTH.search(text):
        return "mm-yyyy"
    if YEAR.search(text):
        return "other"
    return "absent"


def as_parse(parsed: Read) -> ParseInput:
    return ParseInput(
        profile=Profile(
            name=parsed.name,
            email=parsed.email,
            phone=parsed.phone,
            sections=parsed.sections,
        ),
        layout=Layout(
            columns=parsed.columns,
            contact_in_header=parsed.contact_in_header,
            has_tables=parsed.has_tables,
            has_text_boxes=parsed.has_text_boxes,
            has_images=parsed.has_images,
            bullet_count=parsed.bullet_count,
            dates=parsed.dates,
        ),
        layout_confidence=parsed.layout_confidence,
    )


def factor(label: str, delta: int, advice: str) -> Factor:
    return Factor(label=label, delta=delta, advice=advice)


def verdict(kept: list[str], dropped: list[str], factors: list[Factor]) -> Verdict:
    score = max(0, min(100, BASE + sum(item["delta"] for item in factors)))
    return Verdict(
        kept=kept,
        dropped=dropped,
        score=score,
        summary=summarize(score, factors),
        factors=factors,
    )


def summarize(score: int, factors: list[Factor]) -> str:
    fixes = [item["advice"] for item in factors if item["delta"] < 0 and item["advice"]]
    helped = [item["advice"] for item in factors if item["delta"] > 0 and item["advice"]]
    text = f"Score {score} of 100."
    if not fixes and not helped:
        return text + " Nothing to change for this system."
    if fixes:
        text += " " + " ".join(fixes)
    if helped:
        text += " " + " ".join(helped)
    return text


def decide(provider_id: str, parsed: Read) -> Verdict:
    if parsed.unreadable:
        return verdict(
            [],
            ["file"],
            [factor("could not read file", -40, "Upload a PDF or DOCX the parser can open.")],
        )
    if provider_id == "greenline":
        return decide_greenline(parsed)
    if provider_id == "levr":
        return decide_levr(parsed)
    if provider_id == "workdaye":
        return decide_workdaye(parsed)
    if provider_id == "ash":
        return decide_ash(parsed)
    raise KeyError(provider_id)


def heading_factor(parsed: Read, kept: list[str], factors: list[Factor], delta: int) -> None:
    names = [section["heading"] for section in parsed.sections if section["heading"]]
    if not names:
        return
    kept.extend(names)
    factors.append(factor("standard headings", delta, "Keep the standard headings."))


def decide_greenline(parsed: Read) -> Verdict:
    kept: list[str] = []
    dropped: list[str] = []
    factors: list[Factor] = []
    if parsed.contact_in_header:
        dropped.append("header contact")
        factors.append(
            factor(
                "contact in header",
                -25,
                "Move the email and phone into the body, not the header.",
            )
        )
    elif parsed.email:
        kept.append("email")
        factors.append(factor("email in body", 15, "Keep the email in the body."))
    if parsed.has_tables:
        dropped.append("table")
        factors.append(factor("table", -20, "Replace the table with plain lines of text."))
    if parsed.has_text_boxes:
        dropped.append("text box")
        factors.append(factor("text box", -15, "Take the text out of the text box."))
    if parsed.columns >= 2:
        dropped.append("columns")
        factors.append(factor("two columns", -15, "Keep the page to one column."))
    heading_factor(parsed, kept, factors, 10)
    return verdict(kept, dropped, factors)


def decide_levr(parsed: Read) -> Verdict:
    kept: list[str] = []
    dropped: list[str] = []
    factors: list[Factor] = []
    if parsed.has_tables:
        dropped.append("skills")
        factors.append(
            factor(
                "skills not normalized",
                -15,
                "Write skills as exact tokens on their own lines.",
            )
        )
    if parsed.bullet_count >= 2:
        kept.append("experience bullets")
        factors.append(factor("bullet density", 20, "Keep the bullets. This parser counts them."))
    return verdict(kept, dropped, factors)


def decide_workdaye(parsed: Read) -> Verdict:
    kept: list[str] = []
    dropped: list[str] = []
    factors: list[Factor] = []
    if parsed.contact_in_header:
        dropped.append("header contact")
        factors.append(
            factor(
                "contact in header",
                -25,
                "Move the email and phone into the body, not the header.",
            )
        )
    elif parsed.email:
        kept.append("email")
        factors.append(factor("email in body", 15, "Keep the email in the body."))
    if parsed.has_tables:
        dropped.append("table")
        factors.append(factor("table", -20, "Replace the table with plain lines of text."))
    if parsed.has_text_boxes:
        dropped.append("text box")
        factors.append(factor("text box", -15, "Take the text out of the text box."))
    if parsed.columns >= 2:
        dropped.append("columns")
        factors.append(factor("two columns", -15, "Keep the page to one column."))
    if parsed.dates == "mm-yyyy":
        factors.append(factor("dates MM/YYYY", 10, "Keep dates in MM/YYYY."))
    else:
        dropped.append("dates")
        factors.append(factor("dates not MM/YYYY", -12, "Write dates as MM/YYYY."))
    heading_factor(parsed, kept, factors, 10)
    return verdict(kept, dropped, factors)


def decide_ash(parsed: Read) -> Verdict:
    kept: list[str] = []
    dropped: list[str] = []
    factors: list[Factor] = []
    if parsed.email:
        kept.append("email")
        factors.append(factor("email kept", 10, "Keep the email where it is."))
    if parsed.has_tables:
        kept.append("skills")
        factors.append(
            factor("skills normalized", 8, "Keep the skills list. This parser normalizes it.")
        )
    if parsed.has_images:
        dropped.append("image")
        factors.append(factor("image", -10, "Leave images off the page."))
    heading_factor(parsed, kept, factors, 5)
    return verdict(kept, dropped, factors)


def pause(item: Provider, run_id: str) -> float:
    low, high = delay_band(item)
    if high < low:
        low, high = high, low
    total = random.uniform(low, high)
    steps = ("opening the file", "reading layout", "scoring what survived")
    slice_s = total / len(steps)
    for step in steps:
        print(f"{item.id}: {run_id}: {step}", file=sys.stderr, flush=True)
        time.sleep(slice_s)
    print(f"{item.id}: {run_id}: finished in {total:.2f}s", file=sys.stderr, flush=True)
    return total


def deliver(item: Provider, job: Job, parsed: Read) -> None:
    body: CallbackBody = {
        "run_id": job.run_id,
        "parse": as_parse(parsed),
        "verdict": decide(item.id, parsed),
    }
    raw = json.dumps(body, separators=(",", ":")).encode()
    key = secret()
    last_error = "no attempt"
    for _ in range(3):
        timestamp = int(time.time())
        request = urllib.request.Request(
            job.callback_url,
            data=raw,
            method="POST",
            headers={
                "content-type": "application/json",
                "x-timestamp": str(timestamp),
                "x-signature": sign(key, timestamp, raw),
                "idempotency-key": job.run_id,
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                response.read()
            print(
                f"{item.id}: {job.run_id}: callback {response.status}",
                file=sys.stderr,
                flush=True,
            )
            return
        except urllib.error.HTTPError as err:
            err.read()
            if err.code == 200:
                return
            last_error = f"HTTP {err.code}"
        except urllib.error.URLError as err:
            last_error = str(err.reason)
        time.sleep(0.2)
    print(f"{item.id}: {job.run_id}: callback failed: {last_error}", file=sys.stderr, flush=True)


def run_job(item: Provider, job: Job) -> None:
    try:
        pause(item, job.run_id)
        deliver(item, job, read_file(job.media_type, job.file))
    except Exception as err:
        print(f"{item.id}: {job.run_id}: {err}", file=sys.stderr, flush=True)


def parse_job(item: Provider, payload: object) -> Job:
    if not isinstance(payload, dict):
        raise ValueError("body must be a json object")
    run_id = payload.get("run_id")
    file_value = payload.get("file")
    callback_url = payload.get("callback_url")
    if not isinstance(run_id, str) or not re.fullmatch(
        r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        run_id,
    ):
        raise ValueError("run_id must be a uuid")
    if not isinstance(callback_url, str):
        raise ValueError("callback_url is required")
    parsed = urllib.parse.urlsplit(callback_url)
    path = parsed.path.rstrip("/") or "/"
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        raise ValueError("callback url must be an http(s) url")
    if path != f"/api/callbacks/{item.id}":
        raise ValueError("callback url is not this provider's webhook")
    if not isinstance(file_value, dict):
        raise ValueError("file must be an object")
    media_type = file_value.get("media_type")
    encoded = file_value.get("bytes_base64")
    if not isinstance(media_type, str) or not media_type:
        raise ValueError("file.media_type is required")
    if not isinstance(encoded, str):
        raise ValueError("file.bytes_base64 is required")
    try:
        data = base64.b64decode(encoded, validate=True)
    except ValueError as err:
        raise ValueError("file.bytes_base64 is not base64") from err
    if len(data) > MAX_FILE_BYTES:
        raise ValueError("file is over the 5MB limit")
    return Job(run_id=run_id, media_type=media_type, file=data, callback_url=callback_url)


class ProviderServer(ThreadingHTTPServer):
    allow_reuse_address = True
    daemon_threads = True


def handler_for(item: Provider) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def do_GET(self) -> None:
            if urllib.parse.urlsplit(self.path).path != "/health":
                self.respond(404, b"not found")
                return
            body = json.dumps({"id": item.id}).encode()
            self.respond(200, body, "application/json")

        def do_POST(self) -> None:
            length = int(self.headers.get("content-length", "0") or "0")
            if length <= 0 or length > 8_000_000:
                self.respond(400, b"bad body")
                return
            raw = self.rfile.read(length)
            try:
                job = parse_job(item, json.loads(raw))
            except (ValueError, json.JSONDecodeError) as err:
                self.respond(400, str(err).encode())
                return
            threading.Thread(target=run_job, args=(item, job), daemon=True).start()
            self.respond(202, b'{"status":"accepted"}', "application/json")

        def respond(self, status: int, body: bytes, content_type: str = "text/plain") -> None:
            self.send_response(status)
            self.send_header("content-type", content_type)
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, fmt: str, *args: object) -> None:
            print(f"{item.id}: {self.address_string()} {fmt % args}", file=sys.stderr, flush=True)

    return Handler


def serve(item: Provider, bind: str, port: int) -> None:
    server = ProviderServer((bind, port), handler_for(item))
    print(
        f"{item.name} listening on http://{bind}:{port} (worker only, not routed)",
        file=sys.stderr,
        flush=True,
    )
    server.serve_forever()


def main(argv: list[str] | None = None) -> None:
    choices = [item.id for item in FLEET]
    parser = argparse.ArgumentParser(description="Private stub ATS providers.")
    parser.add_argument("provider", nargs="?", choices=choices)
    parser.add_argument("--all", action="store_true", help="listen for every provider")
    parser.add_argument("--bind", default="127.0.0.1")
    parser.add_argument("--port", type=int)
    args = parser.parse_args(argv)
    if args.all == bool(args.provider):
        parser.error("pass a provider id or --all")
    if args.bind not in {"127.0.0.1", "localhost", "::1"}:
        parser.error("refusing a non-loopback bind; these stubs are not public")
    if args.provider:
        item = provider(args.provider)
        serve(item, args.bind, args.port or item.port)
        return
    threads = [
        threading.Thread(target=serve, args=(item, args.bind, item.port), daemon=True)
        for item in FLEET
    ]
    for thread in threads:
        thread.start()
    assignments = ",".join(f"{item.id}=http://{args.bind}:{item.port}" for item in FLEET)
    print(f"ATS_PROVIDERS={assignments}", file=sys.stderr, flush=True)
    try:
        while True:
            time.sleep(3600)
    except KeyboardInterrupt:
        return
