#!/usr/bin/env python3
"""
Dokter V CLI Client Tool for Aina Agent.
Deterministic wrapper with separation-of-duty guardrails and group-gated access control.
"""

import sys
import os
import json
import argparse
import urllib.request
import urllib.error
import urllib.parse

def get_base_url():
    return os.environ.get("DOKTER_V_BASE_URL", "https://admin.dvlpid.my.id/api/v1")

def get_api_key():
    return os.environ.get("DOKTER_V_API_KEY") or os.environ.get("SECRET_DOKTER_V_API_KEY", "")

def check_caller_permission():
    """
    Checks if the caller has permission to invoke Dokter V operations.
    If DOKTER_V_ALLOWED_GROUPS is configured, caller must belong to one of those groups (or be admin).
    """
    allowed_groups = os.environ.get("DOKTER_V_ALLOWED_GROUPS", "")
    if not allowed_groups:
        return True, "No group restriction configured"

    allowed_list = [g.strip() for g in allowed_groups.split(",") if g.strip()]
    caller_authority = os.environ.get("AINA_CALLER_AUTHORITY", "staff").lower()
    if caller_authority == "admin":
        return True, "Admin authorized"

    caller_groups_raw = os.environ.get("AINA_CALLER_GROUPS", "")
    caller_groups = [g.strip() for g in caller_groups_raw.split(",") if g.strip()]

    # If caller has at least one matching group
    for g in caller_groups:
        if g in allowed_list:
            return True, f"Authorized via group {g}"

    return False, f"Akses ditolak: Operasi Dokter-V dibatasi untuk anggota tim yang diizinkan ({', '.join(allowed_list)})."

def make_request(method, path, body=None, params=None):
    base_url = get_base_url()
    url = f"{base_url}{path}"
    if params:
        query_string = urllib.parse.urlencode({k: v for k, v in params.items() if v is not None})
        url = f"{url}?{query_string}"

    api_key = get_api_key()
    headers = {
        "Accept": "application/json",
        "User-Agent": "Aina-Agent/1.0",
    }
    if api_key:
        headers["X-API-KEY"] = api_key
        headers["Authorization"] = f"Bearer {api_key}"

    data_bytes = None
    if body is not None:
        data_bytes = json.dumps(body).encode("utf-8")
        headers["Content-Type"] = "application/json"

    req = urllib.request.Request(url, data=data_bytes, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            content = resp.read().decode("utf-8")
            try:
                return json.loads(content)
            except Exception:
                return {"success": True, "raw": content}
    except urllib.error.HTTPError as e:
        err_content = e.read().decode("utf-8", errors="replace")
        try:
            parsed = json.loads(err_content)
            return {"error": True, "status_code": e.code, "response": parsed}
        except Exception:
            return {"error": True, "status_code": e.code, "message": err_content}
    except Exception as e:
        return {"error": True, "message": str(e)}

def cmd_status(_args):
    res = make_request("GET", "/kegiatan-manmit", params={"compact": 1, "per_page": 1})
    if res.get("error"):
        print(json.dumps({"healthy": False, "detail": res}, indent=2))
    else:
        print(json.dumps({"healthy": True, "endpoint": get_base_url()}, indent=2))

def cmd_kegiatan(args):
    params = {
        "tahun": args.tahun,
        "bulan": args.bulan,
        "has_honors": 1 if args.has_honors else None,
        "compact": 1 if args.compact else None,
        "q": args.q,
    }
    res = make_request("GET", "/kegiatan-manmit", params=params)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_mitra(args):
    params = {
        "tahun": args.tahun,
        "bulan": args.bulan,
        "available_only": 1 if args.available_only else None,
        "with_sbml": 1 if args.with_sbml else None,
        "compact": 1 if args.compact else None,
        "q": args.q,
    }
    res = make_request("GET", "/mitras", params=params)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_check(args):
    payload = {
        "honor_id": args.honor_id,
        "mitra_id": args.mitra_id,
        "target": args.target,
    }
    res = make_request("POST", "/alokasi/check", body=payload)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_alokasi(args):
    payload = {
        "honor_id": args.honor_id,
        "mitra_id": args.mitra_id,
        "target": args.target,
    }
    res = make_request("POST", "/alokasi", body=payload)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_kontrak(args):
    params = {
        "tahun": args.tahun,
        "bulan": args.bulan,
        "mitra_id": args.mitra_id,
        "compact": 1,
    }
    res = make_request("GET", "/kontrak", params=params)
    base_host = urllib.parse.urlparse(get_base_url()).netloc
    scheme = urllib.parse.urlparse(get_base_url()).scheme or "https"
    cetak_url = f"{scheme}://{base_host}/cetak/kontrak?tahun={args.tahun}&bulan={args.bulan}&mitra_id={args.mitra_id}"
    out = {
        "status": "success",
        "url_cetak_spk": cetak_url,
        "data": res
    }
    print(json.dumps(out, indent=2, ensure_ascii=False))

def cmd_bast(args):
    params = {
        "tahun": args.tahun,
        "bulan": args.bulan,
        "mitra_id": args.mitra_id,
        "id_kegiatan_manmit": args.kegiatan_id,
        "compact": 1,
    }
    res = make_request("GET", "/bast", params=params)
    base_host = urllib.parse.urlparse(get_base_url()).netloc
    scheme = urllib.parse.urlparse(get_base_url()).scheme or "https"
    cetak_url = f"{scheme}://{base_host}/cetak/bast?tahun={args.tahun}&bulan={args.bulan}&id_kegiatan_manmit={args.kegiatan_id}&mitra_id={args.mitra_id}"
    out = {
        "status": "success",
        "url_cetak_bast": cetak_url,
        "data": res
    }
    print(json.dumps(out, indent=2, ensure_ascii=False))

def cmd_penugasan(args):
    params = {
        "q": args.q,
        "status": args.status,
        "tahun": args.tahun,
        "bulan": args.bulan,
        "compact": 1,
    }
    res = make_request("GET", "/penugasan", params=params)
    print(json.dumps(res, indent=2, ensure_ascii=False))

def cmd_approve_guard(_args):
    """
    Hard deterministic guard: AI Agent is strictly forbidden from executing approval of legal/financial orders.
    """
    print(json.dumps({
        "error": True,
        "status": "forbidden",
        "message": "Aksi ditolak: Approval (persetujuan) Surat Tugas & SPD merupakan wewenang mutlak Pejabat Pembuat Komitmen (PPK) atau pejabat penandatangan sah di aplikasi Dokter-V. AI tidak memiliki kewenangan hukum untuk melakukan approval."
    }, indent=2, ensure_ascii=False))
    sys.exit(1)

def main():
    parser = argparse.ArgumentParser(description="Dokter V API Client Tool")
    subparsers = parser.add_subparsers(dest="subcommand", required=True)

    # status
    p_status = subparsers.add_parser("status", help="Check Dokter V API health")
    p_status.set_defaults(func=cmd_status)

    # kegiatan
    p_keg = subparsers.add_parser("kegiatan", help="List master activities and honor positions")
    p_keg.add_argument("--tahun", type=int, default=2026, help="Tahun kegiatan (default: 2026)")
    p_keg.add_argument("--bulan", type=int, help="Bulan kegiatan (1-12)")
    p_keg.add_argument("--has-honors", action="store_true", help="Hanya kegiatan yang memiliki honor")
    p_keg.add_argument("--compact", action="store_true", default=True, help="Mode hemat token compact")
    p_keg.add_argument("--q", help="Pencarian kata kunci")
    p_keg.set_defaults(func=cmd_kegiatan)

    # mitra
    p_mitra = subparsers.add_parser("mitra", help="Lookup statistics partners & remaining SBML limit")
    p_mitra.add_argument("--tahun", type=int, default=2026, help="Tahun kemitraan")
    p_mitra.add_argument("--bulan", type=int, help="Bulan kalender aktif")
    p_mitra.add_argument("--available-only", action="store_true", help="Hanya mitra dengan sisa kuota SBML")
    p_mitra.add_argument("--with-sbml", action="store_true", help="Sertakan kalkulasi SBML")
    p_mitra.add_argument("--compact", action="store_true", default=True, help="Mode hemat token")
    p_mitra.add_argument("--q", help="Pencarian nama/NIK/Sobat mitra")
    p_mitra.set_defaults(func=cmd_mitra)

    # check
    p_check = subparsers.add_parser("check", help="Pre-flight check allocation feasibility")
    p_check.add_argument("--honor-id", required=True, help="ID Pos Honor")
    p_check.add_argument("--mitra-id", type=int, required=True, help="ID Mitra")
    p_check.add_argument("--target", type=float, required=True, help="Volume target dokumen")
    p_check.set_defaults(func=cmd_check)

    # alokasi
    p_alo = subparsers.add_parser("alokasi", help="Create allocation, generate SPK & BAST")
    p_alo.add_argument("--honor-id", required=True, help="ID Pos Honor")
    p_alo.add_argument("--mitra-id", type=int, required=True, help="ID Mitra")
    p_alo.add_argument("--target", type=float, required=True, help="Volume target")
    p_alo.set_defaults(func=cmd_alokasi)

    # kontrak
    p_kon = subparsers.add_parser("kontrak", help="Get monthly SPK contract and print URL")
    p_kon.add_argument("--tahun", type=int, default=2026, required=True)
    p_kon.add_argument("--bulan", type=int, required=True)
    p_kon.add_argument("--mitra-id", type=int, required=True)
    p_kon.set_defaults(func=cmd_kontrak)

    # bast
    p_bast = subparsers.add_parser("bast", help="Get BAST document and print URL")
    p_bast.add_argument("--tahun", type=int, default=2026, required=True)
    p_bast.add_argument("--bulan", type=int, required=True)
    p_bast.add_argument("--kegiatan-id", required=True)
    p_bast.add_argument("--mitra-id", type=int, required=True)
    p_bast.set_defaults(func=cmd_bast)

    # penugasan
    p_pen = subparsers.add_parser("penugasan", help="Check travel assignment (Surat Tugas) status")
    p_pen.add_argument("--q", help="Pencarian NIP atau nama pegawai")
    p_pen.add_argument("--status", help="Filter status penugasan")
    p_pen.add_argument("--tahun", type=int)
    p_pen.add_argument("--bulan", type=int)
    p_pen.set_defaults(func=cmd_penugasan)

    # approve / setujui (Strictly Guarded)
    p_app = subparsers.add_parser("approve", help="Approve Surat Tugas (Forbidden for AI Agent)")
    p_app.set_defaults(func=cmd_approve_guard)
    p_set = subparsers.add_parser("setujui", help="Setujui Surat Tugas (Forbidden for AI Agent)")
    p_set.set_defaults(func=cmd_approve_guard)

    args = parser.parse_args()

    # Pre-execution permission gate
    allowed, reason = check_caller_permission()
    if not allowed:
        print(json.dumps({"error": True, "status": "forbidden", "message": reason}, indent=2, ensure_ascii=False))
        sys.exit(1)

    args.func(args)

if __name__ == "__main__":
    main()
