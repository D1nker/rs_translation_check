# Translation Consistency Checker

This Rust script checks the consistency of translation files across multiple languages. It ensures that:
- All languages have the same translation keys.
- There are no missing or extra keys in any language.
- Variables within translations (e.g., `{name}`) are consistent across all languages.

## Features
- Detects missing and extra translation keys.
- Validates that translation variables match between languages.
- Displays results with colorized output.
- Uses a progress bar for real-time feedback.

## Installation
To use this script, you need to have Rust installed. If you haven't installed Rust yet, you can do so using [Rustup](https://rustup.rs/):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then, clone this repository and navigate to its directory:

```sh
git clone https://github.com/d1nker/translation-checker.git
cd translation-checker
```

## Usage
### Running the script
You can run the script using Cargo:

```sh
cargo run -- /path/to/your/translation/files
```

If no path is provided, it defaults to:
```sh
./webapp/src/assets/i18n
```

### Expected File Structure
Your translation files should be organized in separate folders for each language:

```
/path/to/your/translation/files/
├── en/
│   ├── common.json
│   ├── errors.json
│   └── messages.json
├── fr/
│   ├── common.json
│   ├── errors.json
│   └── messages.json
├── de/
│   ├── common.json
│   ├── errors.json
│   └── messages.json
└── nl/
    ├── common.json
    ├── errors.json
    └── messages.json
```

### Output Example
#### ✅ No Issues Found
```
🌍 Translation Consistency Check
✅ No translation issues found.
```

#### ❌ Issues Found
```
🌍 Translation Consistency Check
🔍 Checking DE
❌ Missing keys:
   - common.greeting
⚠️ Extra keys:
   - errors.timeout
🔄 Variable mismatch detected!
   - Key: messages.welcome
   - Expected variables (FR): {"name"}
   - Found variables (DE): {"username"}
   - Location: Expected in fr/common.json but found in de/messages.json
❌ Translation issues found. Exiting with error.
```

## Dependencies
This project uses the following Rust crates:
- `colored` for colorized terminal output.
- `glob` for reading JSON files in multiple directories.
- `indicatif` for the progress bar.
- `regex` for extracting variables.
- `serde_json` for parsing JSON.

## Contributing
Feel free to submit issues and pull requests to improve this tool!

## License
This project is licensed under the MIT License.

