"""Where the sung f0 crosses from one note's pitch to the next's, for every
note whose pitch differs from the note before it by 2 semitones or more and
follows it without a rest: a model-free measure of when the singer moved.

The crossing is the first frame within ±0.3 s of the grid whose f0 (median
over 3 frames) is nearer the new pitch than the old, after which it stays
nearer for 50 ms.

usage: pitch_changes.py notes.json features.npz lyrics.json
"""
import sys, json
import numpy as np

notes = json.load(open(sys.argv[1]))["notes"]
F = np.load(sys.argv[2]); hop = float(F["hop"])
lyr = {int(k): v for k, v in json.load(open(sys.argv[3])).items()}
f0, voiced = F["f0"], F["voiced"]
m = np.where(voiced & np.isfinite(f0), 69 + 12 * np.log2(np.where(f0 > 0, f0, 1) / 440), np.nan)
from scipy.ndimage import median_filter
m3 = np.where(np.isfinite(m), median_filter(np.nan_to_num(m), 3), np.nan)

res = []
for p, n in zip(notes, notes[1:]):
    if abs(n["pitch"] - p["pitch"]) < 2 or n["start"] - p["end"] > 0.02: continue
    mid = (n["pitch"] + p["pitch"]) / 2; up = n["pitch"] > p["pitch"]
    a, b = int((n["start"] - 0.3) / hop), int((n["start"] + 0.3) / hop)
    seg = m3[a:b]
    side = (seg > mid) if up else (seg < mid)
    side &= np.isfinite(seg)
    hit = next((k for k in range(len(seg) - 5) if side[k: k + 5].all()), None)
    if hit is None: continue
    t = (a + hit) * hop
    res.append((n["i"], t - n["start"], (t - lyr[n["i"]]["vowel"]) if n["i"] in lyr else None, n["syllable"]))

d = np.array([r[1] for r in res]) * 1000
print(f"{len(res)} pitch changes found; crossing − grid: median {np.median(d):+.0f} ms, IQR {np.percentile(d,25):+.0f}..{np.percentile(d,75):+.0f}, "
      f"|d|≤30 {np.mean(abs(d)<=30):.0%}, ≤50 {np.mean(abs(d)<=50):.0%}, ≤100 {np.mean(abs(d)<=100):.0%}")
v = np.array([r[2] for r in res if r[2] is not None]) * 1000
print(f"  of them with a syllable ({len(v)}): crossing − lyrics vowel: median {np.median(v):+.0f} ms, IQR {np.percentile(v,25):+.0f}..{np.percentile(v,75):+.0f}, |d|≤50 {np.mean(abs(v)<=50):.0%}, ≤100 {np.mean(abs(v)<=100):.0%}")
s = np.array([r[1] for r in res if r[3] is None]) * 1000
if len(s): print(f"  in melismas (no syllable, {len(s)}): crossing − grid median {np.median(s):+.0f} ms, IQR {np.percentile(s,25):+.0f}..{np.percentile(s,75):+.0f}")
