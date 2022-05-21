use lazy_static::lazy_static;
use regex::Regex;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

fn normalize_text_into_phrases(text: String) -> Vec<Phrase> {
    split_text_at_periods(&text)
        .map(|subtext| {
            let subtext = normalize_punctuation_to_whitespace(subtext);
            let subtext = normalize_extra_whitespaces(&subtext);
            let subtext = subtext.to_lowercase();

            Phrase(subtext)
        })
        .collect()
}

fn split_text_at_periods(text: &str) -> impl Iterator<Item = &str> {
    text.split(&['.', ';']).filter(|s| !s.is_empty())
}

fn normalize_punctuation_to_whitespace(text: &str) -> Cow<str> {
    lazy_static! {
        static ref PUNCTUATION_PATTERN: Regex = Regex::new(r"[[:punct:]]").unwrap();
    }

    PUNCTUATION_PATTERN.replace_all(text, " ")
}

fn normalize_extra_whitespaces(text: &str) -> Cow<str> {
    lazy_static! {
        static ref EXTRA_WHITESPACE_PATTERN: Regex = Regex::new(r"\s\s+").unwrap();
    }

    EXTRA_WHITESPACE_PATTERN.replace_all(text.trim(), " ")
}

#[derive(PartialEq, Debug)]
struct Phrase(String);

impl From<Phrase> for String {
    fn from(phrase: Phrase) -> Self {
        phrase.0
    }
}

struct IndexedPhrases {
    interned_texts: HashMap<String, usize>,
    indexed_texts: Vec<String>,
    phrase_indices_by_word: HashMap<usize, HashSet<usize>>,
}

#[derive(PartialEq, Eq, Hash, Debug)]
struct Word<'s>(&'s str);

impl IndexedPhrases {
    fn new() -> IndexedPhrases {
        IndexedPhrases {
            interned_texts: HashMap::new(),
            indexed_texts: Vec::new(),
            phrase_indices_by_word: HashMap::new(),
        }
    }

    fn get_common_words(&self) -> impl Iterator<Item = Word> {
        self.phrase_indices_by_word
            .keys()
            .map(|&key_index| Word(&self.indexed_texts[key_index]))
    }

    fn insert_phrase(&mut self, phrase: Phrase) {
        let phrase_content = String::from(phrase);

        if !phrase_content.contains(' ') {
            return;
        }

        let interned_phrase_index = self.intern_text(phrase_content.clone());

        for word in split_phrase_into_words(&phrase_content) {
            let interned_word_index = self.intern_text(word.into());
            self.link_phrase_to_word(interned_phrase_index, interned_word_index);
        }
    }

    fn intern_text(&mut self, text: String) -> usize {
        *self.interned_texts.entry(text.clone()).or_insert_with(|| {
            let new_index = self.indexed_texts.len();
            self.indexed_texts.push(text);
            new_index
        })
    }

    fn link_phrase_to_word(&mut self, phrase_index: usize, word_index: usize) {
        let phrase_indices = self
            .phrase_indices_by_word
            .entry(word_index)
            .or_insert_with(HashSet::new);

        phrase_indices.insert(phrase_index);
    }
}

fn split_phrase_into_words(phrase: &str) -> impl Iterator<Item = &str> {
    phrase.split_ascii_whitespace()
}

#[cfg(test)]
mod normalization_tests {
    use super::{normalize_text_into_phrases, Phrase};

    #[test]
    fn should_do_nothing_if_text_is_considered_to_be_normalized() {
        let phrases = normalize_text_into_phrases("hello world".into());

        assert_eq!(phrases, &[Phrase("hello world".into())]);
    }

    #[test]
    fn should_convert_to_lowercase() {
        let phrases = normalize_text_into_phrases("HELLO WoRlD".into());

        assert_eq!(phrases, &[Phrase("hello world".into())]);
    }

    #[test]
    fn should_remove_extra_spaces() {
        let phrases = normalize_text_into_phrases("   hello    world    ".into());

        assert_eq!(phrases, &[Phrase("hello world".into())]);
    }

    #[test]
    fn should_replace_punctuation_except_period_with_whitespace() {
        let punctuations_except_period = ('\x00'..='\x7f')
            .filter(|&c| c.is_ascii_punctuation())
            .filter(|&c| c != '.' && c != ';')
            .collect::<String>();

        let phrases = normalize_text_into_phrases(format!("foo{}bar", punctuations_except_period));

        assert_eq!(phrases, &[Phrase("foo bar".into())]);
    }

    #[test]
    fn should_split_text_at_period_punctuations() {
        let phrases =
            normalize_text_into_phrases("i think; therefore i am... it is hard to believe.".into());

        assert_eq!(
            phrases,
            &[
                Phrase("i think".into()),
                Phrase("therefore i am".into()),
                Phrase("it is hard to believe".into())
            ]
        );
    }
}

#[cfg(test)]
mod common_words_tests {
    use super::{IndexedPhrases, Phrase, Word};
    use std::collections::HashSet;

    #[test]
    fn should_return_empty_vec_if_no_phrase_was_indexed() {
        let indexed_phrases = IndexedPhrases::new();
        let common_words: Vec<_> = indexed_phrases.get_common_words().collect();

        assert_eq!(common_words, &[]);
    }

    #[test]
    fn should_return_empty_vec_if_indexed_phrase_has_only_one_word() {
        let mut indexed_phrases = IndexedPhrases::new();

        indexed_phrases.insert_phrase(Phrase("hello".into()));
        indexed_phrases.insert_phrase(Phrase("you".into()));
        indexed_phrases.insert_phrase(Phrase("all".into()));

        let common_words: Vec<_> = indexed_phrases.get_common_words().collect();

        assert_eq!(common_words, &[]);
    }

    #[test]
    fn should_return_deduplicated_words_from_phrases_with_two_or_more_words() {
        let mut indexed_phrases = IndexedPhrases::new();

        indexed_phrases.insert_phrase(Phrase("hello hello you all".into()));
        indexed_phrases.insert_phrase(Phrase("nice".into()));
        indexed_phrases.insert_phrase(Phrase("how are you all doing".into()));

        let common_words: HashSet<_> = indexed_phrases.get_common_words().collect();

        assert_eq!(
            common_words,
            HashSet::from_iter(["hello", "you", "all", "how", "are", "doing"].map(Word))
        );
    }
}
