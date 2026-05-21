"""Verify golden files with Python lzstring."""
from lzstring import LZString
import json, os

lz = LZString()
golden_dir = os.path.dirname(os.path.abspath(__file__))

results = {"decompress": [], "roundtrip": []}

files = sorted(os.listdir(golden_dir))
for fname in files:
    if not fname.endswith(".json") or fname == "verify_golden.py":
        continue
    path = os.path.join(golden_dir, fname)
    with open(path) as f:
        data = json.load(f)
    orig_input = data["input"]
    encoded = data["encoded"]

    # Test 1: Decompress
    decoded = lz.decompressFromBase64(encoded)
    if decoded != orig_input:
        results["decompress"].append(f"{fname}: FAIL (got {repr(decoded[:50])})")

    # Test 2: Roundtrip
    re_encoded = lz.compressToBase64(orig_input)
    if re_encoded != encoded:
        msg = f"{fname}: diff"
        if orig_input == "":
            msg += f" (empty string: expected {repr(encoded)}, got {repr(re_encoded)})"
        else:
            msg += f" (expected {repr(encoded[:50])}, got {repr(re_encoded[:50])})"
        results["roundtrip"].append(msg)

print(f"=== Golden File Verification Report ===")
print(f"Total files: {len([f for f in files if f.endswith('.json') and f != 'verify_golden.py'])}")
print()

print("--- Decompress Test ---")
print(f"  Failures: {len(results['decompress'])}")
for r in results["decompress"]:
    print(f"  FAIL: {r}")
if not results["decompress"]:
    print(f"  All passed!")
print()

print("--- Roundtrip Test ---")
print(f"  Failures: {len(results['roundtrip'])}")
for r in results["roundtrip"]:
    if "empty" in r.lower():
        print(f"  NOTE: {r}")
    else:
        print(f"  FAIL: {r}")
if not results["roundtrip"]:
    print(f"  All passed!")
print()

# Self-consistency check
print("--- Self-Consistency Test ---")
sc_fails = 0
for fname in files:
    if not fname.endswith(".json") or fname == "verify_golden.py":
        continue
    path = os.path.join(golden_dir, fname)
    with open(path) as f:
        data = json.load(f)
    orig_input = data["input"]
    encoded = lz.compressToBase64(orig_input)
    decoded = lz.decompressFromBase64(encoded)
    if decoded != orig_input:
        sc_fails += 1
        print(f"  FAIL: {fname}")
print(f"  Failures: {sc_fails}")
if sc_fails == 0:
    print(f"  All passed!")

print()
# Summary for evidence file
print("=== EVIDENCE SUMMARY ===")
print(f"DECOMPRESS_OK={len(results['decompress']) == 0}")
print(f"ROUNDTRIP_OK={len(results['roundtrip']) == 0}")
print(f"SELF_CONSISTENT_OK={sc_fails == 0}")
