use colored::*;
use dashmap::DashMap;
use glob::glob;
use lazy_static::lazy_static;
use memmap2::Mmap;
use rayon::prelude::*;
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::fs::File;
use std::process;
use std::str;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref TRANSLATION_VAR_REGEX: Regex = Regex::new(r"\{(\w+)}").unwrap();
}

// Extracts variables like `{productName}` format from a translation string
fn extract_variables(text: &str) -> HashSet<String> {
    TRANSLATION_VAR_REGEX
        .captures_iter(text)
        .map(|cap| cap[1].to_string())
        .collect()
}

fn get_translation_file<'a>(
    file_mapping: &'a Arc<DashMap<String, HashMap<String, String>>>,
    lang: &str,
    key: &str,
) -> String {
    file_mapping
        .get(lang)
        .and_then(|fm| fm.get(key).map(|s| s.clone()))
        .unwrap_or_else(|| "Unknown file".to_string())
}

// Recursively flattens a JSON structure into dot-separated keys
fn flatten_json(value: &Value, prefix: String, output: &DashMap<String, String>) {
    match value {
        Value::Object(map) => {
            // Convert the map into a Vec of (key, value) tuples and use par_iter() on the Vec
            map.iter()
                .collect::<Vec<_>>() // Convert the map into a Vec of tuples
                .par_iter() // Parallel iteration
                .for_each(|(key, val)| {
                    let new_key = if prefix.is_empty() {
                        key.to_string()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    flatten_json(val, new_key, output); // Recursive call
                });
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
    translations: Arc<DashMap<String, HashMap<String, String>>>,
    file_mapping: Arc<DashMap<String, HashMap<String, String>>>,
) -> bool {
    let base_translation = translations.get(base_lang).unwrap();
    let base_keys: HashSet<_> = base_translation.keys().collect();

    let has_errors = Arc::new(Mutex::new(false));

    let mut impacted_folders = HashSet::new();
    let mut impacted_files = HashSet::new();

    // Dereference translations to access the DashMap and use `par_iter` on it
    translations.as_ref().iter().for_each(|entry| {
        let (lang, keys) = entry.pair();
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
                let file = get_translation_file(&file_mapping, lang, key);
                println!("   - Key: {} | File: {}", key.red(), file.blue());
            }
            local_errors = true;
        }

        if !extra_keys.is_empty() {
            println!("{}", "⚠️ Extra keys:".bold().yellow());
            for key in &extra_keys {
                let file = get_translation_file(&file_mapping, lang, key);
                println!("   - Key: {} | File: {}", key.yellow(), file.blue());
            }
            local_errors = true;
        }

        for key in base_keys.intersection(&other_keys) {
            let base_vars =
                extract_variables(translations.get(base_lang).unwrap().get(*key).unwrap());

            let other_vars = extract_variables(translations.get(lang).unwrap().get(*key).unwrap());

            if base_vars != other_vars {
                let base_file = get_translation_file(&file_mapping, base_lang, key);
                let other_file = get_translation_file(&file_mapping, lang, key);

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

                println!(
                    "   - Location: Expected in {} but found in {}",
                    base_file.yellow(),
                    other_file.blue()
                );

                impacted_files.insert(base_file.clone());
                impacted_files.insert(other_file.clone());
                local_errors = true;
            }
        }

        if local_errors {
            *has_errors.lock().unwrap() = true;
            impacted_folders.insert(lang.to_string());
        }
    });

    // Return whether errors were found
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

    // Print out found language folders and file statistics
    let total_folders = lang_folders.len();
    let total_files: usize = lang_folders
        .par_iter()
        .map(|lang| {
            let pattern = format!("{}/{}/*.json", base_path, lang);
            glob(&pattern)
                .expect("Failed to read glob pattern")
                .filter(|entry| entry.is_ok())
                .count()
        })
        .sum();

    println!(
        "{} {} language folders found.",
        "ℹ️ Info:".bold().cyan(),
        total_folders
    );

    println!(
        "{} {} translation files found across all folders.",
        "ℹ️ Info:".bold().cyan(),
        total_files
    );

    let translations = Arc::new(DashMap::new());
    let file_mapping = Arc::new(DashMap::new());

    lang_folders.par_iter().for_each(|lang| {
        let pattern = format!("{}/{}/*.json", base_path, lang);
        let mut translations_keys_and_values = HashMap::new();
        let mut translations_keys_and_paths = HashMap::new();

        for entry in glob(&pattern).expect("Failed to read glob pattern") {
            if let Ok(path) = entry {
                let file = File::open(&path).expect("Failed to open file");
                let mmap = unsafe { Mmap::map(&file).expect("Failed to map file") };
                let content = str::from_utf8(&mmap).expect("Invalid UTF-8");
                let json: Value = serde_json::from_str(content).expect("Invalid JSON");

                let mut flattened = DashMap::new();
                flatten_json(&json, "".to_string(), &mut flattened);

                for (key, value) in flattened {
                    translations_keys_and_values.insert(key.clone(), value);
                    translations_keys_and_paths.insert(key, path.to_string_lossy().to_string());
                }
            }
        }

        translations.insert(lang.to_string(), translations_keys_and_values);
        file_mapping.insert(lang.to_string(), translations_keys_and_paths);
    });

    let translations = Arc::clone(&translations);
    let file_mapping = Arc::clone(&file_mapping);

    let has_errors = check_translations("fr", Arc::clone(&translations), Arc::clone(&file_mapping));

    // End of process summary
    let impacted_folders = translations
        .as_ref()
        .iter()
        .filter_map(|entry| {
            let (lang, _keys) = entry.pair();
            if lang != "fr" {
                Some(lang.to_string())
            } else {
                None
            }
        })
        .collect::<HashSet<String>>();

    println!(
        "\n{}",
        "🌍 Translation Consistency Check Complete"
            .bold()
            .underline()
    );

    if has_errors {
        println!(
            "{} {} language(s) impacted with inconsistent keys/variables.",
            "❌ Error:".bold().red(),
            impacted_folders.len()
        );

        println!(
            "{} {} file(s) impacted by variable mismatches.",
            "❌ Error:".bold().red(),
            impacted_folders.len()
        );
        process::exit(1);
    } else {
        println!(
            "{} No translation issues found.",
            "✅ Success:".bold().green()
        );
        process::exit(0);
    }
}
