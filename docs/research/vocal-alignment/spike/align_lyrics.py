"""The score's syllables forced-aligned to the vocals with torchaudio's MMS_FA.

Each syllable is aligned as a word (MMS_FA has no space token). A syllable's
onset is its first character's start (the consonant, for most) and its
vowel's start (the first of a e i o u y).

usage: align_lyrics.py notes.json vocals.wav out.json
"""
import sys, json, re, time
import torch, torchaudio
from torchaudio.pipelines import MMS_FA as bundle

notes = json.load(open(sys.argv[1]))["notes"]
sylls = [(n["i"], n["syllable"]) for n in notes if n["syllable"]]
norm = [re.sub(r"[^a-z']", "", s.lower()) for _, s in sylls]
keep = [k for k, w in enumerate(norm) if w]
words = [norm[k] for k in keep]

import librosa; y, sr = librosa.load(sys.argv[2], sr=None, mono=True); wave = torch.from_numpy(y)[None]
wave = torchaudio.functional.resample(wave.mean(0, keepdim=True), sr, bundle.sample_rate)
dev = "cpu"
model = bundle.get_model(with_star=False).to(dev).eval()
t0 = time.time()
chunk = 30 * bundle.sample_rate
ems = []
with torch.inference_mode():
    for a in range(0, wave.shape[1], chunk):
        e, _ = model(wave[:, a: a + chunk].to(dev))
        ems.append(e[0].cpu())
emission = torch.cat(ems)
fps = emission.shape[0] / (wave.shape[1] / bundle.sample_rate)
tok = bundle.get_tokenizer(); aligner = bundle.get_aligner()
spans = aligner(emission, tok(words))
print(f"{len(words)} syllables, emission {emission.shape[0]} frames ({fps:.2f}/s), {time.time()-t0:.1f} s")

out = {}
for k, w, sp in zip(keep, words, spans):
    i = sylls[k][0]
    v = next((s for c, s in zip(w, sp) if c in "aeiouy"), sp[0])
    out[i] = {"text": sylls[k][1], "start": sp[0].start / fps, "vowel": v.start / fps, "end": sp[-1].end / fps,
              "score": float(sum(s.score for s in sp) / len(sp))}
json.dump(out, open(sys.argv[3], "w"), indent=0)
