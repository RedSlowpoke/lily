"""f0 (pYIN) and onset strength of a vocals stem, 10 ms frames.

usage: features.py vocals.wav out.npz
"""
import sys, time
import numpy as np, librosa

SR, HOP = 16000, 160
y, _ = librosa.load(sys.argv[1], sr=SR, mono=True)
t0 = time.time()
f0, voiced, vprob = librosa.pyin(y, fmin=70, fmax=1000, sr=SR, frame_length=1024, hop_length=HOP)
t1 = time.time()
onset = librosa.onset.onset_strength(y=y, sr=SR, hop_length=HOP, n_fft=1024)
rms = librosa.feature.rms(y=y, frame_length=1024, hop_length=HOP)[0]
n = min(len(f0), len(onset), len(rms))
np.savez(sys.argv[2], f0=f0[:n], voiced=voiced[:n], vprob=vprob[:n], onset=onset[:n], rms=rms[:n], hop=HOP / SR)
print(f"{len(y)/SR:.1f} s of audio, pyin {t1-t0:.1f} s, {np.mean(voiced):.0%} voiced")
