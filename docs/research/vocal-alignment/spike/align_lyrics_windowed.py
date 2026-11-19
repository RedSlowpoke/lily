"""The score's syllables forced-aligned phrase by phrase, each in a window the
score's grid gives (its first syllable − PAD to its last note's end + PAD).

A phrase ends at a syllable with a full stop (the score's Karaoke phrases,
D146). Uses MMS_FA with the <star> token, so audio in the window the phrase
does not cover is absorbed rather than forced onto the words.

usage: align_lyrics_windowed.py notes.json vocals.wav out.json [--model mms|en]
"""
import sys, json, re, time
import numpy as np, torch, torchaudio, librosa

PAD = 0.6
notes = json.load(open(sys.argv[1]))["notes"]
which = sys.argv[4] if len(sys.argv) > 4 else "mms"
by_i = {n["i"]: n for n in notes}

if which == "mms":
    from torchaudio.pipelines import MMS_FA as bundle
    model = bundle.get_model(with_star=True).eval()
    labels = bundle.get_labels(star="*")
    sr0 = bundle.sample_rate
    def norm(s): return re.sub(r"[^a-z']", "", s.lower())
    blank = 0
else:  # MIT English model
    bundle = torchaudio.pipelines.WAV2VEC2_ASR_BASE_960H
    model = bundle.get_model().eval()
    labels = bundle.get_labels()  # ('-', '|', 'E', ...)
    sr0 = bundle.sample_rate
    def norm(s): return re.sub(r"[^A-Z']", "", s.upper())
    blank = 0
idx = {c: k for k, c in enumerate(labels)}

y, sr = librosa.load(sys.argv[2], sr=sr0, mono=True)
wave = torch.from_numpy(y)

# phrases
sylls = [(n["i"], n["syllable"]) for n in notes if n["syllable"]]
phrases, cur = [], []
for i, s in sylls:
    if norm(s): cur.append((i, s))
    if s.rstrip().endswith((".", "!", "?")) and cur:
        phrases.append(cur); cur = []
if cur: phrases.append(cur)

out = {}
t0 = time.time()
for ph in phrases:
    first, last = by_i[ph[0][0]], by_i[ph[-1][0]]
    a = max(0.0, first["start"] - PAD); b = last["end"] + PAD
    seg = wave[int(a * sr0): int(b * sr0)][None]
    with torch.inference_mode():
        em, _ = model(seg)
        em = torch.log_softmax(em, -1)
    fps = em.shape[1] / (seg.shape[1] / sr0)
    # tokens: [*] w1 [*] w2 ... (star between syllables absorbs breaths and other voices)
    toks, owner = [], []
    for k, (i, s) in enumerate(ph):
        if which == "mms": toks.append(idx["*"]); owner.append(None)
        elif k: toks.append(idx["|"]); owner.append(None)
        for c in norm(s):
            if c in idx: toks.append(idx[c]); owner.append(k)
    if which == "mms": toks.append(idx["*"]); owner.append(None)
    ali, scores = torchaudio.functional.forced_align(em, torch.tensor([toks], dtype=torch.int32), blank=blank)
    ali, scores = ali[0].numpy(), scores[0].exp().numpy()
    # token spans
    spans = []; t = 0; j = -1
    for f, tok in enumerate(ali):
        if tok == blank: continue
        if f == 0 or ali[f - 1] != tok or (spans and spans[-1][2] != f - 1):
            j += 1; spans.append([j, f, f, [scores[f]]])
        else:
            spans[-1][2] = f; spans[-1][3].append(scores[f])
    # map spans to tokens in order (ali repeats tokens; consecutive frames of one token = one span)
    if len(spans) != len(toks):
        # a repeated letter (e.g. "ll") merges; fall back to merge_tokens
        ts = torchaudio.functional.merge_tokens(torch.tensor(ali), torch.tensor(scores))
        spans = [[k, s.start, s.end - 1, [s.score]] for k, s in enumerate(ts)]
    for k, (i, s) in enumerate(ph):
        mine = [sp for sp, o in zip(spans, owner) if o == k]
        if not mine: continue
        chars = [c for c in norm(s) if c in idx]
        vow = next((sp for c, sp in zip(chars, mine) if c.lower() in "aeiouy"), mine[0])
        out[i] = {"text": s, "start": a + mine[0][1] / fps, "vowel": a + vow[1] / fps, "end": a + (mine[-1][2] + 1) / fps,
                  "score": float(np.mean([np.mean(sp[3]) for sp in mine]))}
json.dump(out, open(sys.argv[3], "w"), indent=0)
print(f"{which}: {len(phrases)} phrases, {len(out)} syllables aligned, {time.time()-t0:.1f} s")
