#!/usr/bin/env python3
"""
agy-doc-extract / doc_extract.py
Production-grade Vision-Language Document & Table Extractor for Aina & Antigravity (AGY)
Supports 9Router OpenAI-compatible VLM hub and CodeBuddy backends.

Key Capabilities:
- 0% Local CPU: Offloads heavy OCR & table recognition to high-performance VLM.
- High-fidelity Table Preservation: Complex borderless, nested, colspan/rowspan tables.
- Cross-Page Table Stitching: Automatically heals split tables across page breaks.
- Automatic CSV Export: Automatically converts extracted HTML tables to .csv files for WhatsApp sharing.
- Flexible CLI Flags: Page filtering, extraction modes, formats, DPI, concurrency, and custom prompts.
"""

import sys, os, time, json, base64, urllib.request, urllib.error, re, argparse, subprocess, tempfile, shutil, csv
from pathlib import Path
from html.parser import HTMLParser
from concurrent.futures import ThreadPoolExecutor, as_completed

DEFAULT_BASE_URL = os.environ.get("DOC_EXTRACT_BASE_URL", "https://router.dvlpid.my.id/v1")
DEFAULT_API_KEY = os.environ.get("DOC_EXTRACT_API_KEY", os.environ.get("CODEBUDDY_API_KEY", "sk-9router-master-key"))
DEFAULT_MODEL = os.environ.get("DOC_EXTRACT_MODEL", "cbai/minimax-m3")


class SimpleHTMLTableToCSV(HTMLParser):
    def __init__(self):
        super().__init__()
        self.rows = []
        self.current_row = []
        self.current_cell = []
        self.in_cell = False

    def handle_starttag(self, tag, attrs):
        if tag in ('td', 'th'):
            self.in_cell = True
            self.current_cell = []
        elif tag == 'tr':
            self.current_row = []

    def handle_endtag(self, tag):
        if tag in ('td', 'th'):
            self.in_cell = False
            cell_text = "".join(self.current_cell).strip()
            # Clean up whitespace
            cell_text = re.sub(r"\s+", " ", cell_text)
            self.current_row.append(cell_text)
        elif tag == 'tr':
            if self.current_row:
                self.rows.append(self.current_row)

    def handle_data(self, data):
        if self.in_cell:
            self.current_cell.append(data)

def html_table_to_csv_rows(html_code: str):
    parser = SimpleHTMLTableToCSV()
    parser.feed(html_code)
    return parser.rows

def find_markdown_tables(text: str) -> list:
    pattern = re.compile(
        r"((?:^[ \t]*\|.+?\|[ \t]*\r?\n)+(?:^[ \t]*\|[-:\s|]+?\|[ \t]*\r?\n)(?:^[ \t]*\|.+?\|[ \t]*(?:\r?\n|\Z))+)",
        re.MULTILINE
    )
    return [m.group(1).strip() for m in pattern.finditer(text)]

def markdown_table_to_csv_rows(md_table_text: str) -> list:
    rows = []
    lines = md_table_text.strip().splitlines()
    for line in lines:
        line = line.strip()
        if not line.startswith("|") or not line.endswith("|"):
            continue
        inner = line[1:-1].strip()
        if re.match(r"^[-:\s|]+$", inner):
            continue
        cells = [re.sub(r"\s+", " ", c.strip()) for c in line.split("|")[1:-1]]
        rows.append(cells)
    return rows

def extract_table_headers(table_str: str) -> list:
    """Extract list of lowercase normalized column headers from HTML or Markdown table."""
    if "<table" in table_str.lower():
        th_matches = re.findall(r"<th[^>]*>([\s\S]*?)</th>", table_str, re.IGNORECASE)
        if th_matches:
            return [re.sub(r"<[^>]+>", "", th).strip().lower() for th in th_matches]
        first_tr = re.search(r"<tr[^>]*>([\s\S]*?)</tr>", table_str, re.IGNORECASE)
        if first_tr:
            tds = re.findall(r"<td[^>]*>([\s\S]*?)</td>", first_tr.group(1), re.IGNORECASE)
            return [re.sub(r"<[^>]+>", "", td).strip().lower() for td in tds]
        return []
    lines = [ln.strip() for ln in table_str.strip().splitlines() if ln.strip().startswith("|")]
    if lines:
        return [c.strip().lower() for c in lines[0].split("|")[1:-1]]
    return []

def headers_match(h1: list, h2: list) -> bool:
    if not h1 or not h2 or len(h1) != len(h2):
        return False
    matches = sum(1 for a, b in zip(h1, h2) if a == b or a in b or b in a)
    return (matches / len(h1)) >= 0.75

def parse_page_range(pages_str: str, total_pages: int):
    if not pages_str or pages_str.strip().lower() == "all":
        return list(range(1, total_pages + 1))
    
    selected = set()
    parts = pages_str.split(",")
    for part in parts:
        part = part.strip()
        if "-" in part:
            sub = part.split("-")
            if len(sub) == 2 and sub[0].isdigit() and sub[1].isdigit():
                start, end = int(sub[0]), int(sub[1])
                for p in range(max(1, start), min(total_pages, end) + 1):
                    selected.add(p)
        elif part.isdigit():
            p = int(part)
            if 1 <= p <= total_pages:
                selected.add(p)
    return sorted(list(selected)) if selected else list(range(1, total_pages + 1))

def call_vlm_completion(base_url: str, api_key: str, model: str, img_b64: str, page_num: int, mode: str, custom_prompt: str, summary: bool) -> str:
    if "codebuddy.ai" in base_url and "/v2" in base_url:
        url = f"{base_url}/chat/completions"
    elif not base_url.endswith("/chat/completions"):
        url = f"{base_url}/chat/completions"
    else:
        url = base_url

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        "User-Agent": "Aina-Vision-Extractor/2026.9",
    }

    mode_instructions = {
        "auto": "Convert all document content, paragraphs, formulas, and tables into clean, readable Markdown. For any tables (including borderless, multi-line, or nested tables), output semantically valid HTML <table> with exact numbers, negative brackets, and colspan/rowspan.",
        "tables": "STRICT TABLE FOCUS: Extract ONLY the tables from this document image. Convert every table into semantically valid HTML <table> with exact numbers, headers, and cell values. Ignore surrounding conversational narrative.",
        "text": "STRICT TEXT FOCUS: Transcribe all paragraphs, bullet points, headers, and footnotes into clean Markdown. Convert tables into simplified Markdown lists.",
        "math": "STRICT STEM/MATH FOCUS: Transcribe formulas into clean LaTeX ($...$ or $$...$$), preserving exact indices, summations, fractions, and matrices along with accompanying text and tables.",
        "raw": "Output full OCR transcription faithfully preserving visual layout."
    }

    system_prompt = (
        f"You are Zerox Vision OCR & Document Intelligence. {mode_instructions.get(mode, mode_instructions['auto'])} "
        "Do not hallucinate numbers or names. Preserve reading order."
    )
    if summary:
        system_prompt += " At the beginning of the page output, provide a concise 2-3 bullet point summary under '### Ringkasan Cepat'."
    if custom_prompt:
        system_prompt += f" Special focus requested by user: {custom_prompt}"

    data = {
        "model": model,
        "stream": False,
        "max_tokens": 4096,
        "messages": [
            {"role": "system", "content": system_prompt},
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": f"Extract document content from Page {page_num}. For any tabular data, you MUST format it as a valid HTML <table> with <thead>, <tbody>, <tr>, <th>, <td> tags (preserving colspan for section headers/banners). Do not use markdown pipe tables."},
                    {"type": "image_url", "image_url": {"url": f"data:image/png;base64,{img_b64}"}}
                ]
            }
        ]
    }

    req = urllib.request.Request(url, data=json.dumps(data).encode("utf-8"), headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            body = resp.read().decode("utf-8")
            res_json = json.loads(body)
            choice = res_json.get("choices", [{}])[0]
            msg = choice.get("message", {})
            content = msg.get("content", "")
            if not content and "reasoning_content" in msg:
                content = msg["reasoning_content"]
            return content or ""
    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8") if e.fp else ""
        print(f"[-] HTTP {e.code} Error from VLM ({url}): {err_body}", file=sys.stderr)
        raise RuntimeError(f"VLM API failed ({e.code}): {err_body}")
    except Exception as e:
        print(f"[-] VLM request failed: {e}", file=sys.stderr)
        raise

def stitch_cross_page_tables(page_results):
    stitched_pages = []
    prev_unclosed_table = None

    for i, res in enumerate(page_results):
        page_num = res["page"]
        content = res["content"]

        has_table_start = "<table" in content.lower()
        has_table_end = "</table>" in content.lower()

        # Check 1: Previous page had unclosed HTML table
        if prev_unclosed_table and stitched_pages:
            m_tbody = re.search(r"^\s*(?:<tbody>)?\s*(<tr>[\s\S]*?)(?=</table>|\Z)", content, re.IGNORECASE)
            if m_tbody and (not has_table_start or content.find("<tr") < content.find("<table")):
                matched_rows = m_tbody.group(1)
                stitched_pages[-1]["content"] = stitched_pages[-1]["content"].rstrip() + "\n" + matched_rows + "\n</table>"
                content = re.sub(r"^\s*(?:<tbody>)?\s*<tr>[\s\S]*?</table>", "", content, flags=re.IGNORECASE)
                prev_unclosed_table = None

        # Check 2: Matching table headers across consecutive pages (HTML or Markdown)
        if stitched_pages and (has_table_start or "|" in content):
            prev_content = stitched_pages[-1]["content"]

            # Try HTML table stitching
            prev_html_tables = list(re.finditer(r"<table[\s\S]*?</table>", prev_content, re.IGNORECASE))
            curr_html_tables = list(re.finditer(r"<table[\s\S]*?</table>", content, re.IGNORECASE))

            if prev_html_tables and curr_html_tables:
                last_prev_match = prev_html_tables[-1]
                first_curr_match = curr_html_tables[0]
                h_prev = extract_table_headers(last_prev_match.group(0))
                h_curr = extract_table_headers(first_curr_match.group(0))

                if headers_match(h_prev, h_curr):
                    c_tbl = first_curr_match.group(0)
                    tbody_m = re.search(r"<tbody[^>]*>([\s\S]*?)</tbody>", c_tbl, re.IGNORECASE)
                    rows_html = tbody_m.group(1) if tbody_m else c_tbl
                    data_trs = re.findall(r"<tr[^>]*>[\s\S]*?</tr>", rows_html, re.IGNORECASE)
                    clean_trs = [tr for tr in data_trs if "<th" not in tr.lower()]

                    if clean_trs:
                        merged_rows = "\n" + "\n".join(clean_trs)
                        p_tbl = last_prev_match.group(0)
                        if "</tbody>" in p_tbl.lower():
                            idx = p_tbl.lower().rfind("</tbody>")
                            new_p_tbl = p_tbl[:idx] + merged_rows + "\n" + p_tbl[idx:]
                        else:
                            idx = p_tbl.lower().rfind("</table>")
                            new_p_tbl = p_tbl[:idx] + merged_rows + "\n" + p_tbl[idx:]

                        stitched_pages[-1]["content"] = (
                            prev_content[:last_prev_match.start()] +
                            new_p_tbl +
                            prev_content[last_prev_match.end():]
                        )
                        stitch_notice = f"\n\n*(Tabel lanjutan telah digabungkan ke tabel Halaman {stitched_pages[-1]['page']})*\n\n"
                        content = content[:first_curr_match.start()] + stitch_notice + content[first_curr_match.end():]

            # Try Markdown table stitching
            prev_md_tables = find_markdown_tables(stitched_pages[-1]["content"])
            curr_md_tables = find_markdown_tables(content)
            if prev_md_tables and curr_md_tables:
                last_prev_md = prev_md_tables[-1]
                first_curr_md = curr_md_tables[0]
                h_prev_md = extract_table_headers(last_prev_md)
                h_curr_md = extract_table_headers(first_curr_md)

                if headers_match(h_prev_md, h_curr_md):
                    md_lines = [ln for ln in first_curr_md.splitlines() if ln.strip().startswith("|")]
                    data_lines = []
                    for idx, ln in enumerate(md_lines):
                        if idx == 0: continue
                        inner = ln.strip()[1:-1].strip()
                        if re.match(r"^[-:\s|]+$", inner): continue
                        data_lines.append(ln)

                    if data_lines:
                        new_prev_md = last_prev_md + "\n" + "\n".join(data_lines)
                        stitched_pages[-1]["content"] = stitched_pages[-1]["content"].replace(last_prev_md, new_prev_md)
                        stitch_notice = f"\n\n*(Tabel lanjutan telah digabungkan ke tabel Halaman {stitched_pages[-1]['page']})*\n\n"
                        content = content.replace(first_curr_md, stitch_notice)

        # Track unclosed table
        if has_table_start and not has_table_end:
            prev_unclosed_table = page_num
        elif has_table_start and has_table_end:
            last_start = content.rfind("<table")
            last_end = content.rfind("</table>")
            if last_start > last_end:
                prev_unclosed_table = page_num
            else:
                prev_unclosed_table = None
        else:
            prev_unclosed_table = None

        stitched_pages.append({"page": page_num, "content": content})

    return stitched_pages

def process_document(args):
    path = Path(args.file)
    if not path.exists():
        raise FileNotFoundError(f"File not found: {args.file}")

    base_url = (args.base_url or os.environ.get("DOC_EXTRACT_BASE_URL") or DEFAULT_BASE_URL).rstrip("/")
    api_key = args.api_key or os.environ.get("DOC_EXTRACT_API_KEY") or os.environ.get("CODEBUDDY_API_KEY") or DEFAULT_API_KEY
    model = args.model or os.environ.get("DOC_EXTRACT_MODEL") or DEFAULT_MODEL

    out_path = Path(args.output)
    out_path.mkdir(parents=True, exist_ok=True)

    temp_dir = tempfile.mkdtemp(prefix="aina_doc_extract_")
    ext = path.suffix.lower()

    try:
        t0 = time.time()
        if not args.stdout:
            print(f"[*] Processing document: {path.name}")
            print(f"[*] VLM Engine: {model} ({base_url})")
            print(f"[*] Mode: {args.mode} | DPI: {args.dpi} | Concurrency: {args.concurrency}")

        all_page_images = []
        if ext == ".pdf":
            prefix = os.path.join(temp_dir, "page")
            subprocess.run(["pdftoppm", "-png", "-r", str(args.dpi), str(path), prefix], check=True)
            files = sorted([os.path.join(temp_dir, f) for f in os.listdir(temp_dir) if f.startswith("page-") and f.endswith(".png")])
            all_page_images = files
        elif ext in [".png", ".jpg", ".jpeg", ".webp", ".tiff"]:
            all_page_images = [str(path)]
        else:
            raise ValueError(f"Unsupported format: {ext}. Supported: PDF, PNG, JPG, WEBP, TIFF.")

        total_available = len(all_page_images)
        selected_pages = parse_page_range(args.pages, total_available)
        
        if not args.stdout:
            print(f"[+] Total pages in doc: {total_available}. Selected for extraction: {len(selected_pages)} ({selected_pages})")

        page_work_items = []
        for p_idx in selected_pages:
            img_file = all_page_images[p_idx - 1]
            page_work_items.append((p_idx, img_file))

        raw_results = {}

        def worker(page_num, img_path):
            with open(img_path, "rb") as f:
                b64 = base64.b64encode(f.read()).decode("utf-8")
            if not args.stdout:
                print(f"[*] Extracting Page {page_num}...")

            max_retries = max(1, args.retries)
            last_err = None

            for attempt in range(1, max_retries + 1):
                try:
                    text = call_vlm_completion(base_url, api_key, model, b64, page_num, args.mode, args.prompt, args.summary)
                    if text and text.strip():
                        return page_num, text
                    else:
                        raise ValueError("VLM returned empty content")
                except Exception as e:
                    last_err = e
                    if attempt < max_retries:
                        sleep_s = args.retry_delay * (1.5 ** (attempt - 1))
                        if not args.stdout:
                            print(f"[!] Halaman {page_num} gagal (percobaan {attempt}/{max_retries}): {e}. Mengulang otomatis dalam {sleep_s:.1f}s...")
                        time.sleep(sleep_s)
                    else:
                        if not args.stdout:
                            print(f"[-] Halaman {page_num} gagal setelah {max_retries}x percobaan: {last_err}")
                        if args.continue_on_error:
                            return page_num, f"*[Peringatan: Gagal mengekstrak Halaman {page_num} setelah {max_retries}x percobaan ({last_err})]*"
                        else:
                            raise last_err


        max_workers = min(args.concurrency, len(page_work_items))
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = [executor.submit(worker, p_num, p_img) for p_num, p_img in page_work_items]
            for fut in as_completed(futures):
                p_num, txt = fut.result()
                raw_results[p_num] = txt

        ordered_results = [{"page": p, "content": raw_results[p]} for p in selected_pages]

        if not args.no_stitch and len(ordered_results) > 1:
            stitched = stitch_cross_page_tables(ordered_results)
        else:
            stitched = ordered_results

        # Format output Markdown
        full_md = []
        full_md.append(f"# Ekstraksi Dokumen: {path.name}\n")
        full_md.append(f"- **Waktu Ekstraksi**: {time.strftime('%Y-%m-%d %H:%M:%S %Z')}")
        full_md.append(f"- **Halaman Diproses**: {len(selected_pages)}/{total_available} ({selected_pages})")
        full_md.append(f"- **Engine**: `{model}` via `{base_url}`\n---\n")

        for p in stitched:
            full_md.append(f"## Halaman {p['page']}\n")
            full_md.append(p["content"])
            full_md.append("\n---\n")

        combined_text = "\n".join(full_md)
        md_file = out_path / "extracted_content.md"
        with open(md_file, "w", encoding="utf-8") as f:
            f.write(combined_text)

        # Extract structured HTML tables into dedicated JSON & optional CSVs
        tables = []
        csv_files = []
        tbl_counter = 0

        for p in stitched:
            # Detect HTML tables
            found_html = re.findall(r"(<table[\s\S]*?</table>)", p["content"], flags=re.IGNORECASE)
            for tidx, tbl in enumerate(found_html):
                tbl_counter += 1
                table_entry = {
                    "table_id": tbl_counter,
                    "page": p["page"],
                    "table_index_on_page": tidx + 1,
                    "format": "html",
                    "html": tbl
                }
                tables.append(table_entry)

                if args.csv or "csv" in args.format:
                    csv_name = f"table_p{p['page']}_{tidx+1}.csv"
                    csv_target = out_path / csv_name
                    rows = html_table_to_csv_rows(tbl)
                    if rows:
                        with open(csv_target, "w", newline="", encoding="utf-8") as cf:
                            writer = csv.writer(cf)
                            writer.writerows(rows)
                        csv_files.append(str(csv_target))

            # Detect Markdown tables (if any)
            found_md = find_markdown_tables(p["content"])
            for tidx, tbl in enumerate(found_md):
                tbl_counter += 1
                table_entry = {
                    "table_id": tbl_counter,
                    "page": p["page"],
                    "table_index_on_page": tidx + 1,
                    "format": "markdown",
                    "markdown": tbl
                }
                tables.append(table_entry)

                if args.csv or "csv" in args.format:
                    csv_name = f"table_p{p['page']}_md{tidx+1}.csv"
                    csv_target = out_path / csv_name
                    rows = markdown_table_to_csv_rows(tbl)
                    if rows:
                        with open(csv_target, "w", newline="", encoding="utf-8") as cf:
                            writer = csv.writer(cf)
                            writer.writerows(rows)
                        csv_files.append(str(csv_target))

        json_file = out_path / "extracted_tables.json"
        with open(json_file, "w", encoding="utf-8") as f:
            json.dump({
                "source_file": str(path),
                "total_pages_in_doc": total_available,
                "extracted_pages": selected_pages,
                "total_tables": len(tables),
                "tables": tables,
                "csv_files": [Path(c).name for c in csv_files]
            }, f, indent=2)

        elapsed = time.time() - t0

        if args.stdout:
            print(combined_text)
        else:
            print(f"[✓] Ekstraksi sukses ({elapsed:.2f}s, {len(selected_pages)} halaman, {len(tables)} tabel)!")
            print(f"[✓] Markdown: {md_file}")
            print(f"[✓] JSON Tables: {json_file}")
            if csv_files:
                print(f"[✓] CSV Tables ({len(csv_files)}): {out_path}/*.csv")

        return str(md_file)

    finally:
        shutil.rmtree(temp_dir, ignore_errors=True)

def main():
    parser = argparse.ArgumentParser(
        description="agy-doc-extract - Advanced Vision-Language Document & Table Extractor",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""Contoh Penggunaan:
  agy-doc-extract laporan.pdf -o output/
  agy-doc-extract laporan.pdf --pages 1-3,5 --mode tables --csv
  agy-doc-extract struk.png --prompt "Hitung total pembayaran dan pajak" --stdout
  agy-doc-extract skripsi.pdf --mode math --pages 12-14
"""
    )
    parser.add_argument("file", help="Path ke berkas dokumen (PDF, PNG, JPG, WEBP, TIFF)")
    parser.add_argument("-o", "--output", default="output/doc_extract", help="Direktori output (default: output/doc_extract)")
    parser.add_argument("-p", "--pages", default="all", help="Rentang halaman yang ingin diekstrak (contoh: 1-5, 3, 7-10, default: all)")
    parser.add_argument("-m", "--mode", choices=["auto", "tables", "text", "math", "raw"], default="auto", help="Mode ekstraksi (default: auto)")
    parser.add_argument("-f", "--format", default="markdown,json", help="Format ekspor: markdown, json, csv (default: markdown,json)")
    parser.add_argument("--csv", action="store_true", help="Ekspor setiap tabel HTML langsung menjadi berkas .csv terpisah")
    parser.add_argument("-q", "--prompt", default="", help="Instruksi / fokus khusus ke VLM (misal: 'audit angka baris EBITDA')")
    parser.add_argument("-s", "--summary", action="store_true", help="Sertakan ringkasan eksekutif cepat di awal hasil")
    parser.add_argument("--dpi", type=int, default=200, help="DPI rasterisasi PDF (default: 200, gunakan 300 untuk teks sangat rapat/kecil)")
    parser.add_argument("-c", "--concurrency", type=int, default=4, help="Jumlah thread paralel per halaman (default: 4)")
    parser.add_argument("-r", "--retries", type=int, default=3, help="Jumlah percobaan ulang otomatis jika ada halaman gagal/error (default: 3)")
    parser.add_argument("--retry-delay", type=float, default=2.0, help="Jeda awal antar percobaan dalam detik dengan exponential backoff (default: 2.0s)")
    parser.add_argument("--continue-on-error", action="store_true", help="Tetap lanjutkan pemrosesan halaman lain jika ada halaman yang gagal setelah retries habis")
    parser.add_argument("--no-stitch", action="store_true", help="Nonaktifkan penggabungan tabel otomatis antar-halaman")
    parser.add_argument("--model", default="", help="Override model VLM (default: cbai/deepseek-v4.1-flash)")

    parser.add_argument("--base-url", default="", help="Override base URL VLM (default: https://router.dvlpid.my.id/v1)")
    parser.add_argument("--api-key", default="", help="Override API Key VLM (default: sk-9router-master-key)")
    parser.add_argument("--stdout", action="store_true", help="Cetak hasil markdown langsung ke stdout")


    args = parser.parse_args()
    try:
        process_document(args)
    except Exception as e:
        print(f"[-] Error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
