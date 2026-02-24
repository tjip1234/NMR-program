#!/usr/bin/env python3
"""Compare NMRPipe files produced by legacy vs Rust conversion tools.

Reads the 2048-byte FDATA header (512 × f32) and the data payload,
then prints differences in header parameters and data statistics.
"""

import struct
import sys
import os
import numpy as np

FDATA_SIZE = 512  # 512 float32 values = 2048 bytes
HEADER_BYTES = FDATA_SIZE * 4

# Key FDATA parameter names (index → name) for readable output
FDATA_NAMES = {
    0: "FDMAGIC",
    1: "FDFLTFORMAT",
    2: "FDFLTORDER",
    15: "FDSIZE",
    24: "FD2DPHASE",
    55: "FDDMXVAL",
    56: "FDDMXFLAG",
    57: "FDDELTATR",
    99: "FDSPECNUM",
    106: "FDREALSIZE",
    111: "FDQUADFLAG",
    199: "FDTEMPERATURE",
    219: "FDFILECOUNT",
    222: "FDPIPEFLAG",
    # Dimension params (X=CUR_XDIM=1)
    # NDSIZE offsets: dims use param tables
}


def read_nmrpipe(path):
    """Read an NMRPipe file → (header_array[512], data_array)."""
    with open(path, "rb") as f:
        raw = f.read()

    if len(raw) < HEADER_BYTES:
        raise ValueError(f"{path}: file too small ({len(raw)} bytes)")

    hdr = np.frombuffer(raw[:HEADER_BYTES], dtype=np.float32).copy()
    data = np.frombuffer(raw[HEADER_BYTES:], dtype=np.float32).copy()

    return hdr, data


def compare_headers(hdr_a, hdr_b, label_a="legacy", label_b="rust"):
    """Compare two 512-element header arrays, return list of diffs."""
    diffs = []
    for i in range(FDATA_SIZE):
        va, vb = hdr_a[i], hdr_b[i]
        if va != vb:
            name = FDATA_NAMES.get(i, f"[{i}]")
            diffs.append((i, name, va, vb))
    return diffs


def compare_data(data_a, data_b, label_a="legacy", label_b="rust"):
    """Compare two data arrays, return statistics dict."""
    stats = {}
    stats["len_a"] = len(data_a)
    stats["len_b"] = len(data_b)

    min_len = min(len(data_a), len(data_b))
    if min_len == 0:
        stats["max_abs_diff"] = float("nan")
        stats["mean_abs_diff"] = float("nan")
        stats["rms_diff"] = float("nan")
        stats["correlation"] = float("nan")
        return stats

    a = data_a[:min_len]
    b = data_b[:min_len]
    diff = a - b
    abs_diff = np.abs(diff)

    stats["max_abs_diff"] = float(np.max(abs_diff))
    stats["mean_abs_diff"] = float(np.mean(abs_diff))
    stats["rms_diff"] = float(np.sqrt(np.mean(diff**2)))

    # Relative error (vs max amplitude)
    max_amp = max(np.max(np.abs(a)), np.max(np.abs(b)), 1e-30)
    stats["max_rel_diff"] = stats["max_abs_diff"] / max_amp
    stats["rms_rel_diff"] = stats["rms_diff"] / max_amp

    # Correlation
    if np.std(a) > 0 and np.std(b) > 0:
        stats["correlation"] = float(np.corrcoef(a, b)[0, 1])
    else:
        stats["correlation"] = float("nan")

    return stats


def main():
    legacy_dir = os.path.join(os.path.dirname(__file__), "legacy")
    rust_dir = os.path.join(os.path.dirname(__file__), "rust")

    # Match files by base name pattern
    pairs = [
        ("tpp_proton", "tpp_proton_legacy.dat", "tpp_proton_rust.dat"),
        ("tpp_carbon", "tpp_carbon_legacy.dat", "tpp_carbon_rust.dat"),
        ("tpp_phos", "tpp_phos_legacy.dat", "tpp_phos_rust.dat"),
        ("cb_proton", "cb_proton_legacy.dat", "cb_proton_rust.dat"),
        ("cb_carbon", "cb_carbon_legacy.dat", "cb_carbon_rust.dat"),
    ]

    # Check for bruker
    if os.path.exists(os.path.join(legacy_dir, "aspirin_legacy.dat")):
        pairs.append(("aspirin_bruk", "aspirin_legacy.dat", "aspirin_rust.dat"))

    all_pass = True
    for name, legacy_file, rust_file in pairs:
        legacy_path = os.path.join(legacy_dir, legacy_file)
        rust_path = os.path.join(rust_dir, rust_file)

        if not os.path.exists(legacy_path):
            print(f"  SKIP {name}: {legacy_file} not found")
            continue
        if not os.path.exists(rust_path):
            print(f"  SKIP {name}: {rust_file} not found")
            continue

        print(f"\n{'='*60}")
        print(f"  {name}")
        print(f"{'='*60}")

        hdr_a, data_a = read_nmrpipe(legacy_path)
        hdr_b, data_b = read_nmrpipe(rust_path)

        # File sizes
        sz_a = os.path.getsize(legacy_path)
        sz_b = os.path.getsize(rust_path)
        print(f"  File sizes:  legacy={sz_a:,}  rust={sz_b:,}  diff={sz_b-sz_a:+,}")

        # Header comparison
        hdr_diffs = compare_headers(hdr_a, hdr_b)
        if hdr_diffs:
            print(f"  Header diffs ({len(hdr_diffs)} fields):")
            for idx, fname, va, vb in hdr_diffs:
                # Show as int if it looks like one
                if va == int(va) and vb == int(vb):
                    print(f"    {fname:20s}  legacy={int(va):12d}  rust={int(vb):12d}")
                else:
                    print(f"    {fname:20s}  legacy={va:14.6g}  rust={vb:14.6g}")
        else:
            print(f"  Header: IDENTICAL")

        # Data comparison
        stats = compare_data(data_a, data_b)
        print(f"  Data points: legacy={stats['len_a']:,}  rust={stats['len_b']:,}")
        print(f"  Max  abs diff:  {stats['max_abs_diff']:.6g}")
        print(f"  Mean abs diff:  {stats['mean_abs_diff']:.6g}")
        print(f"  RMS  diff:      {stats['rms_diff']:.6g}")
        print(f"  Max  rel diff:  {stats['max_rel_diff']:.6e}")
        print(f"  RMS  rel diff:  {stats['rms_rel_diff']:.6e}")
        print(f"  Correlation:    {stats['correlation']:.10f}")

        # Pass/fail judgment
        if stats["correlation"] < 0.999:
            print(f"  ** FAIL: correlation too low")
            all_pass = False
        elif stats["max_rel_diff"] > 0.01:
            print(f"  ** WARN: max relative diff > 1%")
        else:
            print(f"  PASS")

    print(f"\n{'='*60}")
    if all_pass:
        print("  ALL COMPARISONS PASSED")
    else:
        print("  SOME COMPARISONS FAILED")
    print(f"{'='*60}")

    return 0 if all_pass else 1


if __name__ == "__main__":
    sys.exit(main())
