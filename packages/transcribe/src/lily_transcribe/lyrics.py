"""The words of a vocals stem, laid on its notes (DECISIONS D134).

Whisper hears the words with the time of each; the hyphenation dictionary of
the language Whisper heard splits each word into syllables, which share the
word's time by their letters; then the syllables are laid on the notes in
order, each on the note starting nearest it. A note too far from any
syllable gets none (a skip in `\\lyricsto`), and a syllable with no note of its
own is sung on the note before it.
"""

from __future__ import annotations

import contextlib
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path

# Whisper's model on MLX; `LILY_TRANSCRIBE_WHISPER` names another.
WHISPER = "mlx-community/whisper-large-v3-turbo"
# Seconds a note waits for a syllable: what giving a note none costs.
SKIP_NOTE = 0.25
# What singing a syllable on the note before it costs.
SHARE_NOTE = 0.35


@dataclass
class Word:
    text: str
    start: float
    end: float


@dataclass
class Syllable:
    text: str
    start: float
    # Its word goes on in the next syllable.
    hyphen: bool


def hear(stem: Path) -> tuple[list[Word], str]:
    """Whisper's words in `stem`, with their times in seconds, and the language it heard."""
    import mlx_whisper

    with contextlib.redirect_stdout(sys.stderr):
        result = mlx_whisper.transcribe(
            str(stem),
            path_or_hf_repo=os.environ.get("LILY_TRANSCRIBE_WHISPER", WHISPER),
            word_timestamps=True,
            # A song repeats itself; conditioned on what came before, Whisper
            # tends to hear the last chorus again where there is none.
            condition_on_previous_text=False,
            verbose=None,
        )
    words = [
        Word(word["word"].strip(), float(word["start"]), float(word["end"]))
        for segment in result.get("segments", [])
        for word in segment.get("words", [])
        if word["word"].strip()
    ]
    return words, str(result.get("language") or "en")


_LETTERS = re.compile(r"^(\W*)(.*?)(\W*)$", re.UNICODE)


def syllables(words: list[Word], language: str) -> list[Syllable]:
    """Each word split by `language`'s hyphenation, a syllable's time by its share of the letters."""
    import pyphen

    found = pyphen.language_fallback(language) or pyphen.language_fallback("en")
    dictionary = pyphen.Pyphen(lang=found, left=2, right=2) if found else None
    result: list[Syllable] = []
    for word in words:
        # Whisper can leave a line break inside a word: each side is a word of its own.
        for whole, start, end in _shared(word.text.split(), word.start, word.end):
            before, core, after = _LETTERS.match(whole).groups()
            parts = dictionary.inserted(core).split("-") if dictionary and core else [core]
            parts = [part for part in parts if part] or [core]
            parts[0] = before + parts[0]
            parts[-1] = parts[-1] + after
            for index, (part, at, _) in enumerate(_shared(parts, start, end)):
                result.append(Syllable(part, at, index < len(parts) - 1))
    return result


def _shared(parts: list[str], start: float, end: float) -> list[tuple[str, float, float]]:
    """Each of `parts` with the stretch of `start` to `end` its share of the letters gives it."""
    letters = sum(len(part) for part in parts) or 1
    per_letter = (end - start) / letters
    shares = []
    for part in parts:
        shares.append((part, start, start + per_letter * len(part)))
        start += per_letter * len(part)
    return shares


def align(sung: list[Syllable], onsets: list[float]) -> list[tuple[int, str, bool]]:
    """The syllables laid on the notes starting at `onsets`, in order: for each
    note given words, its index, its text and whether a hyphen follows.

    A match costs the seconds between the syllable and its note; a note without
    a syllable costs SKIP_NOTE, a syllable sung on the note before SHARE_NOTE.
    Where that lays no syllable on a note of its own, as when every one is
    further from every note than the two together, each is sung on the note
    nearest it.
    """
    count, notes = len(sung), len(onsets)
    if count == 0 or notes == 0:
        return []
    inf = float("inf")
    cost = [[inf] * (notes + 1) for _ in range(count + 1)]
    step = [[""] * (notes + 1) for _ in range(count + 1)]
    cost[0][0] = 0.0
    for i in range(count + 1):
        for j in range(notes + 1):
            here = cost[i][j]
            if here == inf:
                continue
            if j < notes and here + SKIP_NOTE < cost[i][j + 1]:
                cost[i][j + 1] = here + SKIP_NOTE
                step[i][j + 1] = "skip"
            if i < count and j < notes:
                matched = here + abs(sung[i].start - onsets[j])
                if matched < cost[i + 1][j + 1]:
                    cost[i + 1][j + 1] = matched
                    step[i + 1][j + 1] = "match"
            if i < count and j > 0 and here + SHARE_NOTE < cost[i + 1][j]:
                cost[i + 1][j] = here + SHARE_NOTE
                step[i + 1][j] = "share"
    # Back from the end: which syllables each note sings.
    on: dict[int, list[Syllable]] = {}
    shared: list[Syllable] = []
    i, j = count, notes
    while i > 0 or j > 0:
        move = step[i][j]
        if move == "skip":
            j -= 1
        elif move == "share":
            shared.insert(0, sung[i - 1])
            i -= 1
        else:
            on[j - 1] = [sung[i - 1], *shared]
            shared = []
            i -= 1
            j -= 1
    # Shared before any note had one: they open the first note's words.
    if shared and on:
        first = min(on)
        on[first] = [*shared, *on[first]]
    # No note had one: nothing comes before them, and a song's words are not dropped for it.
    elif shared:
        for syllable in shared:
            nearest = min(range(notes), key=lambda note: abs(onsets[note] - syllable.start))
            on.setdefault(nearest, []).append(syllable)
    laid = []
    for note in sorted(on):
        group = on[note]
        text = ""
        for index, syllable in enumerate(group):
            text += syllable.text if index == 0 or group[index - 1].hyphen else f" {syllable.text}"
        laid.append((note, text, group[-1].hyphen))
    return laid
