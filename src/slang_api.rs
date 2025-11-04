//! Wrapper for the Slang shader compiler API.
//! This is a translation of the C++ SlangCompiler helper class.

use std::ffi::{CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents the result of a shader compilation.
#[derive(Debug)]
pub struct CompileResult {
    pub module: slang::Module,
    pub spirv_path: PathBuf,
}

/// A compiler for Slang shaders.
pub struct SlangCompiler {
    global_session: slang::GlobalSession,
    session: slang::Session,
}

impl SlangCompiler {
    /// Creates a new Slang compiler instance.
    ///
    /// # Arguments
    ///
    /// * `search_paths` - A slice of paths where the compiler should look for imported modules.
    pub fn new(search_paths: &[&Path]) -> Result<Self, String> {
        let global_session = slang::GlobalSession::new()
            .map_err(|e| format!("Failed to create Slang global session: {:?}", e))?;

        // All compiler options are available through this builder.
        let session_options = slang::CompilerOptions::default()
            .optimization(slang::OptimizationLevel::High)
            .matrix_layout_row(true);

        let target_desc = slang::TargetDesc::default()
            .format(slang::CompileTarget::Spirv)
            .profile(
                global_session
                    .find_profile("spirv_1_5")
                    .ok_or("spirv_1_5 profile not found")?,
            );

        let c_search_paths: Vec<CString> = search_paths
            .iter()
            .map(|p| CString::new(p.to_str().unwrap()).unwrap())
            .collect();

        let c_search_paths_ptr: Vec<*const i8> =
            c_search_paths.iter().map(|s| s.as_ptr()).collect();

        let targets = [target_desc];

        let session_desc = slang::SessionDesc::default()
            .targets(&targets)
            .search_paths(&c_search_paths_ptr)
            .options(&session_options);

        let session = global_session
            .create_session(&session_desc)
            .map_err(|e| format!("Failed to create Slang session: {:?}", e))?;

        Ok(Self {
            global_session,
            session,
        })
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
        file_path: &Path,
        entry_point_name: &str,
        output_directory: &Path,
    ) -> Result<CompileResult, String> {
        // Load module from file path
        let module = self
            .session
            .load_module(file_path.to_str().unwrap())
            .map_err(|(diag, _)| unsafe {
                CStr::from_ptr(diag.get_buffer_pointer())
                    .to_string_lossy()
                    .into_owned()
            })?;

        // Find entry point
        let entry_point = module
            .find_entry_point_by_name(entry_point_name)
            .ok_or_else(|| format!("Failed to find entry point '{}'", entry_point_name))?;

        // Compose and link
        let components = [module.clone().into(), entry_point.into()];
        let program = self
            .session
            .create_composite_component_type(&components)
            .map_err(|(diag, _)| unsafe {
                CStr::from_ptr(diag.get_buffer_pointer())
                    .to_string_lossy()
                    .into_owned()
            })?;

        let linked_program = program.link().map_err(|(diag, _)| unsafe {
            CStr::from_ptr(diag.get_buffer_pointer())
                .to_string_lossy()
                .into_owned()
        })?;

        // Get SPIR-V code
        let shader_bytecode =
            linked_program
                .get_entry_point_code(0, 0)
                .map_err(|(diag, _)| unsafe {
                    CStr::from_ptr(diag.get_buffer_pointer())
                        .to_string_lossy()
                        .into_owned()
                })?;

        // Write SPIR-V to file
        let file_stem = file_path.file_stem().unwrap().to_str().unwrap();
        let output_path = output_directory.join(format!("{}.spv", file_stem));

        fs::create_dir_all(output_directory)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;

        fs::write(&output_path, shader_bytecode)
            .map_err(|e| format!("Failed to write SPIR-V to file: {}", e))?;

        Ok(CompileResult {
            module,
            spirv_path: output_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_compile() {
        // Create a dummy shader file for testing
        fs::create_dir_all("target/shaders").unwrap();
        let shader_path = Path::new("target/shaders/test.slang");
        fs::write(
            shader_path,
            "float4 main() : SV_Target { return float4(1,0,0,1); }",
        )
        .unwrap();

        let compiler = SlangCompiler::new(&[Path::new("target/shaders")]).unwrap();
        let result = compiler.compile_shader(shader_path, "main", Path::new("target/spv"));

        assert!(result.is_ok());
        let compile_result = result.unwrap();
        assert!(compile_result.spirv_path.exists());
        assert_eq!(compile_result.spirv_path, Path::new("target/spv/test.spv"));
    }
}
