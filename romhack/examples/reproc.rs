use std::path::PathBuf;

#[cfg(feature = "log")]
use async_std::io::{prelude::SeekExt, ReadExt};
use clap::{arg, command, Parser, ValueHint};
use futures::AsyncWriteExt;
use geckolib::{
    iso::{read::DiscReader, write::DiscWriter}, vfs::GeckoFS
};
#[cfg(feature = "progress")]
use geckolib::UPDATER;
#[cfg(feature = "progress")]
use romhack::progress;

#[derive(Debug, Parser)]
#[command(author, version)]
/// Reprocess a game file (to the level of individual files)
///
/// Takes a SOURCE file and reprocess it (extract and re-pack)
/// into a DEST file. This extracts the whole FileSystem from
/// the SOURCE file.
struct Args {
    #[arg(value_hint = ValueHint::FilePath)]
    /// Game file to reprocess
    source: PathBuf,
    #[arg(value_hint = ValueHint::AnyPath)]
    /// Where to write the reprocessed file
    dest: PathBuf,
}

// Reprocesses a given iso (load iso in to a FileSystem, then save it back into an other iso)
fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    #[cfg(feature = "log")]
    env_logger::init();
    #[cfg(feature = "progress")]
    progress::init_cli_progress();

    let args = Args::parse();

    futures::executor::block_on(async {
        let file = async_std::fs::File::open(args.source).await?;
        #[cfg(feature = "log")]
        let mut f;
        #[cfg(not(feature = "log"))]
        let f;
        f = DiscReader::new(file).await?;
        let disc_info = f.get_disc_info();
        #[cfg(feature = "log")]
        {
            f.seek(std::io::SeekFrom::Start(0)).await?;
            let mut buf = vec![0u8; 0x60];
            f.read(&mut buf).await?;
            log::info!(
                "[{}] Game Title: {:02X?}",
                String::from_utf8_lossy(&buf[..6]),
                String::from_utf8_lossy(&buf[0x20..0x60])
                    .split_terminator('\0')
                    .find(|s| !s.is_empty())
                    .expect("This game has no title")
            );
        }
        let mut out: DiscWriter<async_std::fs::File> = DiscWriter::new(
            async_std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(args.dest)
                .await?,
            disc_info,
        );
        if let DiscWriter::Wii(wii_out) = out.clone() {
            std::pin::pin!(wii_out).init().await?;
        }

        let mut fs = GeckoFS::parse(f).await?;
        #[cfg(feature = "log")]
        log::info!("Encrypting the ISO");
        #[cfg(feature = "progress")]
        if let Ok(mut updater) = UPDATER.lock() {
            updater.init(None)?;
        }
        fs.serialize(&mut out).await?;
        out.close().await?;
        <color_eyre::eyre::Result<()>>::Ok(())
    })?;
    Ok(())
}
