"""A score's tempo and bar 1 from its recording (DECISIONS D159).

Lily Studio's *From the recording* fills a score's `% BPM:` and `% Audio
offset:` (D155) from the beats Beat This finds in its `% Audio:` file. It
differs from `grid.fit`, which writes a transcribed score's `\\tempo`, in
three ways:

- The tempo keeps its hundredths. `% BPM:` takes them, and 0.1 BPM is 0.18 s
  over 3:30, so there is no snap to a whole number (`grid.whole_tempo`).
- The metrical level is the score's. Beat This has no tempo prior, and counts
  a ballad in eighths or a punk song in half notes; of the heard tempo's ×½,
  ×1 and ×2 (and ×⅔ and ×3⁄2 in a compound meter) the one nearest the score's
  own tempo is taken.
- The meter is the score's. Its beats per bar group the beats into bars, and
  the downbeats only choose which beat is the first: their own vote for the
  meter goes wrong where Beat This marks a downbeat every two beats, as in a
  breakdown.
"""

from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction

import numpy as np

from . import grid

# Beats this close to the grid are on it, as in grid.py.
TOLERANCE = grid.TOLERANCE
# The score's tempo when it is not given.
DEFAULT_BPM = 120.0
# Fewer beats than this tell no tempo.
FEWEST_BEATS = 16
# Below this share of steady beat-to-beat intervals the beats are noise, as
# in a vocals stem, not a song that drifts.
FEWEST_REGULAR = 0.8
# Nor are they a song when one tempo misses them by more beats than this.
LONGEST_DRIFT = 8
SIMPLE_LEVELS = (Fraction(1, 2), Fraction(1), Fraction(2))
COMPOUND_LEVELS = SIMPLE_LEVELS + (Fraction(2, 3), Fraction(3, 2))


@dataclass
class Tempo:
    """The tempo, in the score's quarter notes a minute, and bar 1 of a recording."""

    # Quarter notes a minute at the level taken.
    bpm: float
    # Quarter notes a minute as Beat This heard the beat, before the level was chosen.
    heard_bpm: float
    # The factor from the heard beat to the score's: 2 when Beat This counted half notes.
    level: Fraction
    steady: bool
    # The share of the beats on one straight grid.
    share: float
    # The 95th percentile of the beats' distance from the line, in seconds.
    residual: float
    beats_per_bar: int
    # The meter the downbeats vote for, in the score's beats.
    heard_beats_per_bar: int
    # Seconds from the recording's start to its first bar line at or after it.
    bar_one: float


def regular_share(beats: np.ndarray) -> float:
    """The share of beat-to-beat intervals within 15 % of their neighbours' median.

    A song, steady or drifting, changes its tempo slowly; the beats Beat This
    guesses in a voice alone jump about.
    """
    intervals = np.diff(beats)
    if len(intervals) < 3:
        return 0.0
    window = 9
    padded = np.pad(intervals, window // 2, mode="edge")
    local = np.array([np.median(padded[i : i + window]) for i in range(len(intervals))])
    return float(np.mean(np.abs(intervals - local) < 0.15 * local))


def choose_level(heard_beat_bpm: float, score_beat_bpm: float, compound: bool) -> Fraction:
    """The level nearest the score's tempo, on a log scale: ×½, ×1, ×2, and ×⅔, ×3⁄2 in a compound meter."""
    levels = COMPOUND_LEVELS if compound else SIMPLE_LEVELS
    return min(levels, key=lambda level: (abs(np.log(heard_beat_bpm * float(level) / score_beat_bpm)), abs(np.log(float(level)))))


def _straight_line(beats: np.ndarray) -> tuple[float, float, np.ndarray]:
    """The least-squares line through every beat, cleaned, as one tempo would play them: (period, phase, residuals)."""
    cleaned = grid.clean_beats(beats)
    number = np.arange(len(cleaned), dtype=float)
    period, phase = np.linalg.lstsq(np.vstack([number, np.ones(len(cleaned))]).T, cleaned, rcond=None)[0]
    return float(period), float(phase), cleaned - (phase + number * period)


def _bars(downbeats: np.ndarray, period: float, phase: float, level: Fraction, beats_per_bar: int | None) -> tuple[int, int, float]:
    """The meter taken, the downbeats' own vote, and the first bar line at or after the start, on the grid of `period` and `phase`."""
    chosen = period / float(level)
    # Bar lines fall on the steps the heard and the chosen beats share: a
    # chosen beat is `level.denominator` of them, a heard one `level.numerator`.
    step = period / level.numerator
    steps = np.round((downbeats - phase) / step)
    on = np.abs(downbeats - (phase + steps * step)) < TOLERANCE
    marked, steps = downbeats[on], steps[on].astype(int)
    # The downbeats' own vote for the meter, counted in chosen beats from the
    # chosen beat most of them fall on.
    per_beat = level.denominator
    if len(steps):
        anchor = int(np.bincount(steps % per_beat, minlength=per_beat).argmax())
        on_beats = steps[steps % per_beat == anchor]
        voted, _ = grid.meter_and_phase((on_beats - anchor) // per_beat)
    else:
        voted = 4
    meter = beats_per_bar if beats_per_bar and beats_per_bar > 0 else voted
    bar = meter * chosen

    def votes(start: float) -> int:
        offset = (marked - start + bar / 2) % bar - bar / 2
        return int(np.sum(np.abs(offset) < TOLERANCE))

    # Which beat is the first of a bar is all the downbeats choose.
    candidates = phase + step * np.arange(meter * per_beat)
    first = max(candidates, key=lambda start: (votes(start), -((start - phase) % bar)))
    return meter, voted, float(first - np.floor((first + grid.NO_LEAD) / bar) * bar)


def detect(
    beats: np.ndarray,
    downbeats: np.ndarray,
    score_bpm: float | None = None,
    beats_per_bar: int | None = None,
    beat: float = 1.0,
) -> Tempo:
    """The tempo and bar 1 of a recording from Beat This's beats and downbeats, in seconds.

    `score_bpm` is the score's tempo in quarter notes a minute, `beats_per_bar`
    its meter, and `beat` its beat in quarter notes: 1.5 for the dotted
    quarter of 6/8. The answer is in quarter notes a minute too.
    """
    beats = np.sort(np.asarray(beats, dtype=float))
    downbeats = np.asarray(downbeats, dtype=float)
    if len(beats) < FEWEST_BEATS:
        raise ValueError(f"Too few beats were found to tell the tempo: {len(beats)}. A recording with no beat, such as a voice alone, has none to find.")
    period, phase, share = grid.fit_steady(beats)
    steady = share >= grid.STEADY
    if steady:
        number = np.round((beats - phase) / period)
        deviation = beats - (phase + number * period)
        residual = float(np.percentile(np.abs(deviation[np.abs(deviation) < TOLERANCE]), 95))
        bar_period, bar_phase, bar_downbeats = period, phase, downbeats
    else:
        line_period, line_phase, deviation = _straight_line(beats)
        residual = float(np.percentile(np.abs(deviation), 95))
        if regular_share(beats) < FEWEST_REGULAR or residual > LONGEST_DRIFT * line_period:
            raise ValueError(
                f"The beats found keep no tempo: only {share:.0%} of them lie on one. A recording with no beat, such as a voice alone, has none to find."
            )
        # Bar 1 is where the song starts, not where one tempo for all of it
        # would put it: the line through its first beats places it.
        early = beats[: FEWEST_BEATS]
        bar_period, bar_phase, _ = _straight_line(early)
        bar_downbeats = downbeats[downbeats <= early[-1] + TOLERANCE]
        period, phase = line_period, line_phase
    heard = 60.0 / period
    score = DEFAULT_BPM if score_bpm is None or not score_bpm > 0 else score_bpm
    level = choose_level(heard, score / beat, compound=beat != 1.0 and beat % 1.5 == 0)
    chosen = period / float(level)
    meter, voted, bar_one = _bars(bar_downbeats, bar_period, bar_phase, level, beats_per_bar)
    if not steady:
        # Without one grid to trust, bar 1 is the beat heard there.
        nearest = beats[np.argmin(np.abs(beats - bar_one))]
        if abs(nearest - bar_one) < TOLERANCE and nearest >= -grid.NO_LEAD:
            bar_one = float(nearest)
    return Tempo(
        bpm=60.0 / chosen * beat,
        heard_bpm=heard * beat,
        level=level,
        steady=steady,
        share=share,
        residual=residual,
        beats_per_bar=meter,
        heard_beats_per_bar=voted,
        bar_one=bar_one,
    )
