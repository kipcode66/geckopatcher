#[cfg(feature = "log")]
extern crate log;
#[cfg(feature = "parallel")]
extern crate rayon;
extern crate thiserror;
#[macro_use]
extern crate lazy_static;
extern crate async_std;
extern crate cbc;
extern crate eyre;
extern crate num;
extern crate serde;
extern crate sha1_smol;
#[macro_use]
extern crate static_assertions;

pub mod config;
pub mod crypto;
pub mod iso;
pub(crate) mod logs;
#[cfg(feature = "progress")]
pub mod update;
pub mod vfs;

#[cfg(not(target_arch = "wasm32"))]
use std::fs::{File, OpenOptions};
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;

use config::Config;
use eyre::Context;
use iso::builder::IsoBuilder;
use zip::ZipArchive;
use async_std::io::{Read as AsyncRead, Write as AsyncWrite, BufReader};

#[cfg(feature = "progress")]
lazy_static! {
    pub static ref UPDATER: std::sync::Arc<std::sync::Mutex<update::Updater<eyre::Report, usize>>> =
        std::sync::Arc::new(std::sync::Mutex::new(update::Updater::default()));
}

/// Open a config from a patch file
pub async fn open_config_from_patch<R: std::io::Read + std::io::Seek>(
    patch_reader: R,
) -> eyre::Result<IsoBuilder<R>> {
    let mut zip: ZipArchive<R> = ZipArchive::new(patch_reader)?;

    let config: Config = {
        let toml_file = zip.by_name("RomHack.toml")?;

        toml::from_str(&std::io::read_to_string(toml_file)?)?
    };

    Ok(IsoBuilder::new_with_zip(config, zip))
}

// #[cfg(not(target_arch = "wasm32"))]
// pub async fn apply_patch<R1: std::io::Read + std::io::Seek, R2: AsyncRead + std::io::Seek, W: AsyncWrite>(
//     patch: R1,
//     original_game: R2,
//     output: W,
// ) -> eyre::Result<()> {
//     let builder = open_config_from_patch(patch).await?;

//     config.src.iso = original_game;
//     config.build.iso = output;

//     build_and_emit_iso(printer, zip, compiled_library, config).await
// }

#[cfg(not(target_arch = "wasm32"))]
pub fn new(name: &str) -> Result<(), eyre::Report> {
    use std::io::Write;

    let exit_code = Command::new("cargo")
        .args(["new", "--lib", &name])
        .spawn()
        .context("Couldn't create the cargo project")?
        .wait()?;

    assert!(exit_code.success(), "Couldn't create the cargo project");

    let mut file = File::create(format!("{}/RomHack.toml", name))
        .context("Couldn't create the RomHack.toml")?;
    write!(
        file,
        r#"[info]
game-name = "{0}"

[src]
iso = "game.iso" # Provide the path of the game's ISO
patch = "src/patch.asm"
# Optionally specify the game's symbol map
# map = "maps/framework.map"

[files]
# You may replace or add new files to the game here
# "path/to/file/in/iso" = "path/to/file/on/harddrive"

[build]
map = "target/framework.map"
iso = "target/{0}.iso"

[link]
entries = ["init"] # Enter the exported function names here
base = "0x8040_1000" # Enter the start address of the Rom Hack's code here
"#,
        name.replace('-', "_"),
    )
    .context("Couldn't write the RomHack.toml")?;

    let mut file = File::create(format!("{}/src/lib.rs", name))
        .context("Couldn't create the lib.rs source file")?;
    write!(
        file,
        r#"#![no_std]

pub mod panic;

#[no_mangle]
pub extern "C" fn init() {{}}
"#
    )
    .context("Couldn't write the lib.rs source file")?;

    let mut file = File::create(format!("{}/src/panic.rs", name))
        .context("Couldn't create the panic.rs source file")?;
    write!(
        file,
        r#"#[cfg(any(target_arch = "powerpc", target_arch = "wasm32"))]
#[panic_handler]
pub fn panic(_info: &::core::panic::PanicInfo) -> ! {{
    loop {{}}
}}
"#
    )
    .context("Couldn't write the panic.rs source file")?;

    let mut file = File::create(format!("{}/src/patch.asm", name))
        .context("Couldn't create the default patch file")?;
    writeln!(
        file,
        r#"; You can use this to patch the game's code to call into the Rom Hack's code"#
    )
    .context("Couldn't write the default patch file")?;

    let mut file = OpenOptions::new()
        .append(true)
        .open(format!("{}/Cargo.toml", name))
        .context("Couldn't open the Cargo.toml")?;
    writeln!(
        file,
        r#"# Comment this in if you want to use the gcn crate in your rom hack.
# It requires the operating system symbols to be resolved via a map.
# gcn = {{ git = "https://github.com/CryZe/gcn", features = ["panic"] }}

[lib]
crate-type = ["staticlib"]

[profile.dev]
panic = "abort"
opt-level = 1

[profile.release]
panic = "abort"
lto = true"#
    )
    .context("Couldn't write into the Cargo.toml")?;

    let mut file = File::create(format!("{}/.gitignore", name))
        .context("Couldn't create the gitignore file")?;
    write!(
        file,
        r#"/target
**/*.rs.bk
"#
    )
    .context("Couldn't write the gitignore file")?;

    Ok(())
}
