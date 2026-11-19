"""Grid, pitch-aligned and lyrics-aligned onsets compared, per syllable.

usage: compare.py pitch.json lyrics.json
"""
import sys, json
import numpy as np

pitch = json.load(open(sys.argv[1]))
lyr = {int(k): v for k, v in json.load(open(sys.argv[2])).items()}

def stats(name, d):
    d = np.asarray(d) * 1000
    a = np.abs(d)
    print(f"{name:42s} n={len(d):3d} median {np.median(d):+5.0f} ms, IQR {np.percentile(d,25):+4.0f}..{np.percentile(d,75):+4.0f}, "
          f"|d|: mean {a.mean():4.0f}, ≤30 {np.mean(a<=.03*1000):4.0%}, ≤50 {np.mean(a<=50):4.0%}, ≤100 {np.mean(a<=100):4.0%}, >300 {np.mean(a>300):4.0%}")

rows = [(p, lyr[p["i"]]) for p in pitch if p["i"] in lyr]
stats("lyrics consonant − grid", [l["start"] - p["start"] for p, l in rows])
stats("lyrics vowel − grid", [l["vowel"] - p["start"] for p, l in rows])
stats("pitch-aligned − grid", [p["sung"] - p["start"] for p, l in rows])
stats("pitch-aligned − lyrics vowel", [p["sung"] - l["vowel"] for p, l in rows])
stats("pitch-aligned − lyrics consonant", [p["sung"] - l["start"] for p, l in rows])
good = [(p, l) for p, l in rows if abs(l["vowel"] - p["start"]) < 0.3]
stats("  (only where lyrics within 0.3 s of grid)", [p["sung"] - l["vowel"] for p, l in good])

# The singer's lag phrase by phrase: sections of the song.
print("\nlyrics vowel − grid by section (median ms):")
cuts = [0, 30, 60, 90, 120, 150, 180, 230]
for a, b in zip(cuts, cuts[1:]):
    d = [l["vowel"] - p["start"] for p, l in rows if a <= p["start"] < b and abs(l["vowel"] - p["start"]) < 0.4]
    if d: print(f"  {a:3d}–{b:3d} s: {np.median(d)*1000:+4.0f} (n={len(d)}, IQR {np.percentile(d,25)*1000:+.0f}..{np.percentile(d,75)*1000:+.0f})")

print("\nworst disagreements (s):")
for p, l in sorted(rows, key=lambda r: -abs(r[0]["sung"] - r[1]["vowel"]))[:8]:
    print(f"  note {p['i']:3d} {l['text']:10s} grid {p['start']:7.2f} pitch {p['sung']:7.2f} cons {l['start']:7.2f} vowel {l['vowel']:7.2f} score {l['score']:.2f}")
