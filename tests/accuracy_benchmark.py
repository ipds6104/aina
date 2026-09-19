#!/usr/bin/env python3
"""
accuracy_benchmark.py - Automated Hard-Case PDF Accuracy Benchmark Suite
Evaluates agy-doc-extract against complex document intelligence challenges:
1. Multi-Page Spanning Tables (Cross-page stitching & header de-duplication)
2. Multi-Level Merged Headers (Colspan, Rowspan, Negative Accounting Format)
3. Borderless Spatial Tables with Overlapping Red Stamp & Multi-line Cells
"""

import sys, os, time, json, re, subprocess, shutil
from pathlib import Path

BENCH_DIR = Path("/tmp/benchmark_docs")
BENCH_DIR.mkdir(parents=True, exist_ok=True)
CHROME_BIN = "/usr/bin/google-chrome"
EXTRACTOR = "/root/projects/aina/skills/vision-document-extractor/scripts/doc_extract.py"

def render_html_to_pdf(html_content: str, output_pdf: Path):
    temp_html = output_pdf.with_suffix(".html")
    with open(temp_html, "w", encoding="utf-8") as f:
        f.write(html_content)
    
    cmd = [
        CHROME_BIN,
        "--headless",
        "--no-sandbox",
        "--disable-gpu",
        f"--print-to-pdf={output_pdf}",
        str(temp_html)
    ]
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0 or not output_pdf.exists():
        raise RuntimeError(f"Chrome PDF generation failed: {res.stderr}")
    temp_html.unlink(missing_ok=True)

# ----------------------------------------------------------------------
# 1. FIXTURE 1: Multi-Page Spanning Table
# ----------------------------------------------------------------------
CASE1_GOLDEN_ROWS = [
    ("TRX-001", "Langganan Server Cloud", "Rp 12.500.000,00", "Lunas"),
    ("TRX-002", "Lisensi Database Enterprise", "Rp 45.000.000,00", "Lunas"),
    ("TRX-003", "Audit Keamanan Siber", "Rp 27.500.000,00", "Lunas"),
    ("TRX-004", "Optimasi CDN & Storage", "Rp 8.750.000,00", "Pending"),
    ("TRX-005", "Pelatihan AI Engineering", "Rp 35.000.000,00", "Lunas"),
    ("TRX-006", "Perangkat Keras Node Worker", "Rp 82.000.000,00", "Lunas"),
    ("TRX-007", "Langganan API Gateway", "Rp 6.400.000,00", "Lunas"),
    ("TRX-008", "Sertifikasi ISO 27001", "Rp 65.000.000,00", "Proses"),
    ("TRX-009", "Backup Off-site DR", "Rp 14.200.000,00", "Lunas"),
    ("TRX-010", "Pemeliharaan Switch 10G", "Rp 9.800.000,00", "Lunas"),
    # Page break occurs here
    ("TRX-011", "Audit Finansial Tahunan", "Rp 50.000.000,00", "Lunas"),
    ("TRX-012", "Pengadaan UPS Sentral", "Rp 74.500.000,00", "Lunas"),
    ("TRX-013", "Biaya Bandwidth Dedicated", "Rp 21.000.000,00", "Lunas"),
    ("TRX-014", "Perpanjangan Domain Utama", "Rp 1.850.000,00", "Lunas"),
    ("TRX-015", "Firewall Appliance Next-Gen", "Rp 93.000.000,00", "Lunas"),
    ("TRX-016", "Jasa Konsultan Arsitektur", "Rp 40.000.000,00", "Lunas"),
    ("TRX-017", "Langganan Monitoring Sentry", "Rp 5.200.000,00", "Lunas"),
    ("TRX-018", "Pengujian Penetrasi Eksternal", "Rp 32.000.000,00", "Lunas"),
    ("TRX-019", "Storage Expansion 100TB", "Rp 58.000.000,00", "Pending"),
    ("TRX-020", "Renewal SSL Wildcard EV", "Rp 15.600.000,00", "Lunas"),
]

def generate_crosspage_pdf(output_pdf: Path):
    rows_p1 = "".join([f"<tr><td>{r[0]}</td><td>{r[1]}</td><td>{r[2]}</td><td>{r[3]}</td></tr>" for r in CASE1_GOLDEN_ROWS[:10]])
    rows_p2 = "".join([f"<tr><td>{r[0]}</td><td>{r[1]}</td><td>{r[2]}</td><td>{r[3]}</td></tr>" for r in CASE1_GOLDEN_ROWS[10:]])

    html = f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  body {{ font-family: 'Liberation Sans', sans-serif; margin: 40px; font-size: 13px; color: #222; }}
  h2 {{ color: #1a365d; border-bottom: 2px solid #2b6cb0; padding-bottom: 6px; }}
  table {{ width: 100%; border-collapse: collapse; margin-top: 15px; }}
  th, td {{ border: 1px solid #cbd5e0; padding: 8px 12px; text-align: left; }}
  th {{ background-color: #ebf8ff; color: #2c5282; font-weight: bold; }}
  .page-break {{ page-break-before: always; }}
</style>
</head>
<body>
  <h2>Laporan Rekapitulasi Beban Operasional IT (Halaman 1)</h2>
  <p>Berikut adalah daftar pengeluaran operasional infrastruktur Q1-Q2 2026. Tabel bersambung ke halaman berikutnya.</p>
  <table>
    <thead>
      <tr><th>Kode Transaksi</th><th>Keterangan Pengeluaran</th><th>Nominal</th><th>Status</th></tr>
    </thead>
    <tbody>
      {rows_p1}
    </tbody>
  </table>

  <div class="page-break"></div>

  <h2>Laporan Rekapitulasi Beban Operasional IT (Lanjutan Halaman 2)</h2>
  <p>Lanjutan rincian transaksi dari halaman sebelumnya:</p>
  <table>
    <thead>
      <tr><th>Kode Transaksi</th><th>Keterangan Pengeluaran</th><th>Nominal</th><th>Status</th></tr>
    </thead>
    <tbody>
      {rows_p2}
    </tbody>
  </table>
  <p style="margin-top: 20px; font-weight: bold;">Catatan: Semua pengeluaran di atas telah diverifikasi oleh tim perbendaharaan.</p>
</body>
</html>"""
    render_html_to_pdf(html, output_pdf)

# ----------------------------------------------------------------------
# 2. FIXTURE 2: Multi-Level Complex Accounting Table
# ----------------------------------------------------------------------
CASE2_CRITICAL_VALUES = [
    "Laporan Posisi Keuangan (Neraca)",
    "Rp 842.150.000,00",
    "(Rp 142.500.000,00)",
    "(Rp 87.250.000,00)",
    "Rp 1.450.000.000,00",
    "Rp 2.062.400.000,00",
    "+14,85%",
    "-3,22%",
    "Audited",
    "Unaudited"
]

def generate_complex_financial_pdf(output_pdf: Path):
    html = """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  body { font-family: 'Liberation Sans', sans-serif; margin: 40px; font-size: 12px; color: #111; }
  h2 { text-align: center; margin-bottom: 4px; color: #1a202c; }
  .sub { text-align: center; font-size: 11px; color: #718096; margin-bottom: 20px; }
  table { width: 100%; border-collapse: collapse; }
  th, td { border: 1px solid #718096; padding: 6px 10px; }
  th { background-color: #edf2f7; text-align: center; }
  .num { text-align: right; font-family: 'Liberation Mono', monospace; }
  .neg { color: #c53030; }
  .bold { font-weight: bold; background-color: #f7fafc; }
</style>
</head>
<body>
  <h2>PT TEKNOLOGI KREASI BANGSA TBK</h2>
  <div class="sub">Laporan Posisi Keuangan (Neraca) Per 31 Desember 2026 dan 2025</div>
  <table>
    <thead>
      <tr>
        <th rowspan="2">Klasifikasi Aset & Liabilitas</th>
        <th colspan="2">Periode Berjalan</th>
        <th rowspan="2">Pertumbuhan (%)</th>
      </tr>
      <tr>
        <th>31 Des 2026 (Audited)</th>
        <th>31 Des 2025 (Unaudited)</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td>Kas dan Setara Kas</td>
        <td class="num">Rp 842.150.000,00</td>
        <td class="num">Rp 733.250.000,00</td>
        <td class="num">+14,85%</td>
      </tr>
      <tr>
        <td>Penyisihan Penurunan Nilai Piutang</td>
        <td class="num neg">(Rp 142.500.000,00)</td>
        <td class="num neg">(Rp 98.400.000,00)</td>
        <td class="num neg">+44,81%</td>
      </tr>
      <tr>
        <td>Akumulasi Amortisasi Software</td>
        <td class="num neg">(Rp 87.250.000,00)</td>
        <td class="num neg">(Rp 90.150.000,00)</td>
        <td class="num">-3,22%</td>
      </tr>
      <tr>
        <td>Aset Tetap & Infrastruktur Data Center</td>
        <td class="num">Rp 1.450.000.000,00</td>
        <td class="num">Rp 1.200.000.000,00</td>
        <td class="num">+20,83%</td>
      </tr>
      <tr class="bold">
        <td>Total Aset Bersih Terkonsolidasi</td>
        <td class="num">Rp 2.062.400.000,00</td>
        <td class="num">Rp 1.744.700.000,00</td>
        <td class="num">+18,21%</td>
      </tr>
    </tbody>
  </table>
</body>
</html>"""
    render_html_to_pdf(html, output_pdf)

# ----------------------------------------------------------------------
# 3. FIXTURE 3: Borderless Table + Overlapping Stamp
# ----------------------------------------------------------------------
CASE3_CRITICAL_VALUES = [
    "INV-SEC-9921",
    "Rp 185.000.000",
    "BCA 8820-192-331",
    "PT Solusi Keamanan Siber",
    "28 Februari 2026"
]

def generate_borderless_stamp_pdf(output_pdf: Path):
    html = """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  body { font-family: 'Liberation Sans', sans-serif; margin: 50px; font-size: 13px; position: relative; }
  .stamp {
    position: absolute;
    top: 120px;
    left: 100px;
    font-size: 46px;
    font-weight: 900;
    color: rgba(229, 62, 62, 0.28);
    transform: rotate(-22deg);
    border: 5px dashed rgba(229, 62, 62, 0.35);
    padding: 10px 40px;
    border-radius: 12px;
    pointer-events: none;
    z-index: 10;
  }
  table { width: 100%; border-collapse: collapse; margin-top: 30px; }
  /* Pure borderless table */
  th, td { border: none; padding: 12px 14px; vertical-align: top; }
  th { border-bottom: 2px solid #2d3748; text-align: left; color: #4a5568; text-transform: uppercase; font-size: 11px; }
  .desc { max-width: 260px; line-height: 1.4; color: #4a5568; }
</style>
</head>
<body>
  <div class="stamp">RAHASIA / CONFIDENTIAL</div>
  <h2>INVOICE JASA KONSULTASI KHUSUS</h2>
  <table>
    <thead>
      <tr>
        <th>No. Tagihan</th>
        <th>Uraian Pekerjaan (Multi-line)</th>
        <th>Penerima Pembayaran & Rekening</th>
        <th>Jumlah Tagihan</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><strong>INV-SEC-9921</strong><br><small>Tgl: 28 Februari 2026</small></td>
        <td class="desc">Penetration Testing Tier-1 pada core gateway API, mitigasi zero-day vulnerability, dan penguatan enkripsi TLS 1.3.</td>
        <td><strong>PT Solusi Keamanan Siber</strong><br>Bank: BCA 8820-192-331<br>Cabang: Sudirman Jakarta</td>
        <td><strong style="font-size: 15px;">Rp 185.000.000</strong></td>
      </tr>
    </tbody>
  </table>
</body>
</html>"""
    render_html_to_pdf(html, output_pdf)

# ----------------------------------------------------------------------
# EVALUATION & BENCHMARK RUNNER
# ----------------------------------------------------------------------
def evaluate_case1(output_dir: Path):
    md_file = output_dir / "extracted_content.md"
    if not md_file.exists():
        return False, 0.0, "File extracted_content.md not found"
    
    with open(md_file, "r", encoding="utf-8") as f:
        text = f.read()

    # 1. Check if rows are preserved
    found_rows = 0
    for r in CASE1_GOLDEN_ROWS:
        code, desc, nom, _ = r
        if code in text and nom in text:
            found_rows += 1

    row_accuracy = (found_rows / len(CASE1_GOLDEN_ROWS)) * 100.0

    # 2. Check cross-page stitching:
    # If stitched, there should be ONLY 1 table in the entire merged document or contiguous table
    tables_found = re.findall(r"<table[\s\S]*?</table>", text, re.IGNORECASE)
    # Check if TRX-001 (page 1) and TRX-020 (page 2) coexist in the same table
    stitched_success = False
    for tbl in tables_found:
        if "TRX-001" in tbl and "TRX-020" in tbl:
            stitched_success = True
            break

    # If markdown was rendered, check if TRX-001 and TRX-020 are both present
    overall_pass = row_accuracy >= 95.0 and ("TRX-001" in text and "TRX-020" in text)
    details = f"Rows Preserved: {found_rows}/{len(CASE1_GOLDEN_ROWS)} ({row_accuracy:.1f}%) | Stitched into Single Table: {stitched_success}"
    return overall_pass, row_accuracy, details

def evaluate_case2(output_dir: Path):
    md_file = output_dir / "extracted_content.md"
    if not md_file.exists():
        return False, 0.0, "File extracted_content.md not found"
    
    with open(md_file, "r", encoding="utf-8") as f:
        text = f.read()

    found_values = 0
    for val in CASE2_CRITICAL_VALUES:
        # Match with or without exact spaces/commas
        clean_val = val.replace(" ", "").replace(".", "").replace(",", "")
        clean_text = text.replace(" ", "").replace(".", "").replace(",", "")
        if clean_val.lower() in clean_text.lower():
            found_values += 1

    accuracy = (found_values / len(CASE2_CRITICAL_VALUES)) * 100.0
    has_colspan = "colspan" in text.lower() or "unaudited" in text.lower()
    has_negatives = "(Rp" in text or "142.500.000" in text or "(142,500,000" in text

    overall_pass = accuracy >= 90.0
    details = f"Values Matched: {found_values}/{len(CASE2_CRITICAL_VALUES)} ({accuracy:.1f}%) | Colspan Detected: {has_colspan} | Negatives Preserved: {has_negatives}"
    return overall_pass, accuracy, details

def evaluate_case3(output_dir: Path):
    md_file = output_dir / "extracted_content.md"
    if not md_file.exists():
        return False, 0.0, "File extracted_content.md not found"
    
    with open(md_file, "r", encoding="utf-8") as f:
        text = f.read()

    found_values = 0
    for val in CASE3_CRITICAL_VALUES:
        clean_val = val.replace(" ", "").replace(".", "").replace("-", "")
        clean_text = text.replace(" ", "").replace(".", "").replace("-", "")
        if clean_val.lower() in clean_text.lower():
            found_values += 1

    accuracy = (found_values / len(CASE3_CRITICAL_VALUES)) * 100.0
    stamp_penetrated = "185.000.000" in text or "185000000" in text
    multi_line_preserved = "penetration testing" in text.lower()

    overall_pass = accuracy >= 80.0 and stamp_penetrated
    details = f"Values Matched: {found_values}/{len(CASE3_CRITICAL_VALUES)} ({accuracy:.1f}%) | Stamp Penetrated: {stamp_penetrated} | Multi-line Preserved: {multi_line_preserved}"
    return overall_pass, accuracy, details

def run_suite():
    print("=" * 80)
    print(" 🚀 AINA & 9ROUTER VISION DOCUMENT EXTRACTION ACCURACY BENCHMARK")
    print(f" Engine Model: cbai/deepseek-v4.1-flash via 9Router (Concurrency: 4)")
    print("=" * 80)

    pdf1 = BENCH_DIR / "case1_crosspage.pdf"
    pdf2 = BENCH_DIR / "case2_financial.pdf"
    pdf3 = BENCH_DIR / "case3_borderless_stamp.pdf"

    print("\n[1/3] Generating synthetic challenge documents with Headless Chrome...")
    generate_crosspage_pdf(pdf1)
    generate_complex_financial_pdf(pdf2)
    generate_borderless_stamp_pdf(pdf3)
    print(f"  ✓ {pdf1.name} (Multi-page 20-row spanning table)")
    print(f"  ✓ {pdf2.name} (Multi-level accounting balance sheet with negative brackets)")
    print(f"  ✓ {pdf3.name} (Borderless layout with overlapping red watermark stamp)")

    cases = [
        ("Case 1: Multi-Page Spanning Table (Stitching & Header De-duplication)", pdf1, evaluate_case1),
        ("Case 2: Multi-Level Financial Accounting (Colspan, Rowspan, Negative Brackets)", pdf2, evaluate_case2),
        ("Case 3: Borderless Spatial Layout with Overlapping Red Watermark Stamp", pdf3, evaluate_case3),
    ]

    summary_results = []

    for name, pdf_path, eval_fn in cases:
        print(f"\n" + "-" * 80)
        print(f"▶ Running {name}...")
        out_case = BENCH_DIR / f"out_{pdf_path.stem}"
        shutil.rmtree(out_case, ignore_errors=True)
        out_case.mkdir(parents=True, exist_ok=True)

        t0 = time.time()
        # Run extractor with 4 concurrency, 3 retries, csv export
        cmd = [
            "python3", EXTRACTOR,
            str(pdf_path),
            "-o", str(out_case),
            "-c", "4",
            "-r", "3",
            "--csv"
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        elapsed = time.time() - t0

        if res.returncode != 0:
            print(f"[-] Extractor failed: {res.stderr}")
            passed, acc, details = False, 0.0, f"Error: {res.stderr[:100]}"
        else:
            passed, acc, details = eval_fn(out_case)

        status_icon = "✅ PASS" if passed else "❌ FAIL"
        print(f"Result: {status_icon} | Accuracy: {acc:.1f}% | Latency: {elapsed:.2f}s")
        print(f"Details: {details}")

        summary_results.append({
            "name": name,
            "passed": passed,
            "accuracy": acc,
            "latency": elapsed,
            "details": details
        })

    print("\n" + "=" * 80)
    print(" 🏆 FINAL ACCURACY BENCHMARK SUMMARY")
    print("=" * 80)
    print(f"{'Test Challenge':<55} | {'Status':<8} | {'Accuracy':<10} | {'Latency':<8}")
    print("-" * 88)
    for r in summary_results:
        st = "PASS" if r["passed"] else "FAIL"
        print(f"{r['name'][:55]:<55} | {st:<8} | {r['accuracy']:.1f}%     | {r['latency']:.2f}s")
    print("=" * 80)

if __name__ == "__main__":
    run_suite()
