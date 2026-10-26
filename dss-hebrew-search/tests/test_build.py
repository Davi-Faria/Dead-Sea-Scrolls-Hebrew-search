"""Tests for the pure helper functions in build_data.py.

Run with:  python -m unittest discover tests
(These do not need Text-Fabric or the dataset.)
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import build_data as b  # noqa: E402
import bible_parallels as bp  # noqa: E402


def token(text, lemma=None, sp="subs", lang=None, bits=0, after=" "):
    return {"text": text, "lemma": lemma, "sp": sp, "lang": lang, "bits": bits, "after": after}


class HebrewLetters(unittest.TestCase):
    def test_keeps_only_consonants(self):
        self.assertEqual(b.hebrew_letters("עַיִן"), "עין")
        self.assertEqual(b.hebrew_letters("אמת׳ו"), "אמתו")
        self.assertEqual(b.hebrew_letters("{ שמו }"), "שמו")

    def test_presentation_forms_are_decomposed(self):
        # U+FB2A is shin with shin dot; it must not be dropped.
        self.assertEqual(b.hebrew_letters("קָדֹו\ufb2a"), "קדוש")

    def test_none_and_empty(self):
        self.assertEqual(b.hebrew_letters(None), "")
        self.assertEqual(b.hebrew_letters(""), "")


class DisplayText(unittest.TestCase):
    def test_letters_win(self):
        self.assertEqual(b.display_text("עיני׳ו", "עיני?׳ו", None), "עיניו")

    def test_gap_marker_becomes_ellipsis(self):
        self.assertEqual(b.display_text(None, "\u03b5", None), "\u2026")

    def test_punctuation_is_kept(self):
        self.assertEqual(b.display_text(None, "׃", None), "׃")


class Grouping(unittest.TestCase):
    def test_prefixes_join_the_following_word(self):
        words = b.group_tokens([
            token("ל", "לְ", sp="ptcl", after=None),
            token("הביט", "נבט", sp="verb"),
            token("ב", "בְּ", sp="ptcl", after=None),
            token("אור", "אור", sp="subs"),
        ])
        self.assertEqual([w["text"] for w in words], ["להביט", "באור"])
        self.assertEqual([w["lemma"] for w in words], ["נבט", "אור"])

    def test_lemma_comes_from_the_stem_not_the_prefix(self):
        word = b.merge_tokens([token("ו", "וְ", sp="ptcl", after=None), token("ברוח", "רוח")])
        self.assertEqual(word["lemma"], "רוח")

    def test_word_with_only_a_particle_has_no_lemma(self):
        self.assertIsNone(b.merge_tokens([token("ו", "וְ", sp="ptcl")])["lemma"])

    def test_language_is_taken_from_the_stem(self):
        word = b.merge_tokens([token("ו", "וְ", sp="ptcl", lang="a", after=None), token("חזא", "חזה", lang="a")])
        self.assertEqual(word["lang"], "a")
        self.assertEqual(b.merge_tokens([token("חזה", "חזה")])["lang"], "h")

    def test_unfinished_word_at_end_of_line_is_kept(self):
        words = b.group_tokens([token("ל", "לְ", sp="ptcl", after=None)])
        self.assertEqual([w["text"] for w in words], ["ל"])

    def test_flags_combine(self):
        word = b.merge_tokens([
            token("ו", "וְ", sp="ptcl", bits=b.BIT_REC_FULL, after=None),
            token("אמת", "אמת", bits=b.BIT_REC_FULL | b.BIT_UNC),
        ])
        self.assertTrue(word["bits"] & b.BIT_REC_FULL)
        self.assertTrue(word["bits"] & b.BIT_UNC)

    def test_partly_reconstructed_stays_partial(self):
        word = b.merge_tokens([
            token("ו", "וְ", sp="ptcl", bits=0, after=None),
            token("אמת", "אמת", bits=b.BIT_REC_FULL),
        ])
        self.assertTrue(word["bits"] & b.BIT_REC_PART)
        self.assertFalse(word["bits"] & b.BIT_REC_FULL)

    def test_status_alphabet_has_room_for_all_flags(self):
        top = b.BIT_REC_PART | b.BIT_REC_FULL | b.BIT_UNC | b.BIT_COR | b.BIT_REM
        self.assertLess(top, len(b.B32))


class BibleReferences(unittest.TestCase):
    def test_placed_passages_get_numbers(self):
        self.assertEqual(b.bible_ref("Is", "61", "1"), ("Is", 61, 1))

    def test_unplaced_manuscripts_get_none(self):
        self.assertIsNone(b.bible_ref("1Q4", "f2", "3"))
        self.assertIsNone(b.bible_ref(None, None, None))

    def test_a_word_takes_the_reference_of_its_stem(self):
        word = b.merge_tokens([
            dict(token("ו", "וְ", sp="ptcl", after=None), ref=("Is", 61, 1)),
            dict(token("יען", "יען"), ref=("Is", 61, 1)),
        ])
        self.assertEqual(word["ref"], ("Is", 61, 1))

    def test_verse_segments_split_where_the_verse_changes(self):
        words = [{"ref": ("Is", 61, 1)}, {"ref": ("Is", 61, 1)}, {"ref": ("Is", 61, 2)},
                 {"ref": None}, {"ref": ("Is", 61, 2)}]
        self.assertEqual(b.verse_segments(7, words),
                         [[7, "Is", 61, 1, 0, 2], [7, "Is", 61, 2, 2, 3], [7, "Is", 61, 2, 4, 5]])


class QuotationDetection(unittest.TestCase):
    LINES = [
        [0, "1", "1", ["רוח", "אדני", "יהוה", "עלי", "יען", "משח"], [], "000000", 1, "Is 61:1"],
        [1, "2", "1", ["פשרו", "אשר", "רוח", "אדני", "יהוה", "עלי", "יען", "ויאמר"], [], "00000000", 0, ""],
    ]
    SEGMENTS = [[0, "Is", 61, 1, 0, 6]]

    def test_skeleton_ignores_vowel_letters_and_final_forms(self):
        self.assertEqual(bp.skeleton("אדוני"), bp.skeleton("אדני"))
        self.assertEqual(bp.skeleton("עין"), bp.skeleton("עינ"))

    def test_a_four_word_match_is_found(self):
        self.assertEqual(bp.find_quotes(self.LINES, self.SEGMENTS), [[1, 2, 7, "Is", 61, 1, 1, 1]])

    def test_no_match_in_biblical_lines_themselves(self):
        self.assertEqual([q for q in bp.find_quotes(self.LINES, self.SEGMENTS) if q[0] == 0], [])

    def test_short_matches_are_ignored(self):
        lines = [self.LINES[0], [1, "2", "1", ["פשרו", "רוח", "אדני", "יהוה"], [], "0000", 0, ""]]
        self.assertEqual(bp.find_quotes(lines, self.SEGMENTS), [])

    def test_quotation_can_run_over_two_lines(self):
        lines = [self.LINES[0],
                 [1, "2", "1", ["פשרו", "רוח", "אדני"], [], "000", 0, ""],
                 [1, "2", "2", ["יהוה", "עלי", "יען"], [], "000", 0, ""]]
        found = bp.find_quotes(lines, self.SEGMENTS)
        self.assertEqual([(q[0], q[1], q[2]) for q in found], [(1, 1, 3), (2, 0, 3)])
        self.assertEqual(len({q[7] for q in found}), 1)   # one span, two lines


if __name__ == "__main__":
    unittest.main()
