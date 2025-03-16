use colored::*;
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::process;
use std::sync::{Arc, Mutex};

// Extracts variables like `{productName}` format from a translation string
fn extract_variables(text: &str) -> HashSet<String> {
    let translation_variable_regex = Regex::new(r"\{(\w+)}").unwrap();
    translation_variable_regex
        .captures_iter(text)
        .map(|cap| cap[1].to_string())
        .collect()
}

// Recursively flattens a JSON structure into dot-separated keys
fn flatten_json(value: &Value, prefix: String, output: &mut HashMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let new_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                flatten_json(val, new_key, output);
            }
        }
        Value::String(text) => {
            output.insert(prefix, text.clone());
        }
        _ => {}
    }
}

// Checks translation consistency across languages in parallel
fn check_translations(
    base_lang: &str,
    translations: Arc<HashMap<String, HashMap<String, String>>>,
    file_mapping: Arc<HashMap<String, HashMap<String, String>>>,
    progress_bar: Arc<ProgressBar>,
) -> bool {
    let base_keys: HashSet<_> = translations.get(base_lang).unwrap().keys().collect();
    let has_errors = Arc::new(Mutex::new(false));

    translations.par_iter().for_each(|(lang, keys)| {
        if lang == base_lang {
            return;
        }

        let other_keys: HashSet<_> = keys.keys().collect();
        let missing_keys: Vec<_> = base_keys.difference(&other_keys).collect();
        let extra_keys: Vec<_> = other_keys.difference(&base_keys).collect();

        let mut local_errors = false;

        println!("\n🔍 Checking {}", lang.to_uppercase().bold().blue());

        if !missing_keys.is_empty() {
            println!("{}", "❌ Missing keys:".bold().red());
            for key in &missing_keys {
                println!("   - {}", key.red());
            }
            local_errors = true;
        }
        if !extra_keys.is_empty() {
            println!("{}", "⚠️ Extra keys:".bold().yellow());
            for key in &extra_keys {
                println!("   - {}", key.yellow());
            }
            local_errors = true;
        }

        for key in base_keys.intersection(&other_keys) {
            let base_vars = extract_variables(translations[base_lang].get(*key).unwrap());
            let other_vars = extract_variables(translations[lang].get(*key).unwrap());

            if base_vars != other_vars {
                println!("{}", "🔄 Variable mismatch detected!".bold().magenta());
                println!("   - Key: {}", key.magenta());

                println!(
                    "   - Expected variables ({}): {}",
                    base_lang.to_uppercase().bold(),
                    format!("{:?}", base_vars).green()
                );
                println!(
                    "   - Found variables ({}): {}",
                    lang.to_uppercase().bold(),
                    format!("{:?}", other_vars).cyan()
                );

                let base_file = file_mapping
                    .get(base_lang)
                    .and_then(|fm| fm.get(*key))
                    .cloned()
                    .unwrap_or_else(|| "Unknown file".to_string());

                let other_file = file_mapping
                    .get(lang)
                    .and_then(|fm| fm.get(*key))
                    .cloned()
                    .unwrap_or_else(|| "Unknown file".to_string());

                println!(
                    "   - Location: Expected in {} but found in {}",
                    base_file.yellow(),
                    other_file.blue()
                );

                local_errors = true;
            }

            progress_bar.inc(1);
        }

        if missing_keys.is_empty() && extra_keys.is_empty() && !local_errors {
            println!("{}", "✅ All keys are consistent!".bold().green());
        }

        if local_errors {
            *has_errors.lock().unwrap() = true;
        }
    });

    *has_errors.lock().unwrap()
}

// Loads all translation files and checks consistency in parallel
fn main() {
    let args: Vec<String> = env::args().collect();
    let base_path = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("../../circularx/webapp/src/assets/i18n");

    let lang_folders: Vec<String> = fs::read_dir(base_path)
        .expect("Failed to read directory")
        .filter_map(|entry| {
            entry.ok().and_then(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    entry.file_name().into_string().ok()
                } else {
                    None
                }
            })
        })
        .collect();

    println!("{:?}", lang_folders);

    let translations = Arc::new(Mutex::new(HashMap::new()));
    let file_mapping = Arc::new(Mutex::new(HashMap::new()));

    // Progress bar setup
    let progress_bar = Arc::new(ProgressBar::new(100));
    progress_bar.set_style(
        ProgressStyle::with_template("{bar:40.green} {pos:>7}/{len:7} ({eta}) {msg}")
            .unwrap()
            .progress_chars("=>-")
            .tick_chars("=>-|"),
    );

    lang_folders.par_iter().for_each(|lang| {
        let pattern = format!("{}/{}/*.json", base_path, lang);
        let mut translations_keys_and_values = HashMap::new();
        let mut translations_keys_and_paths = HashMap::new();

        for entry in glob(&pattern).expect("Failed to read glob pattern") {
            if let Ok(path) = entry {
                let content = fs::read_to_string(&path).expect("Failed to read file");
                let json: Value = serde_json::from_str(&content).expect("Invalid JSON");

                let mut flattened = HashMap::new();
                flatten_json(&json, "".to_string(), &mut flattened);

                for (key, value) in flattened {
                    translations_keys_and_values.insert(key.clone(), value);
                    translations_keys_and_paths.insert(key, path.to_string_lossy().to_string());
                }
            }
        }

        translations
            .lock()
            .unwrap()
            .insert(lang.to_string(), translations_keys_and_values);
        file_mapping
            .lock()
            .unwrap()
            .insert(lang.to_string(), translations_keys_and_paths);
    });

    let total_entries: usize = translations
        .lock()
        .unwrap()
        .values()
        .map(|keys| keys.len())
        .sum();
    progress_bar.set_length(total_entries as u64);
    progress_bar.set_message(format!(
        "{} - Checking translations...",
        "🚀 Progress".bold().cyan()
    ));

    let translations = Arc::new(translations.lock().unwrap().clone());
    let file_mapping = Arc::new(file_mapping.lock().unwrap().clone());

    let has_errors = check_translations(
        "fr",
        Arc::clone(&translations),
        Arc::clone(&file_mapping),
        Arc::clone(&progress_bar),
    );

    progress_bar.finish_with_message("Translation check complete ✅.");

    println!("{}", "🌍 Translation Consistency Check".bold().underline());

    if has_errors {
        println!(
            "{}",
            "❌ Translation issues found. Exiting with error."
                .bold()
                .red()
        );
        process::exit(1);
    } else {
        println!("{}", "✅ No translation issues found.".bold().green());
        process::exit(0);
    }
}
