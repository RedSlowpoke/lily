"""The score's lead notes and syllables in recording time (grid timing).

usage: prepare.py score.midi out.json --bpm 113.76 --offset -0.33 --lead 1 --lyrics 7
recording time = music time at the BPM − offset (D155).
"""
import argparse, json
import mido, pretty_midi

ap = argparse.ArgumentParser()
ap.add_argument("midi"); ap.add_argument("out")
ap.add_argument("--bpm", type=float, required=True)
ap.add_argument("--offset", type=float, required=True)
ap.add_argument("--lead", type=int, default=1)
ap.add_argument("--lyrics", type=int, default=7)
a = ap.parse_args()

pm = pretty_midi.PrettyMIDI(a.midi)
mf = mido.MidiFile(a.midi)
first_tempo = next(e.tempo for e in mf.tracks[0] if e.type == "set_tempo")
stretch = (60e6 / first_tempo) / a.bpm
rec = lambda s: s * stretch - a.offset

# pretty_midi drops empty tracks; find the lead's instrument by its track name.
names = [next((e.name for e in t if e.type == "track_name"), "") for t in mf.tracks]
lead = next(i for i in pm.instruments if i.name == names[a.lead])
notes = sorted(lead.notes, key=lambda n: n.start)

t, syl = 0, []
for e in mf.tracks[a.lyrics]:
    t += e.time
    if e.type == "lyrics":
        syl.append((pm.tick_to_time(t), e.text))

out = []
si = 0
for k, n in enumerate(notes):
    s = None
    while si < len(syl) and syl[si][0] < n.start - 1e-6:
        si += 1
    if si < len(syl) and abs(syl[si][0] - n.start) < 1e-6:
        s = syl[si][1]; si += 1
    out.append({"i": k, "pitch": n.pitch, "start": rec(n.start), "end": rec(n.end), "syllable": s})

json.dump({"notes": out, "stretch": stretch, "offset": a.offset}, open(a.out, "w"), indent=0)
print(len(out), "notes,", sum(1 for o in out if o["syllable"]), "with a syllable of", len(syl))
