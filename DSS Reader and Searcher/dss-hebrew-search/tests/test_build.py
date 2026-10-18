"""Tests for the pure helper functions in build_data.py.

Run with:  python -m unittest discover tests
(These do not need Text-Fabric or the dataset.)
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import build_data as b  # noqa: E402


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


if __name__ == "__main__":
    unittest.main()
