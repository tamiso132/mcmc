use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // // 1. Get the path to your project's root directory
    // let manifest_dir =
    //     env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR environment variable not set");

    // // 2. Define the path to the Slang source code
    // let slang_src_dir = PathBuf::from(&manifest_dir).join("thirdparty/slang");

    // // 3. Define the exact build output directory you specified
    // let slang_build_dir = slang_src_dir.join("build/Release");

    // // 4. Tell Cargo to re-run this script if anything in the Slang directory changes.
    // //    This is crucial for incremental builds.
    // println!("cargo:rerun-if-changed={}", slang_src_dir.display());

    // // 5. Execute the CMake workflow command.
    // //    We run this command *from within* the 'thirdparty/slang' directory.
    // let status = Command::new("cmake")
    //     .arg("--workflow")
    //     .arg("--preset")
    //     .arg("release")
    //     .current_dir(&slang_src_dir) // <-- This is key
    //     .status()
    //     .expect("Failed to execute CMake workflow. Is 'cmake' in your system's PATH?");

    // // 6. Check if the build command was successful.
    // if !status.success() {
    //     // If it fails, panic to stop the build.
    //     panic!("CMake build workflow failed with status: {}", status);
    // }

    // // 7. Define the specific paths `shader-slang-sys` is looking for.

    // // The headers are in the *build* directory, as you confirmed.
    // let slang_include_dir = slang_build_dir.join("include");

    // // The libraries are in the *build* directory, as you confirmed.
    // let slang_lib_dir = slang_build_dir.join("lib");

    // // 8. Set all the environment variables the `sys` crate checks for.
    // //    This is the key fix.
    // println!("cargo:rustc-env=SLANG_DIR={}", slang_build_dir.display());
    // println!(
    //     "cargo:rustc-env=SLANG_INCLUDE_DIR={}",
    //     slang_include_dir.display()
    // );
    // println!("cargo:rustc-env=SLANG_LIB_DIR={}", slang_lib_dir.display());

    // // 9. Print the values to the console for you to see.
    // //    These regular println! statements will show up in your build log.
    // println!("SLANG_DIR set to: {}", slang_build_dir.display());
    // println!("SLANG_INCLUDE_DIR set to: {}", slang_include_dir.display());
    // println!("SLANG_LIB_DIR set to: {}", slang_lib_dir.display());
}
