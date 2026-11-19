"""A score's tempo and bar 1 from its recording's beats (D159)."""

import numpy as np
import pytest

from lily_transcribe import grid, tempo


def click(bpm: float, start: float, count: int, jitter: float = 0.0, seed: int = 0) -> np.ndarray:
    """A click track's beats as Beat This hears them: at `bpm`, from `start`, within ±`jitter` s."""
    rng = np.random.default_rng(seed)
    return start + np.arange(count) * 60.0 / bpm + rng.uniform(-jitter, jitter, count)


def test_a_fractional_tempo_is_found_to_the_hundredth_with_its_bar_one():
    # Song A: 113.76, bar 1 at 0.325 s, 3:37 of beats within Beat This's 20 ms frame.
    beats = click(113.76, 0.325, 410, jitter=0.01)
    found = tempo.detect(beats, beats[::4], score_bpm=114, beats_per_bar=4)
    assert found.steady
    assert found.bpm == pytest.approx(113.76, abs=0.01)
    assert found.bar_one == pytest.approx(0.325, abs=0.01)
    assert found.share > 0.99
    assert found.residual < 0.012
    assert found.level == 1


def test_no_whole_tempo_snap():
    # grid.fit writes 123 for a \tempo, which takes whole numbers; % BPM: keeps the hundredths.
    beats = click(123.04, 0.1, 200, jitter=0.005)
    assert grid.fit(beats, beats[::4]).bpm == 123.0
    assert tempo.detect(beats, beats[::4], score_bpm=123).bpm == pytest.approx(123.04, abs=0.01)
    beats = click(113.76, 0.1, 200, jitter=0.005)
    assert tempo.detect(beats, beats[::4], score_bpm=114).bpm == pytest.approx(113.76, abs=0.01)


def test_an_offset_after_a_lead_in_of_less_than_a_bar():
    beats = click(88.2, 0.127, 300, jitter=0.008)
    # The first beats Beat This hears are a bar's last two, before bar 1.
    found = tempo.detect(beats, beats[2::4], score_bpm=88)
    assert found.bpm == pytest.approx(88.2, abs=0.01)
    assert found.bar_one == pytest.approx(0.127 + 2 * 60 / 88.2, abs=0.01)


@pytest.mark.parametrize(("heard", "level"), [(56.88, 2), (227.52, 0.5)])
def test_the_score_chooses_the_metrical_level(heard, level):
    # Beat This counted the song in half notes, or in eighths; the score says quarters.
    beats = click(heard, 0.325, int(heard * 3.5), jitter=0.005)
    downbeats = beats[:: max(1, round(4 / level))] if level < 1 else beats[::2]
    found = tempo.detect(beats, downbeats, score_bpm=114, beats_per_bar=4)
    assert found.level == level
    assert found.bpm == pytest.approx(113.76, abs=0.01)
    assert found.heard_bpm == pytest.approx(heard, abs=0.01)
    assert found.bar_one == pytest.approx(0.325, abs=0.01)


def test_without_a_score_tempo_the_level_nearest_120_is_taken():
    beats = click(227.52, 0.2, 700, jitter=0.005)
    assert tempo.detect(beats, beats[::8]).bpm == pytest.approx(113.76, abs=0.01)
    assert tempo.detect(beats, beats[::8], score_bpm=220).bpm == pytest.approx(227.52, abs=0.01)


def test_a_compound_meter_is_written_in_quarters():
    # 6/8 at a dotted quarter of 60: ♩ = 90. Beat This hears the dotted quarters.
    beats = click(60.0, 0.5, 200, jitter=0.005)
    found = tempo.detect(beats, beats[::2], score_bpm=90, beats_per_bar=2, beat=1.5)
    assert found.level == 1
    assert found.bpm == pytest.approx(90.0, abs=0.01)
    assert found.beats_per_bar == 2
    assert found.bar_one == pytest.approx(0.5, abs=0.01)


def test_a_compound_meter_heard_in_quarters_is_taken_at_two_thirds():
    # The same 6/8 heard at 90 beats a minute, three to the bar of two dotted quarters.
    beats = click(90.0, 0.5, 300, jitter=0.005)
    found = tempo.detect(beats, beats[::3], score_bpm=90, beats_per_bar=2, beat=1.5)
    assert found.level == pytest.approx(2 / 3)
    assert found.bpm == pytest.approx(90.0, abs=0.01)
    assert found.bar_one == pytest.approx(0.5, abs=0.01)


def test_the_scores_meter_overrides_a_bad_downbeat_vote():
    beats = click(113.76, 0.325, 400, jitter=0.005)
    # A downbeat every bar, and a breakdown marked every two beats for long enough to win the vote.
    downbeats = np.sort(np.concatenate([beats[:200:4], beats[200::2]]))
    alone = tempo.detect(beats, downbeats)
    assert alone.heard_beats_per_bar == 2
    found = tempo.detect(beats, downbeats, score_bpm=114, beats_per_bar=4)
    assert found.beats_per_bar == 4
    assert found.heard_beats_per_bar == 2
    assert found.bar_one == pytest.approx(0.325, abs=0.01)


def test_a_three_four_bar_one_from_the_downbeats():
    beats = click(100.0, 0.0, 120)
    found = tempo.detect(beats, beats[1::3], score_bpm=100, beats_per_bar=3)
    assert found.bar_one == pytest.approx(0.6)


def test_a_drifting_take_is_not_steady_and_says_by_how_much():
    # Slowing from 120 to 100: one tempo misses by most of a second.
    intervals = np.linspace(0.5, 0.6, 300)
    beats = 0.3 + np.concatenate([[0.0], np.cumsum(intervals)])
    found = tempo.detect(beats, beats[::4], score_bpm=110)
    assert not found.steady
    assert 100 < found.bpm < 120
    assert found.residual > 0.5
    # Bar 1 is where the song starts, not where the one tempo would put it.
    assert found.bar_one == pytest.approx(0.3, abs=0.02)


def test_too_few_beats_is_an_error():
    with pytest.raises(ValueError, match="Too few beats"):
        tempo.detect(click(120, 0, 10), click(120, 0, 10)[::4])


def test_beats_without_a_tempo_are_an_error():
    # A voice alone: Beat This's guesses jump about.
    rng = np.random.default_rng(3)
    beats = np.cumsum(rng.uniform(0.2, 1.4, 200))
    with pytest.raises(ValueError, match="keep no tempo"):
        tempo.detect(beats, beats[::4])
