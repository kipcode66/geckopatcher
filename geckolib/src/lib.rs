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
extern crate regex;
extern crate syn;

pub mod patch;
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

use async_std::io::{Read as AsyncRead, Seek as AsyncSeek};
#[cfg(not(target_arch = "wasm32"))]
use async_std::{fs, path::PathBuf};
use config::Config;
#[cfg(not(target_arch = "wasm32"))]
use eyre::Context;
use futures::AsyncWrite;
use iso::builder::IsoBuilder;
#[cfg(not(target_os = "unknown"))]
use iso::builder::PatchBuilder;
use iso::read::DiscReader;
use vfs::GeckoFS;
use zip::ZipArchive;

#[cfg(feature = "progress")]
lazy_static! {
    /// Progress updater
    pub static ref UPDATER: std::sync::Arc<std::sync::Mutex<update::Updater<eyre::Report, usize>>> =
        std::sync::Arc::new(std::sync::Mutex::new(update::Updater::default()));
}

/// Open a config from a patch file
pub async fn open_config_from_patch<RConfig, RDisc, W>(
    patch_reader: RConfig,
    iso_reader: RDisc,
    writer: W,
) -> eyre::Result<IsoBuilder<RConfig, RDisc, W>>
where
    RConfig: std::io::Read + std::io::Seek,
    RDisc: AsyncRead + AsyncSeek + Clone + Unpin + 'static,
    W: AsyncWrite + Clone + Unpin,
{
    let mut zip: ZipArchive<RConfig> = ZipArchive::new(patch_reader)?;

    let mut config: Config = {
        let toml_file = zip.by_name("RomHack.toml")?;

        toml::from_str(&std::io::read_to_string(toml_file)?)?
    };

    if let Some(link) = &mut config.link {
        link.libs.insert(0, "libcompiled.a".into());
    };

    let disc_reader = DiscReader::new(iso_reader).await?;
    Ok(IsoBuilder::new_with_zip(
        config,
        zip,
        GeckoFS::parse(disc_reader.clone()).await?,
        disc_reader,
        writer,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
/// Open a config from a file on the FileSystem to return an IsoBuilder
pub async fn open_config_from_fs_iso(
    config_file: &PathBuf,
) -> eyre::Result<IsoBuilder<File, async_std::fs::File, async_std::fs::File>> {
    #[cfg(feature = "progress")]
    if let Ok(mut updater) = UPDATER.lock() {
        updater.set_message("Parsing RomHack.toml
        ...".into())?;
    }

    let config: Config = toml::from_str(&fs::read_to_string(config_file).await?)?;
    let writer = async_std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&config.build.iso)
        .await?;
    let disc_reader = DiscReader::new(async_std::fs::File::open(&config.src.iso).await?).await?;
    let gfs = GeckoFS::parse(disc_reader.clone()).await?;
    Ok(IsoBuilder::new_with_fs(config, PathBuf::new(), gfs, disc_reader, writer))
}

#[cfg(not(target_arch = "wasm32"))]
/// Open a config from a file on the FileSystem to return a PatchBuilder
pub async fn open_config_from_fs_patch(config_file: &PathBuf) -> eyre::Result<PatchBuilder> {
    let config: Config = toml::from_str(&fs::read_to_string(config_file).await?)?;
    Ok(PatchBuilder::with_config(config))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn new(name: &str) -> eyre::Result<()> {
    use std::io::Write;

    let exit_code = Command::new("cargo")
        .args(["new", "--lib", name])
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
