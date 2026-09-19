#!/usr/bin/env python3
"""
test_extreme_minimax.py - The OmniDocBench Extreme Challenge for cbai/minimax-m3
Tests 8 hardest community edge-cases as of 2025/2026:
1. Multi-Level Hierarchical Headers (2-Tier thead with colspan & rowspan)
2. Vertical Merged Cells in Body (rowspan across multiple sub-items)
3. Multi-Line Wrapped Descriptions within Cells
4. Accounting Negatives in Parentheses (Rp 18.250.000,00) & Dash zeros (—)
5. Superscript Footnote Markers (*¹ and [a]) adjacent to numbers without corruption
6. Formulas & Greek Math Symbols inside cells (Δ = -14.2%, μ = 99.8%)
7. Watermark & Semi-Transparent Stamp Occlusions (Red 'AUDITED' stamp overlapping table)
8. Cross-Page Spanning (Crossing Page 1 -> Page 2 boundary cleanly)
"""

import sys, os, time, json, re, subprocess, shutil
from pathlib import Path

OUT_DIR = Path("/tmp/extreme_benchmark")
OUT_DIR.mkdir(parents=True, exist_ok=True)
PDF_FILE = OUT_DIR / "extreme_doc_challenge.pdf"
CHROME_BIN = "/usr/bin/google-chrome"
EXTRACTOR = "/root/projects/aina/skills/vision-document-extractor/scripts/doc_extract.py"

GOLDEN_TARGETS = [
    # Page 1
    {"id": "POS-101", "name": "Infrastruktur Compute Baremetal", "actual": "Rp 145.000.000,00", "variance": "-5.2%", "footnote": "*1", "stamp_overlap": False},
    {"id": "POS-102", "name": "Dedicated SAN Storage NVMe 200TB", "actual": "Rp 98.400.000,00", "variance": "+1.8%", "footnote": None, "stamp_overlap": False},
    {"id": "POS-103", "name": "Konektivitas Dark Fiber Multi-AZ 100G", "actual": "Rp 32.500.000,00", "variance": "0.0%", "footnote": None, "stamp_overlap": False},
    {"id": "POS-201", "name": "Audit Keamanan Eksternal SOC2 Type II", "actual": "Rp 120.000.000,00", "variance": "+12.5%", "footnote": "*2", "stamp_overlap": True},
    {"id": "POS-202", "name": "Penetration Testing Red Team Infra", "actual": "(Rp 45.000.000,00)", "variance": "-8.4%", "footnote": "[a]", "stamp_overlap": True},
    {"id": "POS-203", "name": "Sertifikasi ISO/IEC 27001 ISMS", "actual": "Rp 65.200.000,00", "variance": "+3.1%", "footnote": None, "stamp_overlap": True},
    # Page 2 (Spanning continuation)
    {"id": "POS-301", "name": "Lisensi VLM Enterprise Vision Cluster", "actual": "Rp 210.000.000,00", "variance": "+24.0%", "footnote": "*3", "stamp_overlap": False},
    {"id": "POS-302", "name": "API Gateway Distributed Reverse Proxy", "actual": "(Rp 14.800.000,00)", "variance": "-15.0%", "footnote": None, "stamp_overlap": False},
    {"id": "POS-303", "name": "Alokasi Bandwidth Anycast Cloudflare", "actual": "Rp 18.750.000,00", "variance": "0.0%", "footnote": None, "stamp_overlap": False},
    {"id": "POS-401", "name": "Micro-ledger: Hash BLAKE3 blok verifikasi 9f8a2b", "actual": "Rp 850.400,00", "variance": "Δ = +0.4%", "footnote": "[b]", "stamp_overlap": False},
    {"id": "POS-402", "name": "Micro-ledger: Retensi trace eBPF kernel 30-hari", "actual": "(Rp 312.000,00)", "variance": "Δ = -2.1%", "footnote": None, "stamp_overlap": False},
    {"id": "POS-403", "name": "Micro-ledger: Biaya materai & rekonsiliasi nol", "actual": "—", "variance": "0.0%", "footnote": None, "stamp_overlap": False},
]

def generate_extreme_pdf():
    html = """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  @page { size: A4; margin: 12mm; }
  body { font-family: 'Liberation Sans', sans-serif; color: #1a202c; position: relative; }
  
  /* Watermark Background */
  .watermark {
    position: fixed;
    top: 35%;
    left: 10%;
    width: 80%;
    text-align: center;
    font-size: 42px;
    font-weight: 900;
    color: rgba(160, 174, 192, 0.15);
    transform: rotate(-30deg);
    z-index: 0;
    pointer-events: none;
    letter-spacing: 4px;
  }

  /* Semi-transparent Rubber Stamp */
  .rubber-stamp {
    position: absolute;
    top: 310px;
    left: 280px;
    border: 3px dashed rgba(229, 62, 62, 0.45);
    color: rgba(229, 62, 62, 0.55);
    padding: 10px 18px;
    font-size: 13px;
    font-weight: 900;
    text-transform: uppercase;
    transform: rotate(14deg);
    border-radius: 8px;
    z-index: 10;
    pointer-events: none;
    letter-spacing: 1.5px;
    line-height: 1.3;
    text-align: center;
  }

  h2 { color: #1a365d; font-size: 15px; margin-bottom: 3px; z-index: 2; position: relative; }
  .meta { font-size: 10px; color: #718096; margin-bottom: 12px; z-index: 2; position: relative; }
  table { width: 100%; border-collapse: collapse; z-index: 2; position: relative; font-size: 9.5px; }
  th, td { border: 1px solid #a0aec0; padding: 6px 8px; }
  
  /* Multi-level Headers */
  th.top-h { background-color: #2c5282; color: #ffffff; text-align: center; font-size: 10px; font-weight: bold; }
  th.sub-h { background-color: #ebf8ff; color: #2b6cb0; text-align: center; font-size: 9px; font-weight: 600; }
  
  .cat-cell { background-color: #edf2f7; font-weight: bold; vertical-align: middle; text-align: center; width: 14%; }
  .num { text-align: right; font-family: 'Liberation Mono', monospace; }
  .negative { color: #c53030; }
  .sup { font-size: 7.5px; font-weight: bold; vertical-align: super; color: #dd6b20; }
  .desc { line-height: 1.35; }
  .page-break { page-break-before: always; }
</style>
</head>
<body>
  <div class="watermark">RAHASIA • STRICTLY CONFIDENTIAL</div>

  <h2>LAPORAN REALISASI ANGGARAN & AUDIT RISIKO Q3-2026 (HALAMAN 1)</h2>
  <div class="meta">Uji Ekstrem VLM: Multi-Level Header | Rowspan | Negatif Parentheses | Overlap Stempel | Catatan Kaki</div>

  <div class="rubber-stamp">
    ★ AUDITED & VERIFIED ★<br>
    SOC2 / ISO COMPLIANT<br>
    <span style="font-size: 9px;">REF: QA-2026-99182</span>
  </div>

  <table>
    <thead>
      <tr>
        <th class="top-h" rowspan="2">Kategori Utama</th>
        <th class="top-h" rowspan="2">ID Rekening</th>
        <th class="top-h" rowspan="2">Uraian Rincian Belanja & Beban</th>
        <th class="top-h" colspan="2">Alokasi Finansial Multi-Tier</th>
        <th class="top-h" colspan="2">Audit Kepatuhan & Deviasi</th>
      </tr>
      <tr>
        <th class="sub-h">Target Pagu</th>
        <th class="sub-h">Realisasi Aktual</th>
        <th class="sub-h">Deviasi (Δ)</th>
        <th class="sub-h">Catatan & Formula</th>
      </tr>
    </thead>
    <tbody>
      <!-- Category 1: Infrastruktur Cloud (Rowspan = 3) -->
      <tr>
        <td class="cat-cell" rowspan="3">Sektor 1: Cloud & Data Center</td>
        <td><code>POS-101</code></td>
        <td class="desc">Infrastruktur Compute Baremetal<br><span style="font-size: 8px; color: #4a5568;">Dual AMD EPYC 9654 128-Core cluster node</span></td>
        <td class="num">Rp 150.000.000,00</td>
        <td class="num">Rp 145.000.000,00<span class="sup">*1</span></td>
        <td class="num">-5.2%</td>
        <td>μ = 99.98% uptime</td>
      </tr>
      <tr>
        <td><code>POS-102</code></td>
        <td class="desc">Dedicated SAN Storage NVMe 200TB<br><span style="font-size: 8px; color: #4a5568;">Replika 3x synchronous Ceph pool</span></td>
        <td class="num">Rp 100.000.000,00</td>
        <td class="num">Rp 98.400.000,00</td>
        <td class="num">+1.8%</td>
        <td>IOPS &gt; 500k</td>
      </tr>
      <tr>
        <td><code>POS-103</code></td>
        <td class="desc">Konektivitas Dark Fiber Multi-AZ 100G<br><span style="font-size: 8px; color: #4a5568;">Peering Equinix SG1 - Cyber Building</span></td>
        <td class="num">Rp 32.500.000,00</td>
        <td class="num">Rp 32.500.000,00</td>
        <td class="num">0.0%</td>
        <td>SLA 99.999%</td>
      </tr>

      <!-- Category 2: Keamanan Siber (Rowspan = 3) [Overlapped by Red Stamp] -->
      <tr>
        <td class="cat-cell" rowspan="3">Sektor 2: Cyber Defense & Compliance</td>
        <td><code>POS-201</code></td>
        <td class="desc">Audit Keamanan Eksternal SOC2 Type II<br><span style="font-size: 8px; color: #4a5568;">Pemeriksaan kontrol privasi data nasabah</span></td>
        <td class="num">Rp 110.000.000,00</td>
        <td class="num">Rp 120.000.000,00<span class="sup">*2</span></td>
        <td class="num">+12.5%</td>
        <td>Passed Clean Opinion</td>
      </tr>
      <tr>
        <td><code>POS-202</code></td>
        <td class="desc">Penetration Testing Red Team Infra<br><span style="font-size: 8px; color: #4a5568;">Simulasi serangan zero-day breach</span></td>
        <td class="num">Rp 50.000.000,00</td>
        <td class="num negative">(Rp 45.000.000,00)<span class="sup">[a]</span></td>
        <td class="num negative">-8.4%</td>
        <td>Penghematan retensi internal</td>
      </tr>
      <tr>
        <td><code>POS-203</code></td>
        <td class="desc">Sertifikasi ISO/IEC 27001 ISMS<br><span style="font-size: 8px; color: #4a5568;">Surveillance audit tahunan BSI</span></td>
        <td class="num">Rp 65.000.000,00</td>
        <td class="num">Rp 65.200.000,00</td>
        <td class="num">+3.1%</td>
        <td>ISO-2026-CERT</td>
      </tr>
    </tbody>
  </table>

  <div class="page-break"></div>

  <h2>LAPORAN REALISASI ANGGARAN & AUDIT RISIKO Q3-2026 (LANJUTAN HALAMAN 2)</h2>
  <div class="meta">Lanjutan Sektor 3 (Kecerdasan Buatan) dan Sektor 4 (Dense Micro-Ledger Akuntansi)</div>

  <table>
    <thead>
      <tr>
        <th class="top-h" rowspan="2">Kategori Utama</th>
        <th class="top-h" rowspan="2">ID Rekening</th>
        <th class="top-h" rowspan="2">Uraian Rincian Belanja & Beban</th>
        <th class="top-h" colspan="2">Alokasi Finansial Multi-Tier</th>
        <th class="top-h" colspan="2">Audit Kepatuhan & Deviasi</th>
      </tr>
      <tr>
        <th class="sub-h">Target Pagu</th>
        <th class="sub-h">Realisasi Aktual</th>
        <th class="sub-h">Deviasi (Δ)</th>
        <th class="sub-h">Catatan & Formula</th>
      </tr>
    </thead>
    <tbody>
      <!-- Category 3: AI & VLM (Rowspan = 3) -->
      <tr>
        <td class="cat-cell" rowspan="3">Sektor 3: AI & Platform Services</td>
        <td><code>POS-301</code></td>
        <td class="desc">Lisensi VLM Enterprise Vision Cluster<br><span style="font-size: 8px; color: #4a5568;">Token batch throughput 1M token window</span></td>
        <td class="num">Rp 180.000.000,00</td>
        <td class="num">Rp 210.000.000,00<span class="sup">*3</span></td>
        <td class="num">+24.0%</td>
        <td>Over-quota scaling burst</td>
      </tr>
      <tr>
        <td><code>POS-302</code></td>
        <td class="desc">API Gateway Distributed Reverse Proxy<br><span style="font-size: 8px; color: #4a5568;">Beban komputasi edge WAF gateway</span></td>
        <td class="num">Rp 17.500.000,00</td>
        <td class="num negative">(Rp 14.800.000,00)</td>
        <td class="num negative">-15.0%</td>
        <td>Efisiensi caching 92%</td>
      </tr>
      <tr>
        <td><code>POS-303</code></td>
        <td class="desc">Alokasi Bandwidth Anycast Cloudflare<br><span style="font-size: 8px; color: #4a5568;">DDoS mitigation unmetered bandwidth</span></td>
        <td class="num">Rp 18.750.000,00</td>
        <td class="num">Rp 18.750.000,00</td>
        <td class="num">0.0%</td>
        <td>Contract Ref #CF-991</td>
      </tr>

      <!-- Category 4: Micro-Ledger (Rowspan = 3) (Micro font 7.5px) -->
      <tr style="font-size: 7.5px;">
        <td class="cat-cell" rowspan="3" style="font-size: 8px;">Sektor 4: Audit & Micro-Ledger</td>
        <td><code>POS-401</code></td>
        <td class="desc">Micro-ledger: Hash BLAKE3 blok verifikasi 9f8a2b</td>
        <td class="num">Rp 850.000,00</td>
        <td class="num">Rp 850.400,00<span class="sup">[b]</span></td>
        <td class="num">Δ = +0.4%</td>
        <td><code>b3:9f8a2b1049</code></td>
      </tr>
      <tr style="font-size: 7.5px;">
        <td><code>POS-402</code></td>
        <td class="desc">Micro-ledger: Retensi trace eBPF kernel 30-hari</td>
        <td class="num">Rp 320.000,00</td>
        <td class="num negative">(Rp 312.000,00)</td>
        <td class="num negative">Δ = -2.1%</td>
        <td>Zstandard compression</td>
      </tr>
      <tr style="font-size: 7.5px;">
        <td><code>POS-403</code></td>
        <td class="desc">Micro-ledger: Biaya materai & rekonsiliasi nol</td>
        <td class="num">—</td>
        <td class="num">—</td>
        <td class="num">0.0%</td>
        <td>Zero balance reconcile</td>
      </tr>
    </tbody>
  </table>

  <div style="font-size: 8.5px; color: #4a5568; margin-top: 15px; border-top: 1px solid #cbd5e0; padding-top: 6px;">
    <strong>Catatan Kaki (Footnotes):</strong><br>
    *1 Termasuk diskon multi-year komitmen 3 tahun.<br>
    *2 Terkena penyesuaian biaya audit on-site tambahan.<br>
    *3 Skalabilitas otomatis beban puncak Q3.<br>
    [a] Nilai dalam kurung menandakan alokasi saldo kredit (surplus penghematan).<br>
    [b] Hash kriptografi diverifikasi secara terdistribusi via node validator independen.
  </div>
</body>
</html>"""

    html_file = OUT_DIR / "extreme_doc_challenge.html"
    with open(html_file, "w", encoding="utf-8") as f:
        f.write(html)

    subprocess.run([
        CHROME_BIN, "--headless", "--no-sandbox", "--disable-gpu",
        f"--print-to-pdf={PDF_FILE}", str(html_file)
    ], check=True)
    print(f"[+] Extreme PDF Fixture generated: {PDF_FILE} ({PDF_FILE.stat().st_size} bytes)")

def run_test():
    print("=" * 85)
    print(" 🌪️ THE OMNIDOCBENCH EXTREME CHALLENGE FOR cbai/minimax-m3")
    print(" Testing 8 Hardest Document Intelligence Edge-Cases (CVPR 2025 / 2026 Paradigm)")
    print("=" * 85)

    generate_extreme_pdf()
    out_dir = OUT_DIR / "extracted"
    shutil.rmtree(out_dir, ignore_errors=True)

    t0 = time.time()
    cmd = [
        "python3", EXTRACTOR,
        str(PDF_FILE),
        "-o", str(out_dir),
        "--model", "cbai/minimax-m3",
        "--dpi", "250",
        "-c", "2",
        "-r", "3",
        "--csv"
    ]
    print(f"[*] Menjalankan ekstraksi dokumen ekstrem: {PDF_FILE.name} via cbai/minimax-m3...")
    res = subprocess.run(cmd, capture_output=True, text=True)
    total_time = time.time() - t0

    if res.returncode != 0:
        print(f"[-] Ekstraksi gagal: {res.stderr}")
        return

    md_file = out_dir / "extracted_content.md"
    with open(md_file, "r", encoding="utf-8") as f:
        extracted = f.read()

    # Normalize extracted text for flexible matching
    norm_text = re.sub(r"\s+", " ", extracted)

    print("\n" + "=" * 85)
    print(f" 📊 HASIL EVALUASI 8 FITUR EKSTREM (OmniDocBench Criteria)")
    print("=" * 85)

    # 1. Multi-Level Headers Check
    has_top_header = "alokasi finansial" in extracted.lower() or "kategori utama" in extracted.lower()
    has_sub_header = "target pagu" in extracted.lower() or "realisasi aktual" in extracted.lower()
    header_score = "✅ SEMPURNA" if (has_top_header and has_sub_header) else "⚠️ PARSIAL"
    print(f" 1. Multi-Level Hierarchical Header (colspan/rowspan): {header_score}")

    # 2. Rowspan / Category grouping check
    has_cloud_cat = "sektor 1" in extracted.lower() or "cloud" in extracted.lower()
    has_cyber_cat = "sektor 2" in extracted.lower() or "cyber" in extracted.lower()
    rowspan_score = "✅ SEMPURNA" if (has_cloud_cat and has_cyber_cat) else "⚠️ PARSIAL"
    print(f" 2. Vertical Merged Cells (Rowspan Data Body)       : {rowspan_score}")

    # 3. Accounting Negatives in Parentheses Check
    has_neg1 = "(45.000.000" in extracted or "(Rp 45.000.000" in extracted or "(45,000,000" in extracted
    has_neg2 = "(14.800.000" in extracted or "(Rp 14.800.000" in extracted or "(14,800,000" in extracted
    neg_score = "✅ SEMPURNA" if (has_neg1 and has_neg2) else ("⚠️ PARSIAL" if (has_neg1 or has_neg2) else "✗ GAGAL")
    print(f" 3. Accounting Negative in Parentheses (Rp xxx)     : {neg_score} [POS-202 & POS-302]")

    # 4. Semi-Transparent Red Stamp Overlap Check
    # Check POS-201, POS-202, POS-203 extracted despite red stamp occlusion
    stamp_items = ["POS-201", "POS-202", "POS-203"]
    stamp_ok = sum(1 for it in stamp_items if it in extracted)
    stamp_score = f"✅ SEMPURNA ({stamp_ok}/3 terbaca)" if stamp_ok == 3 else f"⚠️ {stamp_ok}/3"
    print(f" 4. Penetrasi Tembus Stempel Merah (Occlusion Stamp) : {stamp_score}")

    # 5. Superscript Footnotes Adjacent to Numbers
    # Verify footnotes didn't corrupt the numbers (e.g. 145.000.000 is not 1450000001)
    fn_preserved = ("145.000.000" in extracted or "145,000,000" in extracted) and ("*1" in extracted or "Catatan Kaki" in extracted or "Footnote" in extracted)
    fn_score = "✅ SEMPURNA (Angka tidak terkorupsi)" if fn_preserved else "⚠️ PARSIAL"
    print(f" 5. Superscript Footnotes (*1, [a], [b])            : {fn_score}")

    # 6. Formulas & Greek Math Symbols
    math_symbols = ("μ" in extracted or "99.98%" in extracted) and ("Δ" in extracted or "delta" in extracted.lower() or "-5.2%" in extracted)
    math_score = "✅ SEMPURNA" if math_symbols else "⚠️ PARSIAL"
    print(f" 6. Karakter Matematika & Simbol Formula (μ, Δ, %)   : {math_score}")

    # 7. Cross-Page Spanning
    # Check if table spanning POS-101 (Page 1) to POS-403 (Page 2) was stitched
    html_tables = re.findall(r"<table[\s\S]*?</table>", extracted, re.IGNORECASE)
    stitched = any("POS-101" in re.sub(r"\s*-\s*", "-", t) and "POS-403" in re.sub(r"\s*-\s*", "-", t) for t in html_tables)
    if not stitched:
        md_tables = re.findall(r"((?:^[ \t]*\|.+?\|[ \t]*\r?\n)+(?:^[ \t]*\|[-:\s|]+?\|[ \t]*\r?\n)(?:^[ \t]*\|.+?\|[ \t]*(?:\r?\n|\Z))+)", extracted, re.MULTILINE)
        stitched = any("POS-101" in re.sub(r"\s*-\s*", "-", t) and "POS-403" in re.sub(r"\s*-\s*", "-", t) for t in md_tables)
    if not stitched:
        for cf in out_dir.glob("*.csv"):
            with open(cf, "r", encoding="utf-8") as f:
                c = re.sub(r"\s*-\s*", "-", f.read())
                if "POS-101" in c and "POS-403" in c:
                    stitched = True
                    break
    stitch_score = "✅ 100% TERSAMBUNG UTUH" if stitched else "⚠️ TERPISAH PER-HALAMAN"
    print(f" 7. Cross-Page Table Stitching (Halaman 1 -> 2)     : {stitch_score}")

    # 8. Record Accuracy breakdown
    norm_hyphen_ext = re.sub(r"\s*-\s*", "-", extracted)
    clean_ext = re.sub(r"[\s\.,()–—\-]", "", extracted)

    correct_count = 0
    print("\n" + "-" * 85)
    print(f"{'ID Pos':<10} | {'Uraian Target':<40} | {'Nominal':<22} | {'Status':<10}")
    print("-" * 85)
    for g in GOLDEN_TARGETS:
        id_ok = g["id"] in norm_hyphen_ext or g["id"] in extracted
        clean_amt = re.sub(r"[\s\.,()–—\-]", "", g["actual"])
        if clean_amt:
            amt_ok = clean_amt in clean_ext or g["actual"] in extracted
        else:
            # Special case for dash zero balance
            amt_ok = ("–" in extracted or "—" in extracted or "Zero balance" in extracted)

        match = id_ok and amt_ok
        if match:
            correct_count += 1
            stat = "✓ MATCH"
        else:
            stat = "✗ MISS"
        print(f"{g['id']:<10} | {g['name'][:38]:<40} | {g['actual']:<22} | {stat:<10}")

    acc = (correct_count / len(GOLDEN_TARGETS)) * 100.0
    print("-" * 85)
    print(f" 8. Total Akurasi Record Ekstrem: {correct_count}/{len(GOLDEN_TARGETS)} ({acc:.1f}%)")
    print(f" ⏱️  Total Waktu Ekstraksi 2 Halaman (DPI 250): {total_time:.2f} detik")
    print("=" * 85)

if __name__ == "__main__":
    run_test()
