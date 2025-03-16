
# Translation Consistency Checker

This Rust script checks the consistency of translation files across multiple languages. It ensures that:

- All languages have the same translation keys.
- There are no missing or extra keys in any language.
- Variables within translations (e.g., `{productName}`) are consistent across all languages.

## Features

- Detects missing and extra translation keys.
- Validates that translation variables match between languages.
- Displays results with colorized output.
- Detailed information about missing/extra keys and variable mismatches.
- Lists the files where issues were found.
- Real-time summary of errors across languages and files.

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
🌍 Translation Consistency Check Complete
✅ No translation issues found.
```

#### ❌ Issues Found

```
🌍 Translation Consistency Check Complete
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

### Detailed Output

During the check, missing and extra keys are listed with the following details:

- **Key**: The missing or extra key.
- **File**: The file where the key was found (or expected).
- **Variable Mismatches**: If a key has variables (e.g., `{name}`), the script will compare them across languages and highlight any mismatches.

For example:

```
❌ Missing keys:
   - Key: {productName} | File: assets/i18n/en/product.json
⚠️ Extra keys:
   - Key: {oldProductName} | File: assets/i18n/en/product.json
🔄 Variable mismatch detected!
   - Key: {greeting}
   - Expected variables (FR): {"userName"}
   - Found variables (EN): {"username"}
   - Location: Expected in assets/i18n/fr/product.json but found in assets/i18n/en/product.json
```

## Dependencies

This project uses the following Rust crates:

- `colored` for colorized terminal output.
- `glob` for reading JSON files in multiple directories.
- `rayon` for parallel processing to improve performance.
- `regex` for extracting variables.
- `serde_json` for parsing JSON.

## Contributing

Feel free to submit issues and pull requests to improve this tool!

## License

This project is licensed under the MIT License.
