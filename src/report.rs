use crate::config::OutputSettings;
use chrono::{DateTime, Utc};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
};

pub fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
}

/// A single rendered string supplies exactly the same bytes to stdout and TXT.
pub fn write_report(
    output: &OutputSettings,
    account: &str,
    started_at: DateTime<Utc>,
    summary: &str,
    terminal: &mut impl Write,
) -> io::Result<Option<PathBuf>> {
    let mut path = None;
    if output.txt_file_output {
        let account = account
            .parse::<solana_pubkey::Pubkey>()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid benchmark account public key",
                )
            })?
            .to_string();
        let prefix = format!("{}_{}", &account[..4], &account[account.len() - 4..]);
        fs::create_dir_all(&output.directory)?;
        let timestamp = started_at.format(&output.timestamp_format).to_string();
        if !safe_component(&timestamp) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unsafe output filename",
            ));
        }
        for suffix in 0_u64.. {
            let extra = if suffix == 0 {
                String::new()
            } else {
                format!("_{suffix}")
            };
            let candidate = PathBuf::from(&output.directory)
                .join(format!("{}_{}{extra}.txt", prefix, timestamp));
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&candidate)
            {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
                Ok(mut file) => {
                    if let Err(error) = file
                        .write_all(summary.as_bytes())
                        .and_then(|_| file.flush())
                    {
                        drop(file);
                        let _ = fs::remove_file(&candidate);
                        return Err(error);
                    }
                    path = Some(candidate);
                    break;
                }
            }
        }
    }
    terminal.write_all(summary.as_bytes())?;
    terminal.flush()?;
    Ok(path)
}
