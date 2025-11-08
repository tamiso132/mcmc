//! Wrapper for the Slang shader compiler API.
//! This is a translation of the C++ SlangCompiler helper class.

use std::ffi::{CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};

use shader_slang::{self as slang, CapabilityID, CompilerOptions, ComponentType, Downcast};

/// Represents the result of a shader compilation.

/// A compiler for Slang shaders.
pub struct SlangCompiler {
    global_session: slang::GlobalSession,
    session: slang::Session,
    out_dir: String,
}

impl SlangCompiler {
    /// Creates a new Slang compiler instance.
    ///
    /// # Arguments
    ///
    /// * `search_paths` - A slice of paths where the compiler should look for imported modules.
    pub fn new<'a>(search_paths: Vec<CString>, output_directory: &'a str) -> Self {
        let global_session = slang::GlobalSession::new().unwrap();

        let s_paths: Vec<*const i8> = search_paths.iter().map(|s| s.as_c_str().as_ptr()).collect();

        let target_desc = slang::TargetDesc::default()
            .format(slang::CompileTarget::Spirv)
            .profile(global_session.find_profile("glsl_450"));

        let session_options = slang::CompilerOptions::default()
            .optimization(slang::OptimizationLevel::None)
            .matrix_layout_row(true);

        let targets = [target_desc];

        let session_desc = slang::SessionDesc::default()
            .targets(&targets)
            .search_paths(&s_paths)
            .options(&session_options);

        let session = global_session.create_session(&session_desc).unwrap();

        Self {
            global_session,
            session,
            out_dir: output_directory.to_owned(),
        }
    }

    /// Compiles a shader from a source file.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path to the Slang shader file.
    /// * `entry_point_name` - The name of the entry point function in the shader.
    /// * `output_directory` - The directory where the compiled SPIR-V will be saved.
    pub fn compile_shader(
        &self,
        file_path: &str,
        entry_point_name: &str,
    ) -> slang::Result<slang::Module> {
        // Load module from file path
        println!("path: {}", file_path);
        let module = self.session.load_module(file_path).unwrap();

        // Find entry point
        let entry_point = module.find_entry_point_by_name(entry_point_name).unwrap();

        // Compose and link
        let program = self.session.create_composite_component_type(&[
            module.downcast().clone(),
            entry_point.downcast().clone(),
        ])?;

        let linked_program = program.link()?;

        // Get SPIR-V code
        let shader_bytecode = linked_program.entry_point_code(0, 0)?;
        println!("here?");

        // Write SPIR-V to file

        let output_path: String = {
            if self.out_dir.is_empty() {
                // If out_dir is empty, just use the file_path
                format!("{}.spv", file_path)
            } else {
                // If out_dir is NOT empty, join them with a slash
                format!("{}/{}.spv", self.out_dir, file_path)
            }
        };

        fs::create_dir_all(self.out_dir.clone())
            .map_err(|e| format!("Failed to create output directory: {}", e))
            .unwrap();

        fs::write(&output_path, shader_bytecode.as_slice())
            .map_err(|e| format!("Failed to write SPIR-V to file: {}", e))
            .unwrap();

        Ok(module)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_compile() {
        // Create a dummy shader file for testing

        let compiler = SlangCompiler::new(vec![], "");
        let mut result = compiler.compile_shader(&"simple.slang", "computeMain");

        assert!(result.is_ok());
        let compile_result = result.unwrap();
    }
}
