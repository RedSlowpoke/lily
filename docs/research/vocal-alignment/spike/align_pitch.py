"""Each score note's sung onset and offset from the vocals' f0 and onsets.

A Viterbi over every note's shift from its grid time (±W, 10 ms steps): a
note's cost at a shift is how badly the f0 just after it matches the note's
pitch, how weak the vocals' onset strength is there, and how far it is from
the grid; moving from one note's shift to the next costs by the change, so
the singer's lag is locally coherent. Notes keep their order.

usage: align_pitch.py notes.json features.npz out.json
"""
import sys, json
import numpy as np

W = 0.35          # seconds a note may move from the grid
STEP = 0.01
W_ONSET = 0.6     # weight of the onset strength
W_PRIOR = 0.25    # weight of the distance from the grid (per W)
MU = 1.5          # cost a second of change in shift between neighbours
MIN_GAP = 0.05    # seconds between two onsets at least
UNVOICED = 0.55   # pitch cost of an unvoiced frame (a consonant, a breath)

notes = json.load(open(sys.argv[1]))["notes"]
F = np.load(sys.argv[2])
hop = float(F["hop"])
f0, voiced, onset = F["f0"], F["voiced"], F["onset"]
n_frames = len(f0)
midi = np.where(voiced & np.isfinite(f0), 69 + 12 * np.log2(np.where(f0 > 0, f0, 1) / 440), np.nan)

# The octave the singer is in, against the score.
def frames(t0, t1):
    a, b = max(0, int(round(t0 / hop))), min(n_frames, int(round(t1 / hop)))
    return slice(a, max(a, b))
diffs = []
for n in notes:
    m = midi[frames(n["start"] + 0.05, n["end"])]
    m = m[np.isfinite(m)]
    if len(m): diffs.append(np.median(m) - n["pitch"])
shift = 12 * round(np.median(diffs) / 12) if diffs else 0
print("octave shift of the singer:", shift)

on = onset / (np.percentile(onset, 95) + 1e-9)
on = np.clip(on, 0, 1)
# best onset within ±20 ms of each frame
on_near = np.array([on[max(0, i - 2): i + 3].max() for i in range(n_frames)])

def pitch_cost(t, n):
    L = min(n["end"] - n["start"], 0.25)
    m = midi[frames(t + 0.03, t + max(L, 0.06))]
    if len(m) == 0: return UNVOICED
    c = np.where(np.isfinite(m), np.minimum(np.abs(m - n["pitch"] - shift), 3) / 3, UNVOICED)
    return float(c.mean())

shifts = np.arange(-W, W + 1e-9, STEP)
S = len(shifts)
E = np.zeros((len(notes), S))
for i, n in enumerate(notes):
    for k, d in enumerate(shifts):
        t = n["start"] + d
        fi = min(n_frames - 1, max(0, int(round(t / hop))))
        E[i, k] = pitch_cost(t, n) + W_ONSET * (1 - on_near[fi]) + W_PRIOR * abs(d) / W

# Viterbi
D = np.full((len(notes), S), np.inf); B = np.zeros((len(notes), S), int)
D[0] = E[0]
for i in range(1, len(notes)):
    gap = notes[i]["start"] - notes[i - 1]["end"]
    mu = MU * (0.3 if gap > 0.3 else 1.0)       # after a rest the lag may reset
    for k in range(S):
        t = notes[i]["start"] + shifts[k]
        prev_t = notes[i - 1]["start"] + shifts
        ok = prev_t + MIN_GAP <= t
        cost = D[i - 1] + mu * np.abs(shifts[k] - shifts)
        cost[~ok] = np.inf
        j = int(np.argmin(cost)); D[i, k] = cost[j] + E[i, k]; B[i, k] = j
k = int(np.argmin(D[-1])); path = [k]
for i in range(len(notes) - 1, 0, -1):
    k = B[i, k]; path.append(k)
path = path[::-1]

out = []
for i, n in enumerate(notes):
    d = float(shifts[path[i]]); t = n["start"] + d
    nxt = notes[i + 1]["start"] + float(shifts[path[i + 1]]) if i + 1 < len(notes) else t + 10
    # offset: where the voice stops for 80 ms, searched from half the note on
    end = min(n["end"] + d, nxt)
    if notes[i + 1]["start"] - n["end"] > 0.05 if i + 1 < len(notes) else True:
        a = frames(t + 0.5 * (n["end"] - n["start"]), min(n["end"] + d + 0.3, nxt))
        v = voiced[a]
        for j in range(len(v) - 8):
            if not v[j: j + 8].any():
                end = (a.start + j) * hop; break
    out.append({**n, "sung": t, "sung_end": end, "shift": d,
                "pitch_cost": pitch_cost(t, n), "grid_pitch_cost": pitch_cost(n["start"], n)})
json.dump(out, open(sys.argv[3], "w"), indent=0)

sh = np.array([o["shift"] for o in out]) * 1000
print(f"{len(out)} notes; shift from the grid ms: median {np.median(sh):+.0f}, "
      f"IQR {np.percentile(sh,25):+.0f}..{np.percentile(sh,75):+.0f}, "
      f"|shift|>30 {np.mean(abs(sh)>30):.0%}, >50 {np.mean(abs(sh)>50):.0%}, >100 {np.mean(abs(sh)>100):.0%}, at ±W {np.mean(abs(sh)>=W*1000-1):.0%}")
pc = np.array([o["pitch_cost"] for o in out]); gc = np.array([o["grid_pitch_cost"] for o in out])
print(f"pitch cost at the note's start: grid {gc.mean():.3f}, aligned {pc.mean():.3f}")
