use colored::*;
use dashmap::{DashMap, DashSet};
use glob::glob;
use lazy_static::lazy_static;
use rayon::prelude::*;
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::str;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

fn flatten_json(value: &Value, prefix: String, output: &DashMap<String, String>) {
    let mut stack = vec![(prefix, value)];

    while let Some((curr_prefix, curr_value)) = stack.pop() {
        match curr_value {
            Value::Object(map) => {
                for (key, val) in map {
                    let new_key = if curr_prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", curr_prefix, key)
                    };
                    stack.push((new_key, val));
                }
            }
            Value::String(text) => {
                output.insert(curr_prefix, text.clone());
            }
            _ => {}
        }
    }
}

fn extract_keys_from_content(content: &str, base_keys: &HashSet<String>) -> HashSet<String> {
    let used_keys: HashSet<String> = base_keys
        .par_iter()
        .filter(|key| content.contains(*key))
        .cloned()
        .collect();

    used_keys
}

fn get_all_files_by_extension(path: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = path.read_dir() {
        files.extend(entries.filter_map(Result::ok).flat_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                get_all_files_by_extension(&path, extension)
            } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
                vec![path]
            } else {
                vec![]
            }
        }));
    }
    files
}

fn process_files(files: &[PathBuf], base_keys: &HashSet<String>) -> HashSet<String> {
    let used_keys: HashSet<String> = files
        .par_iter()
        .filter_map(|file_path| {
            if let Ok(content) = fs::read_to_string(file_path) {
                Some(extract_keys_from_content(&content, base_keys))
            } else {
                None
            }
        })
        .flatten()
        .collect();

    used_keys
}

fn check_translations_usage(base_keys: &HashSet<String>, files: &[PathBuf]) -> HashSet<String> {
    let used_keys = process_files(files, base_keys);

    let unused_keys: HashSet<_> = base_keys.difference(&used_keys).cloned().collect();

    unused_keys
}

fn check_translations(
    base_lang: &str,
    translations: Arc<DashMap<String, HashMap<String, String>>>,
    file_mapping: Arc<DashMap<String, HashMap<String, String>>>,
    unused_keys: &DashSet<String>, // We're now using unused_keys here
) -> bool {
    let base_translation = translations.get("fr").unwrap();
    let base_keys: HashSet<_> = base_translation.keys().collect();
    let has_errors = Arc::new(AtomicBool::new(false));

    let impacted_files = DashSet::new();

    translations.iter().par_bridge().for_each(|entry| {
        let (lang, keys) = entry.pair();
        if lang == base_lang {
            return;
        }

        let other_keys: HashSet<_> = keys.keys().collect();
        let missing_keys: Vec<_> = base_keys.difference(&other_keys).collect();
        let extra_keys: Vec<_> = other_keys.difference(&base_keys).collect();

        let mut local_errors = false;

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

                impacted_files.insert(base_file);
                impacted_files.insert(other_file);
                local_errors = true;
            }
        }

        for key in unused_keys.iter() {
            let local_key = key.as_str();

            if other_keys.contains(&local_key.to_string()) {
                println!("{}", "⚠️ Unused key found in translation:".bold().yellow());
                let file: String = get_translation_file(&file_mapping, lang, local_key);
                println!("   - Key: {} | File: {}", key.yellow(), file.blue());
                local_errors = true;
            }
        }

        // Update the error status if any error is found
        if local_errors {
            has_errors.store(true, Ordering::Relaxed);
        }
    });

    has_errors.load(Ordering::Relaxed)
}

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

    let translations = Arc::new(DashMap::new());
    let file_mapping = Arc::new(DashMap::new());

    lang_folders.par_iter().for_each(|lang| {
        let pattern = format!("{}/{}/*.json", base_path, lang);
        let mut translations_keys_and_values = HashMap::new();
        let mut translations_keys_and_paths = HashMap::new();

        for entry in glob(&pattern).expect("Failed to read glob pattern") {
            if let Ok(path) = entry {
                let content = fs::read_to_string(&path).expect("Failed to read file");
                let json: Value = serde_json::from_str(&content).expect("Invalid JSON");

                let mut flattened = DashMap::new();
                flatten_json(&json, String::new(), &mut flattened);

                for (key, value) in flattened {
                    translations_keys_and_values.insert(key.clone(), value);
                    translations_keys_and_paths.insert(key, path.to_string_lossy().to_string());
                }
            }
        }

        translations.insert(lang.to_string(), translations_keys_and_values);
        file_mapping.insert(lang.to_string(), translations_keys_and_paths);
    });

    let has_errors = check_translations(
        "fr",
        translations.clone(),
        file_mapping.clone(),
        &DashSet::new(),
    );

    let files: Vec<PathBuf> = ["ts", "js", "vue"]
        .par_iter()
        .flat_map(|ext| get_all_files_by_extension(Path::new("../../circularx/webapp/src"), ext))
        .collect();

    let base_translation = translations.get("fr").unwrap();
    let base_keys: HashSet<String> = base_translation.keys().cloned().collect();

    let unused_keys = check_translations_usage(&base_keys, &files);

    println!("Unused keys: {:?}", unused_keys.len());

    process::exit(if has_errors { 1 } else { 0 });
}
