mod phrase_indexing;

use crate::phrase_indexing::{IndexedPhrases, WordIndex};
use rand::{self, Rng, SeedableRng};
use std::collections::HashSet;
use std::io;
use std::path::Path;
use tbot::{prelude::*, Bot};
use tokio::sync::Mutex;

struct BotState {
    indexed_phrases: IndexedPhrases,
    reply_prob: f32,
    rng: rand::rngs::StdRng,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let database_path = Path::new("bot_memory.txt");

    let state = BotState {
        indexed_phrases: init_indexed_phrases(database_path)?,
        reply_prob: 1.0,
        rng: rand::rngs::StdRng::from_entropy(),
    };

    let mut bot = Bot::from_env("BOT_TOKEN").stateful_event_loop(Mutex::new(state));

    bot.text(move |context, state| async move {
        let state = &mut *state.lock().await;

        let mut word_indices_from_phrases = HashSet::new();

        let msg_text = &context.text.value;
        for phrase in phrase_indexing::normalize_text_into_phrases(msg_text.into()) {
            let insertion_res = state.indexed_phrases.insert_phrase(phrase.clone());

            word_indices_from_phrases.extend(insertion_res.word_indices_from_phrase);

            if !insertion_res.has_inserted_phrase {
                continue;
            }

            if let Err(err) = store_line_in_database(database_path, phrase.as_ref()) {
                log::error!(
                    "couldn't store line in database: `{}`, due to error: {}",
                    phrase.as_ref(),
                    err
                )
            }
        }

        if state.rng.gen::<f32>() >= state.reply_prob {
            return;
        }

        let generated_response = generate_phrase(
            &state.indexed_phrases,
            word_indices_from_phrases.into_iter().collect(),
            &mut state.rng,
        );

        let call_result = context.send_message(&generated_response).call().await;

        if let Err(err) = call_result {
            log::error!(
                "couldn't send message `{}`, due to error: {}",
                generated_response,
                err
            );
        } else {
            log::info!("generated response: `{}`", generated_response);
        }
    });

    bot.command("setprob", |context, state| async move {
        let msg_text = &context.text.value;

        if let Ok(new_prob) = msg_text.parse::<f32>() {
            state.lock().await.reply_prob = new_prob;
        }
    });

    log::info!("starting to poll");

    bot.polling().start().await.unwrap();

    Ok(())
}

fn init_indexed_phrases(database_path: &Path) -> std::io::Result<IndexedPhrases> {
    use std::fs::File;
    use std::io::{prelude::*, BufReader};

    let file = File::open(database_path)?;
    let lines = BufReader::new(file).lines();

    let mut indexed_phrases = IndexedPhrases::new();
    let mut corrected_lines = Vec::new();

    for line in lines {
        let line = line?;
        for phrase in phrase_indexing::normalize_text_into_phrases(line.clone()) {
            if indexed_phrases.insert_phrase(phrase).has_inserted_phrase {
                corrected_lines.push(line.clone());
            }
        }
    }

    let mut file = File::create(database_path.with_extension("new"))?;
    for line in corrected_lines {
        writeln!(file, "{}", line)?;
    }

    Ok(indexed_phrases)
}

fn store_line_in_database(database_path: &Path, line: &str) -> io::Result<()> {
    use std::fs::File;
    use std::io::prelude::*;

    let mut file = File::options()
        .write(true)
        .append(true)
        .open(database_path)?;

    writeln!(file, "{}", line)?;
    file.flush()?;

    Ok(())
}

fn generate_phrase(
    indexed_phrases: &IndexedPhrases,
    word_indices_from_phrases: Vec<WordIndex>,
    rng: &mut impl Rng,
) -> String {
    use rand::seq::SliceRandom;

    let words = {
        if word_indices_from_phrases.is_empty() {
            indexed_phrases.get_common_words().collect::<Vec<_>>()
        } else {
            indexed_phrases.get_words_for_indices(&word_indices_from_phrases)
        }
    };
    let word_index = rng.gen_range(0..words.len());
    let picked_word = words[word_index];

    let phrases = indexed_phrases
        .get_phrases_with_word_in_common(picked_word)
        .collect::<Vec<_>>();

    let first_phrase = phrases.choose(rng).unwrap();
    let second_phrase = phrases.choose(rng).unwrap();

    phrase_indexing::concatenate_indexed_phrases(*first_phrase, *second_phrase)
}
