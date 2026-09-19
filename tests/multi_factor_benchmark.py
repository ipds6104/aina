#!/usr/bin/env python3
"""
multi_factor_benchmark.py - Multi-Factor Document Table Extraction Benchmark
Evaluates VLM performance across:
1. Font Sizes: Standard (12pt), Small (9pt), Micro/Tiny (6.5pt)
2. Font Types: Sans-Serif, Serif, Monospace, and Mixed Typography
3. Border Styles:
   - Full Grid (All cell borders present)
   - No Vertical Border (Horizontal rules only, columns separated by whitespace)
   - Completely Borderless (Zero cell lines, financial ledger whitespace layout)
4. Multi-Page Spanning: Table crossing Page 1 -> Page 2 boundary
5. VLM Models: cbai/minimax-m3, cbai/glm-5v-turbo, cbai/kimi-k2.7
6. Concurrency: Fully parallelized page extraction via 9Router load-balancer.
"""

import sys, os, time, json, re, subprocess, shutil
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

WORK_DIR = Path("/tmp/multifactor_benchmark")
WORK_DIR.mkdir(parents=True, exist_ok=True)
CHROME_BIN = "/usr/bin/google-chrome"
EXTRACTOR = "/root/projects/aina/skills/vision-document-extractor/scripts/doc_extract.py"

# Golden Records with Font Size, Font Style, and Field Values
RECORDS = [
    # --- PAGE 1: Standard (12pt) & Small (9pt) ---
    # Rows 1-4: Standard (12pt) - Sans-Serif
    {"id": "STD-01", "size": "12pt", "family": "sans", "desc": "Infrastruktur Node Kubernetes Baremetal", "amount": "Rp 142.500.000,00", "code": "K8S-901-OK"},
    {"id": "STD-02", "size": "12pt", "family": "sans", "desc": "Penyediaan NVMe SAN Storage 100TB", "amount": "Rp 98.400.000,00", "code": "SAN-902-ST"},
    # Rows 5-6: Standard (12pt) - Serif
    {"id": "STD-03", "size": "12pt", "family": "serif", "desc": "Konsultasi Legal Hak Paten Intelektual", "amount": "Rp 75.000.000,00", "code": "LGL-903-IP"},
    {"id": "STD-04", "size": "12pt", "family": "serif", "desc": "Audit Kepatuhan Perlindungan Data Finansial", "amount": "Rp 110.250.000,00", "code": "AUD-904-FN"},
    # Rows 7-8: Standard (12pt) - Mixed (ID Mono, Desc Serif, Amount Mono)
    {"id": "STD-05", "size": "12pt", "family": "mixed", "desc": "Pengadaan Switch Cisco Nexus 400G Tier-1", "amount": "Rp 230.000.000,00", "code": "NET-905-CS"},
    {"id": "STD-06", "size": "12pt", "family": "mixed", "desc": "Beban Daya Listrik Colocation Equinix SG1", "amount": "Rp 45.800.000,00", "code": "PWR-906-EQ"},

    # Rows 9-10: Small (9pt) - Monospace
    {"id": "SML-07", "size": "9pt", "family": "mono", "desc": "Pipeline CI/CD Runner Dedicated GitLab Enterprise", "amount": "Rp 14.200.000,00", "code": "OPS-411-GL"},
    {"id": "SML-08", "size": "9pt", "family": "mono", "desc": "Sertifikat TLS Wildcard EV DigiCert Global Root", "amount": "Rp 28.500.000,00", "code": "SEC-412-EV"},
    # Rows 11-12: Small (9pt) - Mixed Typography (Page 1 end)
    {"id": "SML-09", "size": "9pt", "family": "mixed", "desc": "Edge Proxy Anycast Cloudflare Enterprise DDoS Protection", "amount": "Rp 16.750.000,00", "code": "WAF-413-CF"},
    {"id": "SML-10", "size": "9pt", "family": "mixed", "desc": "Audit Arsitektur Redundansi Zero-Trust SD-WAN", "amount": "Rp 34.000.000,00", "code": "ZTN-414-ZT"},

    # >>> PAGE BREAK HERE <<<

    # --- PAGE 2: Small (9pt) Continuation & Micro (6.5pt) ---
    # Rows 13-14: Small (9pt) - Sans-Serif Continuation
    {"id": "SML-11", "size": "9pt", "family": "sans", "desc": "Langganan Telemetri Distributed Tracing Datadog", "amount": "Rp 19.300.000,00", "code": "MON-415-DD"},
    {"id": "SML-12", "size": "9pt", "family": "sans", "desc": "Sistem Incident Management On-Call PagerDuty", "amount": "Rp 7.850.000,00", "code": "ALR-416-PD"},
    # Rows 15-16: Small (9pt) - Serif
    {"id": "SML-13", "size": "9pt", "family": "serif", "desc": "Jasa Notaris Korporasi & Akta Perjanjian SLA", "amount": "Rp 12.500.000,00", "code": "NOT-417-NT"},
    {"id": "SML-14", "size": "9pt", "family": "serif", "desc": "Biaya Sertifikasi Standarisasi ISO/IEC 27001", "amount": "Rp 54.000.000,00", "code": "ISO-418-IS"},

    # Rows 17-18: Micro (6.5pt) - Monospace
    {"id": "MCR-15", "size": "6.5pt", "family": "mono", "desc": "Micro-ledger: Hash integritas blok ledger BLAKE3: 7c4e92a8b1", "amount": "Rp 640.200,00", "code": "BLK-101-M"},
    {"id": "MCR-16", "size": "6.5pt", "family": "mono", "desc": "Micro-ledger: Biaya retensi log eBPF kernel 14-hari tier-cold", "amount": "Rp 890.500,00", "code": "LOG-102-M"},
    # Rows 19-20: Micro (6.5pt) - Sans-Serif
    {"id": "MCR-17", "size": "6.5pt", "family": "sans", "desc": "Micro-ledger: Alokasi bandwidth burst 95th percentile IXP SG", "amount": "Rp 2.450.000,00", "code": "NET-103-M"},
    {"id": "MCR-18", "size": "6.5pt", "family": "sans", "desc": "Micro-ledger: Overhead amortisasi server blade UCS Q2-2026", "amount": "Rp 1.780.000,00", "code": "UCS-104-M"},
    # Rows 21-22: Micro (6.5pt) - Serif
    {"id": "MCR-19", "size": "6.5pt", "family": "serif", "desc": "Micro-ledger: Pajak pertambahan nilai PPN WAPU transaksi jasa", "amount": "Rp 4.120.000,00", "code": "TAX-105-M"},
    {"id": "MCR-20", "size": "6.5pt", "family": "serif", "desc": "Micro-ledger: Biaya materai elektronik pos kliring perbankan", "amount": "Rp 320.000,00", "code": "MTR-106-M"},
    # Rows 23-24: Micro (6.5pt) - Mixed Typography
    {"id": "MCR-21", "size": "6.5pt", "family": "mixed", "desc": "Micro-ledger: Komputasi serverless cold-start duration audit", "amount": "Rp 540.800,00", "code": "SRV-107-M"},
    {"id": "MCR-22", "size": "6.5pt", "family": "mixed", "desc": "Micro-ledger: Replikasi multi-AZ quorum sync delta packet", "amount": "Rp 980.400,00", "code": "QRM-108-M"},
]

BORDER_CONFIGS = {
    "full_grid": {
        "title": "Full Grid (Semua Border Horizontal & Vertikal)",
        "table_css": "border-collapse: collapse; width: 100%;",
        "th_css": "border: 1px solid #2b6cb0; background: #2b6cb0; color: #fff; padding: 6px 8px; text-align: left;",
        "td_css": "border: 1px solid #cbd5e0; padding: 5px 8px;",
        "zebra": False
    },
    "no_vertical": {
        "title": "No Vertical Borders (Garis Baris Saja, Kolom Terpisah Spasi)",
        "table_css": "border-collapse: collapse; width: 100%;",
        "th_css": "border-top: 2px solid #2d3748; border-bottom: 2px solid #2d3748; border-left: none; border-right: none; background: #edf2f7; color: #1a202c; padding: 7px 10px; text-align: left;",
        "td_css": "border-bottom: 1px solid #e2e8f0; border-top: none; border-left: none; border-right: none; padding: 6px 10px;",
        "zebra": False
    },
    "borderless": {
        "title": "Completely Borderless (Tanpa Border Kolom/Baris, Zebra Striping Tipis)",
        "table_css": "border-collapse: collapse; width: 100%; border: none;",
        "th_css": "border-bottom: 1px solid #718096; background: transparent; color: #1a202c; font-weight: bold; padding: 6px 8px; text-align: left;",
        "td_css": "border: none; padding: 5px 8px;",
        "zebra": True
    }
}

def generate_pdf_fixture(border_type: str) -> Path:
    cfg = BORDER_CONFIGS[border_type]
    out_pdf = WORK_DIR / f"test_{border_type}.pdf"
    out_html = WORK_DIR / f"test_{border_type}.html"

    def render_row(r, idx):
        size_px = "13px" if r["size"] == "12pt" else ("10px" if r["size"] == "9pt" else "7.5px")
        fam = r["family"]
        if fam == "sans":
            row_font = "font-family: 'Liberation Sans', sans-serif;"
            id_font = row_font
            desc_font = row_font
            amt_font = row_font
            code_font = row_font
        elif fam == "serif":
            row_font = "font-family: 'Liberation Serif', serif;"
            id_font = row_font
            desc_font = row_font
            amt_font = row_font
            code_font = row_font
        elif fam == "mono":
            row_font = "font-family: 'Liberation Mono', monospace;"
            id_font = row_font
            desc_font = row_font
            amt_font = row_font
            code_font = row_font
        else: # mixed
            id_font = "font-family: 'Liberation Mono', monospace; font-weight: bold;"
            desc_font = "font-family: 'Liberation Serif', serif;"
            amt_font = "font-family: 'Liberation Mono', monospace;"
            code_font = "font-family: 'Liberation Mono', monospace;"

        bg = "background-color: #f7fafc;" if (cfg["zebra"] and idx % 2 == 1) else ""
        return (
            f"<tr style='font-size: {size_px}; {bg}'>"
            f"<td style='{cfg['td_css']} {id_font}'>{r['id']}</td>"
            f"<td style='{cfg['td_css']} {desc_font}'>{r['desc']}</td>"
            f"<td style='{cfg['td_css']} {amt_font} text-align: right;'>{r['amount']}</td>"
            f"<td style='{cfg['td_css']} {code_font}'>{r['code']}</td>"
            f"</tr>"
        )

    p1_rows = "".join([render_row(r, i) for i, r in enumerate(RECORDS[:10])])
    p2_rows = "".join([render_row(r, i) for i, r in enumerate(RECORDS[10:])])

    html_content = f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  @page {{ size: A4; margin: 15mm; }}
  body {{ font-family: 'Liberation Sans', sans-serif; color: #1a202c; }}
  h2 {{ color: #1a365d; font-size: 15px; margin-bottom: 2px; }}
  .sub {{ font-size: 10px; color: #718096; margin-bottom: 12px; }}
  table {{ {cfg['table_css']} }}
  th {{ {cfg['th_css']} }}
  .page-break {{ page-break-before: always; }}
  .sec-bar {{ background: #edf2f7; font-weight: bold; font-size: 9.5px; padding: 4px 8px; border: 1px solid #cbd5e0; }}
</style>
</head>
<body>
  <h2>REKAPITULASI DOKUMEN MULTI-FAKTOR: {cfg['title'].upper()} (HALAMAN 1)</h2>
  <div class="sub">Evaluasi VLM: Variasi Font 12pt s.d. 6.5pt | Sans, Serif, Mono, Mixed | {cfg['title']}</div>
  <table>
    <thead>
      <tr>
        <th style="width: 12%;">ID Pos</th>
        <th style="width: 48%;">Uraian Pengeluaran</th>
        <th style="width: 22%; text-align: right;">Nominal Tagihan</th>
        <th style="width: 18%;">Kode Otorisasi</th>
      </tr>
    </thead>
    <tbody>
      {p1_rows}
    </tbody>
  </table>

  <div class="page-break"></div>

  <h2>REKAPITULASI DOKUMEN MULTI-FAKTOR: {cfg['title'].upper()} (LANJUTAN HALAMAN 2)</h2>
  <div class="sub">Lanjutan Tabel Bersambung dari Halaman 1 | Multi-Font Dense Ledger</div>
  <table>
    <thead>
      <tr>
        <th style="width: 12%;">ID Pos</th>
        <th style="width: 48%;">Uraian Pengeluaran</th>
        <th style="width: 22%; text-align: right;">Nominal Tagihan</th>
        <th style="width: 18%;">Kode Otorisasi</th>
      </tr>
    </thead>
    <tbody>
      {p2_rows}
    </tbody>
  </table>
</body>
</html>"""

    with open(out_html, "w", encoding="utf-8") as f:
        f.write(html_content)

    subprocess.run([
        CHROME_BIN, "--headless", "--no-sandbox", "--disable-gpu",
        f"--print-to-pdf={out_pdf}", str(out_html)
    ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    return out_pdf

def evaluate_extraction(extracted_md_path: Path, out_dir: Path):
    if not extracted_md_path.exists():
        return {"accuracy": 0.0, "total": len(RECORDS), "matched": 0, "by_size": {}, "by_family": {}, "stitched": False}

    with open(extracted_md_path, "r", encoding="utf-8") as f:
        text = f.read()

    # Normalize text for robust number checking
    norm_text = re.sub(r"[\s\.,]", "", text)

    by_size = {"12pt": {"tot": 0, "ok": 0}, "9pt": {"tot": 0, "ok": 0}, "6.5pt": {"tot": 0, "ok": 0}}
    by_family = {"sans": {"tot": 0, "ok": 0}, "serif": {"tot": 0, "ok": 0}, "mono": {"tot": 0, "ok": 0}, "mixed": {"tot": 0, "ok": 0}}

    matched_count = 0
    for r in RECORDS:
        s = r["size"]
        fam = r["family"]
        by_size[s]["tot"] += 1
        by_family[fam]["tot"] += 1

        id_found = r["id"] in text
        # check amount without dots and commas
        clean_amt = re.sub(r"[\s\.,]", "", r["amount"])
        amt_found = clean_amt in norm_text
        code_found = r["code"] in text

        if id_found and (amt_found or code_found):
            matched_count += 1
            by_size[s]["ok"] += 1
            by_family[fam]["ok"] += 1

    # Check cross-page stitching (did table span STD-01 and MCR-22)
    stitched = False
    html_tables = re.findall(r"<table[\s\S]*?</table>", text, re.IGNORECASE)
    for tbl in html_tables:
        if "STD-01" in tbl and "MCR-22" in tbl:
            stitched = True
            break
    if not stitched:
        md_tables = re.findall(r"((?:^[ \t]*\|.+?\|[ \t]*\r?\n)+(?:^[ \t]*\|[-:\s|]+?\|[ \t]*\r?\n)(?:^[ \t]*\|.+?\|[ \t]*(?:\r?\n|\Z))+)", text, re.MULTILINE)
        for mdt in md_tables:
            if "STD-01" in mdt and "MCR-22" in mdt:
                stitched = True
                break
    if not stitched:
        for cf in out_dir.glob("*.csv"):
            try:
                with open(cf, "r", encoding="utf-8") as f:
                    csv_c = f.read()
                    if "STD-01" in csv_c and "MCR-22" in csv_c:
                        stitched = True
                        break
            except Exception:
                pass

    return {
        "accuracy": (matched_count / len(RECORDS)) * 100.0,
        "total": len(RECORDS),
        "matched": matched_count,
        "by_size": by_size,
        "by_family": by_family,
        "stitched": stitched
    }

def run_single_test(model: str, border_type: str, pdf_path: Path):
    test_key = f"{model}__{border_type}"
    out_dir = WORK_DIR / "runs" / test_key
    shutil.rmtree(out_dir, ignore_errors=True)
    out_dir.mkdir(parents=True, exist_ok=True)

    t0 = time.time()
    cmd = [
        "python3", EXTRACTOR,
        str(pdf_path),
        "-o", str(out_dir),
        "--model", model,
        "--dpi", "250",
        "-c", "2",
        "-r", "3",
        "--csv"
    ]
    res = subprocess.run(cmd, capture_output=True, text=True)
    duration = time.time() - t0

    eval_res = evaluate_extraction(out_dir / "extracted_content.md", out_dir)
    eval_res["duration"] = duration
    eval_res["model"] = model
    eval_res["border_type"] = border_type
    eval_res["pdf"] = pdf_path.name
    eval_res["success"] = (res.returncode == 0)
    return eval_res

def main():
    print("=" * 90)
    print(" 🚀 MULTI-FACTOR DOCUMENT TABLE EXTRACTION BENCHMARK")
    print(" Multi-Model | Multi-Font-Size | Multi-Typeface | Varied Borders | Parallel Concurrency")
    print("=" * 90)

    # 1. Generate PDF fixtures
    print("[*] Generating 3 multi-page test PDF fixtures with varied border styles...")
    fixtures = {}
    for b_type in ["full_grid", "no_vertical", "borderless"]:
        pdf_p = generate_pdf_fixture(b_type)
        fixtures[b_type] = pdf_p
        print(f"    • {b_type:<15} -> {pdf_p.name} ({pdf_p.stat().st_size} bytes)")

    models = ["cbai/minimax-m3", "cbai/glm-5v-turbo", "cbai/kimi-k2.7"]
    print(f"\n[*] Target Models: {models}")
    print(f"[*] Total Test Combinations: {len(models)} models × {len(fixtures)} border styles = 9 test matrices")
    print(f"[*] Dispatching concurrent benchmark executions across 9Router...")

    start_all = time.time()
    results = []

    # Execute all 9 test matrix combinations in parallel
    with ThreadPoolExecutor(max_workers=6) as executor:
        futures = {}
        for m in models:
            for b_type, pdf_p in fixtures.items():
                fut = executor.submit(run_single_test, m, b_type, pdf_p)
                futures[fut] = (m, b_type)

        for fut in as_completed(futures):
            m, b_type = futures[fut]
            try:
                res = fut.result()
                results.append(res)
                stitch_str = "✅ STITCHED" if res["stitched"] else "⚠️ SEPARATE"
                print(f"[+] Done: {m:<18} | {b_type:<12} | Acc: {res['accuracy']:.1f}% | Time: {res['duration']:.2f}s | {stitch_str}")
            except Exception as e:
                print(f"[-] Error on {m} + {b_type}: {e}")

    total_bench_time = time.time() - start_all

    # Save detailed JSON
    with open(WORK_DIR / "benchmark_matrix_results.json", "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)

    # Presentation Output Tables
    print("\n" + "=" * 90)
    print(" 📊 MATRIKS HASIL AKURASI: MODEL x BORDER TYPE x CROSS-PAGE STITCHING")
    print("=" * 90)
    print(f"{'Model VLM':<20} | {'Border Style':<14} | {'Akurasi (%)':<12} | {'Durasi (s)':<10} | {'Stitching Lintas Halaman':<25}")
    print("-" * 90)
    for r in sorted(results, key=lambda x: (x["model"], x["border_type"])):
        stitch_label = "✅ 100% Utuh (Tersambung)" if r["stitched"] else "⚠️ Terpisah Per-Halaman"
        print(f"{r['model']:<20} | {r['border_type']:<14} | {r['accuracy']:>10.1f}% | {r['duration']:>8.2f}s | {stitch_label:<25}")

    print("\n" + "=" * 90)
    print(" 🔬 MATRIKS AKURASI BERDASARKAN UKURAN FONT (Standard 12pt vs Small 9pt vs Micro 6.5pt)")
    print("=" * 90)
    print(f"{'Model VLM':<20} | {'Standard 12pt':<18} | {'Small 9pt (Spanning)':<22} | {'Micro 6.5pt (Dense)':<20}")
    print("-" * 90)
    for m in models:
        m_runs = [r for r in results if r["model"] == m]
        if not m_runs: continue
        # aggregate
        s_12_tot = sum(r["by_size"]["12pt"]["tot"] for r in m_runs)
        s_12_ok  = sum(r["by_size"]["12pt"]["ok"]  for r in m_runs)
        s_9_tot  = sum(r["by_size"]["9pt"]["tot"]  for r in m_runs)
        s_9_ok   = sum(r["by_size"]["9pt"]["ok"]   for r in m_runs)
        s_6_tot  = sum(r["by_size"]["6.5pt"]["tot"] for r in m_runs)
        s_6_ok   = sum(r["by_size"]["6.5pt"]["ok"]  for r in m_runs)

        acc_12 = (s_12_ok / s_12_tot * 100) if s_12_tot else 0
        acc_9  = (s_9_ok  / s_9_tot  * 100) if s_9_tot  else 0
        acc_6  = (s_6_ok  / s_6_tot  * 100) if s_6_tot  else 0

        print(f"{m:<20} | {s_12_ok}/{s_12_tot} ({acc_12:>5.1f}%)     | {s_9_ok}/{s_9_tot} ({acc_9:>5.1f}%)        | {s_6_ok}/{s_6_tot} ({acc_6:>5.1f}%)")

    print("\n" + "=" * 90)
    print(" 🖋️ MATRIKS AKURASI BERDASARKAN JENIS TIPOGRAFI / FONT FAMILY")
    print("=" * 90)
    print(f"{'Model VLM':<20} | {'Sans-Serif':<16} | {'Serif':<16} | {'Monospace':<16} | {'Mixed / Campur':<16}")
    print("-" * 90)
    for m in models:
        m_runs = [r for r in results if r["model"] == m]
        if not m_runs: continue
        sans_tot  = sum(r["by_family"]["sans"]["tot"] for r in m_runs)
        sans_ok   = sum(r["by_family"]["sans"]["ok"] for r in m_runs)
        serif_tot = sum(r["by_family"]["serif"]["tot"] for r in m_runs)
        serif_ok  = sum(r["by_family"]["serif"]["ok"] for r in m_runs)
        mono_tot  = sum(r["by_family"]["mono"]["tot"] for r in m_runs)
        mono_ok   = sum(r["by_family"]["mono"]["ok"] for r in m_runs)
        mix_tot   = sum(r["by_family"]["mixed"]["tot"] for r in m_runs)
        mix_ok    = sum(r["by_family"]["mixed"]["ok"] for r in m_runs)

        a_sans  = (sans_ok / sans_tot * 100) if sans_tot else 0
        a_serif = (serif_ok / serif_tot * 100) if serif_tot else 0
        a_mono  = (mono_ok / mono_tot * 100) if mono_tot else 0
        a_mix   = (mix_ok / mix_tot * 100) if mix_tot else 0

        print(f"{m:<20} | {sans_ok}/{sans_tot} ({a_sans:>5.1f}%) | {serif_ok}/{serif_tot} ({a_serif:>5.1f}%) | {mono_ok}/{mono_tot} ({a_mono:>5.1f}%) | {mix_ok}/{mix_tot} ({a_mix:>5.1f}%)")

    print("\n" + "=" * 90)
    print(f"🏁 Total Waktu Pengujian 9 Kombinasi (Paralel 18 Halaman): {total_bench_time:.2f} detik")
    print("=" * 90)

if __name__ == "__main__":
    main()
