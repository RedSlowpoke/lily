"""The words heard, split into syllables and laid on the notes (D134)."""

from lily_transcribe.lyrics import Syllable, Word, align, syllables


def test_a_word_is_split_by_the_dictionary_and_its_time_shared_by_letters():
    found = syllables([Word("lantern,", 1.0, 1.8), Word("glow", 2.0, 2.4)], "en")
    assert [(s.text, round(s.start, 2), s.hyphen) for s in found] == [
        ("lan", 1.0, True),
        ("tern,", 1.3, False),
        ("glow", 2.0, False),
    ]


def test_a_line_break_inside_a_word_makes_two_words_in_its_time():
    found = syllables([Word("oh\nlantern,", 1.0, 2.0), Word("(la\nla)\n", 2.0, 2.4)], "en")
    assert [(s.text, round(s.start, 2), s.hyphen) for s in found] == [
        ("oh", 1.0, False),
        ("lan", 1.2, True),
        ("tern,", 1.5, False),
        ("(la", 2.0, False),
        ("la)", 2.2, False),
    ]


def test_a_syllable_goes_on_the_note_starting_nearest_it():
    sung = [Syllable("lan", 1.0, True), Syllable("tern", 1.5, False), Syllable("glow", 2.0, False)]
    assert align(sung, [1.02, 1.49, 2.05]) == [(0, "lan", True), (1, "tern", False), (2, "glow", False)]


def test_a_note_far_from_the_words_gets_none():
    sung = [Syllable("it", 1.0, False), Syllable("was", 1.5, False)]
    # A note in the break before, heard where nobody sings.
    assert align(sung, [0.0, 1.0, 1.5]) == [(1, "it", False), (2, "was", False)]


def test_a_syllable_without_a_note_is_sung_on_the_one_before():
    sung = [Syllable("and", 1.0, False), Syllable("the", 1.1, False), Syllable("lan", 1.5, True), Syllable("tern", 1.6, False)]
    # Of a word, joined; of two words, with a space; the last one's hyphen.
    assert align(sung, [1.0, 1.5]) == [(0, "and the", False), (1, "lantern", False)]


def test_a_syllable_far_from_every_note_is_kept_on_the_nearest():
    assert align([Syllable("hey", 5.0, False)], [0.0, 1.0]) == [(1, "hey", False)]
    assert align([Syllable("hey", -5.0, False)], [0.0, 1.0]) == [(0, "hey", False)]


def test_with_no_match_at_all_each_syllable_keeps_to_its_nearest_note():
    sung = [Syllable("lan", 1.0, True), Syllable("tern", 2.0, False), Syllable("oh", 20.0, False), Syllable("yeah", 21.0, False)]
    assert align(sung, [10.0, 11.0]) == [(0, "lantern", False), (1, "oh yeah", False)]
    # The first is too far to match and has no note before it, so the second, on the note, gets no match either.
    assert align([Syllable("a", -10.0, False), Syllable("b", 0.0, False)], [0.0]) == [(0, "a b", False)]


def test_no_words_or_no_notes_lay_nothing():
    assert align([], [1.0]) == []
    assert align([Syllable("la", 1.0, False)], []) == []
