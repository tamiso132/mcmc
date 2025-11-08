// build.rs
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// --- Part 1: Parsing (Input -> "Parse Tokens") ---
mod parser {
    #[derive(Debug)]
    pub struct FieldDef {
        pub name: String,
        pub cpp_type: String,
    }

    #[derive(Debug)]
    pub struct StructDef {
        pub name: String,
        pub attributes: Vec<String>, // Stores words like SHADER_ALIGN_16
        pub fields: Vec<FieldDef>,
    }

    /// A simple, "best-effort" parser for C-like struct definitions.
    pub fn parse_cpp_structs(content: &str) -> Vec<StructDef> {
        let mut structs = Vec::new();
        let mut current_struct: Option<StructDef> = None;

        for line in content.lines() {
            let trimmed_line = line.trim();

            if let Some(s) = &mut current_struct {
                // --- We are INSIDE a struct block ---

                // Check for the end of the struct
                if trimmed_line.starts_with('}') {
                    if let Some(s) = current_struct.take() {
                        structs.push(s);
                    }
                    continue; // Done with this line
                }

                // Try to parse a field
                let field_line = trimmed_line.trim_end_matches(';');
                if field_line.len() == trimmed_line.len() || field_line.is_empty() {
                    continue; // Not a field line (e.g., a comment, or no ';')
                }

                if let Some(last_space) = field_line.rfind(char::is_whitespace) {
                    let cpp_type = field_line[..last_space].trim().to_string();
                    let name = field_line[last_space..].trim().to_string();

                    if !name.is_empty() && !cpp_type.is_empty() {
                        s.fields.push(FieldDef { name, cpp_type });
                    }
                }
            } else if trimmed_line.starts_with("struct") && trimmed_line.contains('{') {
                // --- Start of a NEW struct ---

                // Get the text between "struct" and "{"
                let name_part = trimmed_line
                    .strip_prefix("struct")
                    .unwrap_or(trimmed_line)
                    .strip_suffix('{')
                    .unwrap_or(trimmed_line)
                    .trim();

                let mut parts: Vec<String> =
                    name_part.split_whitespace().map(String::from).collect();

                if !parts.is_empty() {
                    let name = parts.pop().unwrap();
                    let attributes = parts;

                    current_struct = Some(StructDef {
                        name: name,
                        attributes: attributes, // Store the attributes
                        fields: Vec::new(),
                    });
                }
            }
        }
        structs
    }
}

// --- Part 2: Code Generation ("Parse Tokens" -> Rust File) ---
// (This function is unchanged)
fn generate_rust_code(
    parsed_structs: &Vec<parser::StructDef>,
    type_map: &HashMap<String, String>,
    struct_attr_map: &HashMap<String, StructAttributeConfig>,
) -> String {
    let mut generated_code = String::new();

    for s in parsed_structs {
        generated_code.push_str("\n"); // Add a newline before the struct

        // --- NEW LOGIC ---
        // We will *always* add #[repr(C)], as you suggested.
        // Then we'll add any extra properties from the config.
        let mut repr_parts = vec!["C".to_string()];
        let mut has_packed = false;
        let mut max_align: Option<u32> = None;

        // Check all attributes found on the struct (e.g., "SHADER_ALIGN_16")
        for attr_name in &s.attributes {
            if let Some(attr_config) = struct_attr_map.get(attr_name) {
                // Check if this config specifies alignment
                if let Some(align_val) = attr_config.align {
                    // If we find multiple, use the *largest* one
                    max_align = Some(max_align.unwrap_or(0).max(align_val));
                }
                // Check if this config specifies packing
                if attr_config.packed.unwrap_or(false) {
                    has_packed = true;
                }
            }
        }

        // Now, build the final repr string
        if let Some(align_val) = max_align {
            repr_parts.push(format!("align({})", align_val));
        }
        if has_packed {
            repr_parts.push("packed".to_string());
        }

        // Write the final attribute, e.g., "#[repr(C, align(16))]"
        let repr_string = format!("#[repr({})]", repr_parts.join(", "));
        generated_code.push_str(&repr_string);
        generated_code.push_str("\n");
        // --- END NEW LOGIC ---

        // Add the default derive
        generated_code.push_str("#[derive(Debug, Clone)]\n");
        generated_code.push_str(&format!("pub struct {} {{\n", s.name));

        for field in &s.fields {
            let rust_type = type_map.get(&field.cpp_type).unwrap_or_else(|| {
                // If not in the map, assume it's a struct we just parsed
                // (like `NodeKey`)
                &field.cpp_type
            });

            generated_code.push_str(&format!("    pub {}: {},\n", field.name, rust_type));
        }
        generated_code.push_str("}\n");
    }

    generated_code
}

// --- Orchestration (The Main Function) ---

// A struct to deserialize the structured attribute config
#[derive(Deserialize, Debug, Default)]
struct StructAttributeConfig {
    #[serde(default)] // Allows `align` to be missing
    align: Option<u32>,
    #[serde(default)] // Allows `packed` to be missing
    packed: Option<bool>,
}

// --- NEW: A struct for the [general] config section ---
#[derive(Deserialize, Debug, Default)]
struct GeneralConfig {
    #[serde(default = "Vec::new")]
    use_statements: Vec<String>,
}

// Struct for your config.toml
#[derive(Deserialize)]
struct Config {
    #[serde(default = "HashMap::new")]
    type_mappings: HashMap<String, String>,

    #[serde(default = "HashMap::new")]
    struct_attribute_mappings: HashMap<String, StructAttributeConfig>,

    // --- NEW FIELD ---
    #[serde(default = "GeneralConfig::default")] // Makes this section optional
    general: GeneralConfig,
}

fn main() {
    const INPUT_DIR: &str = "shaders/shared";
    const OUTPUT_FILE: &str = "generated_structs.rs";
    const DEBUG_FILE: &str = "debug_structs.txt";

    // --- Load Configs ---
    println!("cargo:rerun-if-changed=type_mappings.toml");
    let config_content = fs::read_to_string("type_mappings.toml").unwrap_or_else(|_| {
        eprintln!("Warning: type_mappings.toml not found. Using defaults.");
        String::new()
    });
    let config: Config = toml::from_str(&config_content).unwrap_or_else(|e| {
        eprintln!("Warning: Failed to parse type_mappings.toml: {}", e);
        Config {
            type_mappings: HashMap::new(),
            struct_attribute_mappings: HashMap::new(),
            general: GeneralConfig::default(),
        }
    });

    let type_map = config.type_mappings;
    let struct_attr_map = config.struct_attribute_mappings;
    let general_config = config.general; // <-- NEW

    // --- Iterate Input Directory ---
    println!("cargo:rerun-if-changed={}", INPUT_DIR);
    let mut all_generated_code = String::new();
    let mut all_debug_info = String::new();

    // --- NEW: Add headers and use statements FIRST ---
    all_generated_code.push_str("// This file is auto-generated by build.rs\n");
    all_generated_code.push_str("// Do not edit this file manually.\n\n");

    // Add all `use` statements from the config
    for use_stmt in general_config.use_statements {
        all_generated_code.push_str(&use_stmt);
        if !use_stmt.ends_with(';') {
            all_generated_code.push_str(";")
        }
        all_generated_code.push_str("\n");
    }
    // --- END NEW ---

    all_debug_info.push_str("--- PARSER DEBUG OUTPUT ---\n");

    let input_path = PathBuf::from(INPUT_DIR);
    if !input_path.exists() {
        eprintln!("Warning: Input directory not found: {}", INPUT_DIR);
        // Create an empty file to avoid build errors
        let out_dir = env::var("OUT_DIR").unwrap();
        let dest_path = Path::new(&out_dir).join(OUTPUT_FILE);
        fs::write(&dest_path, all_generated_code).expect("Failed to write empty generated code");
        let debug_dest_path = Path::new(&out_dir).join(DEBUG_FILE);
        fs::write(&debug_dest_path, all_debug_info).expect("Failed to write empty debug file");
        return;
    }

    for entry in fs::read_dir(input_path).expect("Failed to read input directory") {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_file() {
            let extension = path.extension().and_then(|s| s.to_str());
            if matches!(extension, Some("h") | Some("slang")) {
                // --- 1. Read and Parse ---
                all_debug_info.push_str(&format!("\n--- PARSING FILE: {:?} ---\n", path));
                let source_code =
                    fs::read_to_string(&path).expect(&format!("Failed to read file: {:?}", path));
                let parsed_structs = parser::parse_cpp_structs(&source_code);

                // --- 2. Write to debug log ---
                all_debug_info.push_str(&format!("{:#?}\n", parsed_structs));

                // --- 3. Generate Rust code ---
                if !parsed_structs.is_empty() {
                    // Pass the new map to the generator
                    let rust_code =
                        generate_rust_code(&parsed_structs, &type_map, &struct_attr_map);
                    all_generated_code.push_str(&rust_code);
                }
            }
        }
    }

    // --- Write the output files ---
    let out_dir = env::var("SHADER_OUT_DIR").unwrap();

    // Write generated Rust file
    let dest_path = Path::new(&out_dir).join(OUTPUT_FILE);
    fs::write(&dest_path, all_generated_code).expect("Failed to write generated code");

    all_debug_info.push_str(&format!("\nout_dir: {}", out_dir));

    // Write debug file
    let debug_dest_path = Path::new("").join(DEBUG_FILE);
    fs::write(&debug_dest_path, all_debug_info).expect("Failed to write debug file");
}
