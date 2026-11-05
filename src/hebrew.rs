//! Hebrew text helpers shared by the statistics, the quotation finder and the data builder.

use unicode_normalization::UnicodeNormalization;

/// Is this one of the 27 Hebrew consonant letters (alef to tav, final forms included)?
#[inline]
pub fn is_letter(c: char) -> bool {
    ('\u{05d0}'..='\u{05ea}').contains(&c)
}

/// A final letter (ך ם ן ף ץ) as the ordinary letter (כ מ נ פ צ); anything else unchanged.
#[inline]
pub fn plain_final(c: char) -> char {
    match c {
        '\u{05da}' => '\u{05db}',
        '\u{05dd}' => '\u{05de}',
        '\u{05df}' => '\u{05e0}',
        '\u{05e3}' => '\u{05e4}',
        '\u{05e5}' => '\u{05e6}',
        other => other,
    }
}

/// Make every final letter an ordinary one.
pub fn normalize_finals(text: &str) -> String {
    text.chars().map(plain_final).collect()
}

/// Keep only the Hebrew consonant letters (drops points, marks, spaces).
///
/// Presentation forms such as U+FB2A (shin with shin dot) are decomposed first, so they turn
/// back into the plain letter plus a point that is then dropped.
pub fn hebrew_letters(text: &str) -> String {
    text.nfd().filter(|&c| is_letter(c)).collect()
}

/// Consonants only, final letters made ordinary: the spelling the Search tab matches on.
pub fn normalize_hebrew(text: &str) -> String {
    hebrew_letters(text).chars().map(plain_final).collect()
}

/// A spelling-neutral form of a word, so that full and defective spellings match.
///
/// Keeps only the Hebrew consonants, makes final letters ordinary, and drops ו and י, which
/// the scrolls often add or leave out as vowel letters.
pub fn skeleton(word: &str) -> String {
    word.nfd().filter(|&c| is_letter(c)).map(plain_final).filter(|&c| c != '\u{05d5}' && c != '\u{05d9}').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_drop_points_and_marks() {
        assert_eq!(hebrew_letters("עֵ֣ין"), "עין");
        assert_eq!(hebrew_letters("abc …"), "");
    }

    #[test]
    fn presentation_forms_become_plain_letters() {
        // U+FB2A is shin with shin dot
        assert_eq!(hebrew_letters("\u{fb2a}ל"), "של");
    }

    #[test]
    fn finals_are_made_ordinary() {
        assert_eq!(normalize_hebrew("עין"), "עינ");
        assert_eq!(normalize_hebrew("מלך"), "מלכ");
        assert_eq!(normalize_hebrew("עינ"), normalize_hebrew("עין"));
    }

    #[test]
    fn skeleton_ignores_vowel_letters() {
        assert_eq!(skeleton("ישראל"), skeleton("שראל"));
        assert_eq!(skeleton("דויד"), skeleton("דוד"));
    }
}
