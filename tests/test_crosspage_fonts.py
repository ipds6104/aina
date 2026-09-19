#!/usr/bin/env python3
"""
test_crosspage_fonts.py - Benchmark Multi-Page Spanning Table with Variable Font Sizes
Font scales tested across page boundaries:
- Standard Font: 12pt (Rows 1-8, Page 1)
- Small Font: 9pt (Rows 9-16, Spanning Page 1 to Page 2)
- Tiny / Micro Font: 6.5pt (Rows 17-24, Page 2 dense sub-ledger)
"""

import sys, os, time, json, re, subprocess, shutil
from pathlib import Path

OUT_DIR = Path("/tmp/font_benchmark")
OUT_DIR.mkdir(parents=True, exist_ok=True)
PDF_FILE = OUT_DIR / "crosspage_multi_fonts.pdf"
CHROME_BIN = "/usr/bin/google-chrome"
EXTRACTOR = "/root/projects/aina/skills/vision-document-extractor/scripts/doc_extract.py"

GOLDEN_RECORDS = [
    # --- STANDARD FONT (12pt / 14px) - Page 1 ---
    {"id": "STD-01", "font": "standard", "category": "Infrastruktur Cloud Utama", "amount": "Rp 125.450.000,00", "code": "AUTH-9921-A"},
    {"id": "STD-02", "font": "standard", "category": "Lisensi AI Model Copilot", "amount": "Rp 88.200.000,00", "code": "AUTH-9922-B"},
    {"id": "STD-03", "font": "standard", "category": "Audit Keamanan SOC2 Type II", "amount": "Rp 145.000.000,00", "code": "AUTH-9923-C"},
    {"id": "STD-04", "font": "standard", "category": "Penyediaan Dedicated Fiber 10G", "amount": "Rp 36.750.000,00", "code": "AUTH-9924-D"},
    {"id": "STD-05", "font": "standard", "category": "Perpanjangan Domain & DNS Anycast", "amount": "Rp 14.300.000,00", "code": "AUTH-9925-E"},
    {"id": "STD-06", "font": "standard", "category": "Hardware Switch Arista 100Gbps", "amount": "Rp 210.000.000,00", "code": "AUTH-9926-F"},
    {"id": "STD-07", "font": "standard", "category": "Cluster Database Managed PostgreSQL", "amount": "Rp 64.120.000,00", "code": "AUTH-9927-G"},
    {"id": "STD-08", "font": "standard", "category": "Penyimpanan Objek S3 Tiering 500TB", "amount": "Rp 92.800.000,00", "code": "AUTH-9928-H"},

    # --- SMALL FONT (9pt / 11px) - Crossing Page 1 to Page 2 ---
    {"id": "SML-09", "font": "small", "category": "Sub-Alokasi Edge WAF Cloudflare Pro", "amount": "Rp 7.450.000,00", "code": "SUB-4411-WAF"},
    {"id": "SML-10", "font": "small", "category": "Token Logging Distributed Datadog", "amount": "Rp 18.250.000,00", "code": "SUB-4412-LOG"},
    {"id": "SML-11", "font": "small", "category": "Langganan PagerDuty On-Call Alerts", "amount": "Rp 5.600.000,00", "code": "SUB-4413-ALR"},
    {"id": "SML-12", "font": "small", "category": "Sertifikat TLS Wildcard DigiCert EV", "amount": "Rp 22.000.000,00", "code": "SUB-4414-TLS"},
    # >>> PAGE BREAK HERE <<<
    {"id": "SML-13", "font": "small", "category": "VPN Gateway Site-to-Site WireGuard", "amount": "Rp 9.150.000,00", "code": "SUB-4415-VPN"},
    {"id": "SML-14", "font": "small", "category": "Beban Compute Batch Training H100", "amount": "Rp 320.000.000,00", "code": "SUB-4416-GPU"},
    {"id": "SML-15", "font": "small", "category": "Proxy Reverse Gateway Load Balancer", "amount": "Rp 11.400.000,00", "code": "SUB-4417-LBX"},
    {"id": "SML-16", "font": "small", "category": "Audit Kepatuhan ISO/IEC 27701 PII", "amount": "Rp 48.000.000,00", "code": "SUB-4418-ISO"},

    # --- TINY / MICRO FONT (6.5pt / 8px) - Dense Ledger Page 2 ---
    {"id": "TNY-17", "font": "tiny", "category": "Micro-ledger: Hash verifikasi ledger BLAKE3: 9f82ab410d", "amount": "Rp 450.250,00", "code": "MCR-001-X"},
    {"id": "TNY-18", "font": "tiny", "category": "Micro-ledger: Retensi trace eBPF kernel network packets", "amount": "Rp 875.000,00", "code": "MCR-002-Y"},
    {"id": "TNY-19", "font": "tiny", "category": "Micro-ledger: Biaya komputasi Lambda cold-start overhead", "amount": "Rp 312.400,00", "code": "MCR-003-Z"},
    {"id": "TNY-20", "font": "tiny", "category": "Micro-ledger: Alokasi IP Anycast IPv4 BGP Announcement", "amount": "Rp 1.450.000,00", "code": "MCR-004-A"},
    {"id": "TNY-21", "font": "tiny", "category": "Micro-ledger: Pemeliharaan modul HSM FIPS 140-3 Level 4", "amount": "Rp 3.800.000,00", "code": "MCR-005-B"},
    {"id": "TNY-22", "font": "tiny", "category": "Micro-ledger: Audit kepatuhan GDPR Clause 46 Transfer", "amount": "Rp 2.150.000,00", "code": "MCR-006-C"},
    {"id": "TNY-23", "font": "tiny", "category": "Micro-ledger: Cadangan bandwidth peering Equinix SG1-IX", "amount": "Rp 4.200.000,00", "code": "MCR-007-D"},
    {"id": "TNY-24", "font": "tiny", "category": "Micro-ledger: Penyesuaian amortisasi aset hardware Q2", "amount": "Rp 1.980.000,00", "code": "MCR-008-E"},
]

def generate_pdf():
    def row_to_tr(r):
        f = r["font"]
        if f == "standard":
            style = "font-size: 12px; padding: 7px 10px;"
        elif f == "small":
            style = "font-size: 9px; padding: 5px 8px; color: #2d3748;"
        else: # tiny
            style = "font-size: 6.5px; padding: 3px 6px; color: #4a5568; font-family: 'Liberation Mono', monospace;"
        return f"<tr style='{style}'><td><strong>{r['id']}</strong></td><td>{r['category']}</td><td style='text-align: right;'>{r['amount']}</td><td><code>{r['code']}</code></td></tr>"

    rows_page1 = [row_to_tr(r) for r in GOLDEN_RECORDS[:12]]
    rows_page2 = [row_to_tr(r) for r in GOLDEN_RECORDS[12:]]

    p1_tier1 = "".join(rows_page1[:8])
    p1_tier2 = "".join(rows_page1[8:])
    p2_tier2 = "".join(rows_page2[:4])
    p2_tier3 = "".join(rows_page2[4:])

    html = f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  @page {{ size: A4; margin: 15mm; }}
  body {{ font-family: 'Liberation Sans', sans-serif; color: #1a202c; }}
  h2 {{ color: #1a365d; font-size: 16px; margin-bottom: 4px; }}
  .meta {{ font-size: 11px; color: #718096; margin-bottom: 12px; }}
  table {{ width: 100%; border-collapse: collapse; }}
  th {{ background-color: #2b6cb0; color: #ffffff; text-align: left; font-size: 11px; padding: 8px 10px; border: 1px solid #2b6cb0; }}
  td {{ border: 1px solid #cbd5e0; }}
  .sec-header {{ background-color: #edf2f7; font-weight: bold; font-size: 10px; color: #4a5568; text-transform: uppercase; padding: 5px 10px; }}
  .page-break {{ page-break-before: always; }}
</style>
</head>
<body>
  <h2>REKAPITULASI BIAYA & AUDIT MULTI-TIER (HALAMAN 1)</h2>
  <div class="meta">Dokumen Pengujian Akurasi VLM: Variasi Font Standard (12pt) s.d. Small (9pt)</div>
  <table>
    <thead>
      <tr><th style="width: 12%;">ID Pos</th><th style="width: 48%;">Uraian Pengeluaran</th><th style="width: 22%; text-align: right;">Nominal Tagihan</th><th style="width: 18%;">Kode Otorisasi</th></tr>
    </thead>
    <tbody>
      <tr><td colspan="4" class="sec-header">Tier 1: Beban Pokok Infrastruktur (Font Standar 12pt)</td></tr>
      {p1_tier1}
      <tr><td colspan="4" class="sec-header">Tier 2: Sub-Alokasi Layanan Pendukung (Font Kecil 9pt - Bersambung)</td></tr>
      {p1_tier2}
    </tbody>
  </table>

  <div class="page-break"></div>

  <h2>REKAPITULASI BIAYA & AUDIT MULTI-TIER (LANJUTAN HALAMAN 2)</h2>
  <div class="meta">Lanjutan Tier 2 (Font Kecil 9pt) dan Tier 3 (Font Mikro / Tiny 6.5pt)</div>
  <table>
    <thead>
      <tr><th style="width: 12%;">ID Pos</th><th style="width: 48%;">Uraian Pengeluaran</th><th style="width: 22%; text-align: right;">Nominal Tagihan</th><th style="width: 18%;">Kode Otorisasi</th></tr>
    </thead>
    <tbody>
      {p2_tier2}
      <tr><td colspan="4" class="sec-header">Tier 3: Catatan Audit & Micro-Ledger (Font Sangat Kecil / Tiny 6.5pt)</td></tr>
      {p2_tier3}
    </tbody>
  </table>
</body>
</html>"""

    html_file = OUT_DIR / "crosspage_multi_fonts.html"
    with open(html_file, "w", encoding="utf-8") as f:
        f.write(html)

    subprocess.run([
        CHROME_BIN,
        "--headless",
        "--no-sandbox",
        "--disable-gpu",
        f"--print-to-pdf={PDF_FILE}",
        str(html_file)
    ], check=True)
    print(f"[+] PDF Fixture generated: {PDF_FILE} ({PDF_FILE.stat().st_size} bytes)")

def run_test(model="cbai/glm-5v-turbo"):
    print("=" * 80)
    print(" 🔬 BENCHMARK TABEL LINTAS HALAMAN & UKURAN FONT (12pt -> 9pt -> 6.5pt)")
    print(f" Engine: {model} via 9Router")
    print("=" * 80)

    generate_pdf()

    out_extract = OUT_DIR / "extracted"
    shutil.rmtree(out_extract, ignore_errors=True)

    t0 = time.time()
    # Using 250 DPI for high precision on tiny 6.5pt font
    cmd = [
        "python3", EXTRACTOR,
        str(PDF_FILE),
        "-o", str(out_extract),
        "--model", model,
        "--dpi", "250",
        "-c", "4",
        "-r", "3",
        "--csv"
    ]
    print(f"[*] Menjalankan ekstraksi dokumen: {PDF_FILE.name} (Model: {model}, DPI: 250, Concurrency: 4)...")
    res = subprocess.run(cmd, capture_output=True, text=True)
    total_time = time.time() - t0

    if res.returncode != 0:
        print(f"[-] Ekstraksi gagal: {res.stderr}")
        return

    md_file = out_extract / "extracted_content.md"
    if not md_file.exists():
        print("[-] File markdown hasil tidak ditemukan!")
        return

    with open(md_file, "r", encoding="utf-8") as f:
        extracted_text = f.read()

    # Evaluation breakdown by font tiers
    tiers = {
        "standard": {"name": "Font Standar (12pt, Rows 1-8)", "total": 0, "correct": 0, "samples": []},
        "small":    {"name": "Font Kecil (9pt, Rows 9-16, Lintas Halaman)", "total": 0, "correct": 0, "samples": []},
        "tiny":     {"name": "Font Sangat Kecil (6.5pt, Rows 17-24)", "total": 0, "correct": 0, "samples": []},
    }

    for r in GOLDEN_RECORDS:
        tier_key = r["font"]
        tiers[tier_key]["total"] += 1

        clean_id = r["id"]
        # Check if ID and either Amount or Code exists in extracted text
        id_found = clean_id in extracted_text
        amount_found = r["amount"].replace(" ", "").replace(".", "").replace(",", "") in extracted_text.replace(" ", "").replace(".", "").replace(",", "")
        code_found = r["code"] in extracted_text

        is_match = id_found and (amount_found or code_found)
        if is_match:
            tiers[tier_key]["correct"] += 1
            status = "✓ MATCH"
        else:
            status = "✗ MISS"

        tiers[tier_key]["samples"].append((r["id"], r["amount"], status))

    # Check cross-page stitching
    html_tables = re.findall(r"<table[\s\S]*?</table>", extracted_text, re.IGNORECASE)
    has_stitched_table = any("STD-01" in tbl and "TNY-24" in tbl for tbl in html_tables)

    if not has_stitched_table:
        md_tables = re.findall(r"((?:^[ \t]*\|.+?\|[ \t]*\r?\n)+(?:^[ \t]*\|[-:\s|]+?\|[ \t]*\r?\n)(?:^[ \t]*\|.+?\|[ \t]*(?:\r?\n|\Z))+)", extracted_text, re.MULTILINE)
        for mdt in md_tables:
            if "STD-01" in mdt and "TNY-24" in mdt:
                has_stitched_table = True
                break

    if not has_stitched_table:
        for cf in out_extract.glob("*.csv"):
            with open(cf, "r", encoding="utf-8") as f:
                csv_c = f.read()
                if "STD-01" in csv_c and "TNY-24" in csv_c:
                    has_stitched_table = True
                    break

    print("\n" + "=" * 80)
    print(" 📊 HASIL EVALUASI AKURASI EKSTRAKSI PER KATEGORI FONT")
    print("=" * 80)

    for k, v in tiers.items():
        acc = (v["correct"] / v["total"]) * 100.0
        print(f"\n🏷️  {v['name']}: {v['correct']}/{v['total']} Akurat ({acc:.1f}%)")
        for s_id, s_amt, s_stat in v["samples"]:
            print(f"    [{s_id}] {s_amt:<22} -> {s_stat}")

    total_correct = sum(v["correct"] for v in tiers.values())
    total_records = len(GOLDEN_RECORDS)
    overall_acc = (total_correct / total_records) * 100.0

    print("\n" + "-" * 80)
    print(f"📋 Evaluasi Penggabungan Lintas Halaman (Cross-Page Stitching):")
    print(f"   • Baris Halaman 1 (STD-01 s.d. SML-12) & Halaman 2 (SML-13 s.d. TNY-24) Terhubung: {'✅ YA (100% Utuh)' if has_stitched_table else '⚠️ Terpisah Per-Halaman'}")
    print(f"   • Total Akurasi Keseluruhan: {total_correct}/{total_records} ({overall_acc:.1f}%)")
    print(f"   • Total Waktu Ekstraksi 2 Halaman (Paralel): {total_time:.2f} detik")
    print("=" * 80)

if __name__ == "__main__":
    m = sys.argv[1] if len(sys.argv) > 1 else "cbai/glm-5v-turbo"
    run_test(m)
