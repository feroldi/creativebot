use lazy_static::lazy_static;
use regex::Regex;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

pub(crate) fn normalize_text_into_phrases(text: String) -> Vec<Phrase> {
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

#[derive(PartialEq, Debug, Clone)]
pub(crate) struct Phrase(String);

impl From<Phrase> for String {
    fn from(phrase: Phrase) -> Self {
        phrase.0
    }
}

impl AsRef<str> for Phrase {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub(crate) struct IndexedPhrases {
    interned_texts: HashMap<String, usize>,
    indexed_texts: Vec<String>,
    indexed_phrases_by_word: HashMap<usize, HashSet<IndexedPhrase>>,
}

#[derive(PartialEq, Eq, Hash)]
struct IndexedPhrase {
    interned_phrase_index: usize,
    word_pos_in_phrase: usize,
}

#[derive(PartialEq, Eq, Hash, Debug, Copy, Clone)]
pub(crate) struct IndexedPhraseContent<'s> {
    phrase_content: &'s str,
    word_pos_in_phrase: usize,
}

#[derive(PartialEq, Eq, Hash, Debug, Copy, Clone)]
pub(crate) struct Word<'s>(&'s str);

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub(crate) struct WordIndex(usize);

impl IndexedPhrases {
    pub(crate) fn new() -> IndexedPhrases {
        IndexedPhrases {
            interned_texts: HashMap::new(),
            indexed_texts: Vec::new(),
            indexed_phrases_by_word: HashMap::new(),
        }
    }

    pub(crate) fn get_common_words(&self) -> impl Iterator<Item = Word> {
        self.indexed_phrases_by_word
            .keys()
            .map(|&key_index| Word(&self.indexed_texts[key_index]))
    }

    // TODO(feroldi): Test this.
    pub(crate) fn get_words_for_indices(&self, word_indices: &[WordIndex]) -> Vec<Word> {
        let mut words = Vec::new();

        for word_index in word_indices {
            words.push(Word(&self.indexed_texts[word_index.0]))
        }

        words
    }

    // TODO(feroldi): Maybe return the words that were already interned?
    // TODO(feroldi): Test the returned words.
    pub(crate) fn insert_phrase(&mut self, phrase: Phrase) -> InsertionResult {
        let phrase_content = String::from(phrase);

        if !phrase_content.contains(' ') {
            let interned_word_index = self.intern_text(phrase_content);
            return InsertionResult {
                has_inserted_phrase: false,
                word_indices_from_phrase: vec![WordIndex(interned_word_index)],
            };
        }

        let interned_phrase_index = self.intern_text(phrase_content.clone());
        let mut word_indices_from_phrase = Vec::new();

        let mut word_pos_in_phrase = 0;
        for word in phrase_content.split_ascii_whitespace() {
            let interned_word_index = self.intern_text(word.into());

            self.link_phrase_to_word(
                interned_phrase_index,
                interned_word_index,
                word_pos_in_phrase,
            );

            // Adds one to the word length in order to consider the whitespace character
            // after it.
            word_pos_in_phrase += word.len() + 1;

            word_indices_from_phrase.push(WordIndex(interned_word_index));
        }

        InsertionResult {
            has_inserted_phrase: true,
            word_indices_from_phrase,
        }
    }

    pub(crate) fn get_phrases_with_word_in_common(
        &self,
        word: Word,
    ) -> impl Iterator<Item = IndexedPhraseContent> {
        let word_index = self.interned_texts.get(word.0);

        // This is always true, because the only way we can get a `Word` value is by
        // calling `get_common_words()`, which returns indexed words from the very
        // `phrase_indices_by_word` collection.
        debug_assert!(word_index.is_some());

        let indexed_phrases_of_word = self.indexed_phrases_by_word.get(word_index.unwrap());

        // Always true for the same reason above.
        debug_assert!(indexed_phrases_of_word.is_some());

        indexed_phrases_of_word
            .unwrap()
            .iter()
            .map(|indexed_phrase| {
                let phrase_content = &self.indexed_texts[indexed_phrase.interned_phrase_index];
                IndexedPhraseContent {
                    phrase_content,
                    word_pos_in_phrase: indexed_phrase.word_pos_in_phrase,
                }
            })
    }

    fn intern_text(&mut self, text: String) -> usize {
        *self.interned_texts.entry(text.clone()).or_insert_with(|| {
            let new_index = self.indexed_texts.len();
            self.indexed_texts.push(text);
            new_index
        })
    }

    fn link_phrase_to_word(
        &mut self,
        phrase_index: usize,
        word_index: usize,
        word_pos_in_phrase: usize,
    ) {
        let phrase_indices = self
            .indexed_phrases_by_word
            .entry(word_index)
            .or_insert_with(HashSet::new);

        phrase_indices.insert(IndexedPhrase {
            interned_phrase_index: phrase_index,
            word_pos_in_phrase,
        });
    }
}

pub(crate) struct InsertionResult {
    pub(crate) has_inserted_phrase: bool,
    pub(crate) word_indices_from_phrase: Vec<WordIndex>,
}

pub(crate) fn concatenate_indexed_phrases<'s>(
    mut first_phrase: IndexedPhraseContent<'s>,
    mut second_phrase: IndexedPhraseContent<'s>,
) -> String {
    if first_phrase.word_pos_in_phrase == 0
        && !second_phrase.phrase_content[second_phrase.word_pos_in_phrase..].contains(' ')
    {
        std::mem::swap(&mut first_phrase, &mut second_phrase);
    }

    let first_phrase_half = &first_phrase.phrase_content[..first_phrase.word_pos_in_phrase];
    let second_phrase_half = &second_phrase.phrase_content[second_phrase.word_pos_in_phrase..];

    format!("{}{}", first_phrase_half, second_phrase_half)
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

#[cfg(test)]
mod retrieval_of_phrases_for_word_in_common_tests {
    use super::{IndexedPhraseContent, IndexedPhrases, Phrase, Word};
    use std::collections::HashSet;

    #[test]
    #[should_panic]
    fn should_panic_if_word_is_unknown() {
        let indexed_phrases = {
            let mut ip = IndexedPhrases::new();
            ip.insert_phrase(Phrase("hello there".into()));
            ip
        };

        let _: Vec<_> = indexed_phrases
            .get_phrases_with_word_in_common(Word("hi"))
            .collect();
    }

    #[test]
    fn should_return_indexed_phrases_that_have_the_passed_word_in_common() {
        let indexed_phrases = {
            let mut ip = IndexedPhrases::new();
            ip.insert_phrase(Phrase("hello there friend".into()));
            ip.insert_phrase(Phrase("hey friend what are you up to".into()));
            ip.insert_phrase(Phrase("i have got lots of friends".into()));
            ip.insert_phrase(Phrase("good evening".into()));
            ip
        };

        let phrases: HashSet<_> = indexed_phrases
            .get_phrases_with_word_in_common(Word("friend"))
            .collect();

        assert_eq!(
            phrases,
            HashSet::from_iter([
                IndexedPhraseContent {
                    phrase_content: "hello there friend",
                    word_pos_in_phrase: 12,
                },
                IndexedPhraseContent {
                    phrase_content: "hey friend what are you up to",
                    word_pos_in_phrase: 4,
                }
            ])
        );
    }

    #[test]
    fn should_not_duplicate_phrases() {
        let indexed_phrases = {
            let mut ip = IndexedPhrases::new();
            ip.insert_phrase(Phrase("hello there friend".into()));
            ip.insert_phrase(Phrase("hello there friend".into()));
            ip.insert_phrase(Phrase("hello there friend".into()));
            ip
        };

        let phrases: HashSet<_> = indexed_phrases
            .get_phrases_with_word_in_common(Word("friend"))
            .collect();

        assert_eq!(
            phrases,
            HashSet::from_iter([IndexedPhraseContent {
                phrase_content: "hello there friend",
                word_pos_in_phrase: 12,
            }])
        );
    }
}

#[cfg(test)]
mod phrase_concatenation_tests {
    use super::{concatenate_indexed_phrases, IndexedPhraseContent};

    #[test]
    fn should_split_phrases_and_concatenate_at_the_word_in_common() {
        let phrase_a = IndexedPhraseContent {
            phrase_content: "i have to go to the supermarket",
            word_pos_in_phrase: 10,
        };

        let phrase_b = IndexedPhraseContent {
            phrase_content: "does anyone need to go first",
            word_pos_in_phrase: 20,
        };

        assert_eq!(
            concatenate_indexed_phrases(phrase_a, phrase_b),
            "i have to go first"
        );

        assert_eq!(
            concatenate_indexed_phrases(phrase_b, phrase_a),
            "does anyone need to go to the supermarket"
        );
    }

    #[test]
    fn should_swap_phrases_if_the_first_starts_with_word_and_the_second_ends_with_word() {
        let phrase_a = IndexedPhraseContent {
            phrase_content: "go to the supermarket",
            word_pos_in_phrase: 0,
        };

        let phrase_b = IndexedPhraseContent {
            phrase_content: "does anyone need to go",
            word_pos_in_phrase: 20,
        };

        let phrase_result = concatenate_indexed_phrases(phrase_a, phrase_b);

        assert_eq!(phrase_result, "does anyone need to go to the supermarket");
    }
}
