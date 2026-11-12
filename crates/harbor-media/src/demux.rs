use std::io::Write;
use std::path::Path;

use harbor_sources::handle::MediaHandle;

use crate::au::AccessUnit;
use crate::shared::Shared;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub const MAGIC: &[u8; 4] = b"HBR1";

pub fn write_fixture(path: &Path, aus: &[AccessUnit]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(MAGIC)?;
    for au in aus {
        let size = au.bytes.len() as u32;
        f.write_all(&size.to_le_bytes())?;
        f.write_all(&au.pts_ns.to_le_bytes())?;
        f.write_all(&au.es.to_le_bytes())?;
        f.write_all(&au.bytes)?;
    }
    f.flush()?;
    Ok(())
}

pub fn demux_all(handle: &mut MediaHandle) -> Result<Vec<AccessUnit>, String> {
    let mut off = 0u64;
    let mut magic = [0u8; 4];
    let n = handle.read_at(off, &mut magic).map_err(|e| e.to_string())?;
    if n != 4 || &magic != MAGIC {
        return Err("unsupported container".into());
    }
    off += 4;
    let mut out = Vec::new();
    loop {
        match read_au(handle, &mut off)? {
            Some(au) => out.push(au),
            None => break,
        }
    }
    Ok(out)
}

pub fn run_worker(mut handle: MediaHandle, shared: Arc<Shared>) {
    let mut off = shared.take_seek_byte();
    if off == 0 {
        let mut magic = [0u8; 4];
        match handle.read_at(0, &mut magic) {
            Ok(4) if &magic == MAGIC => off = 4,
            _ => {
                shared.load_completed.store(true, Ordering::Release);
                return;
            }
        }
    }
    loop {
        if shared.stop.load(Ordering::Acquire) {
            break;
        }
        match read_au(&mut handle, &mut off) {
            Ok(Some(au)) => {
                let mut packet = au;
                while !shared.stop.load(Ordering::Acquire) {
                    match shared.queue.try_push(packet) {
                        Ok(()) => break,
                        Err(back) => {
                            packet = back;
                            thread::sleep(Duration::from_millis(1));
                        }
                    }
                }
            }
            Ok(None) => {
                shared.load_completed.store(true, Ordering::Release);
                break;
            }
            Err(_) => {
                shared.load_completed.store(true, Ordering::Release);
                break;
            }
        }
    }
}

fn read_au(handle: &mut MediaHandle, off: &mut u64) -> Result<Option<AccessUnit>, String> {
    let mut hdr = [0u8; 16];
    let n = handle.read_at(*off, &mut hdr).map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(None);
    }
    if n < 16 {
        return Ok(None);
    }
    let size = u32::from_le_bytes(hdr[0..4].try_into().unwrap()) as usize;
    let pts_ns = i64::from_le_bytes(hdr[4..12].try_into().unwrap());
    let es = i32::from_le_bytes(hdr[12..16].try_into().unwrap());
    *off += 16;
    let mut bytes = vec![0u8; size];
    if size > 0 {
        let got = handle.read_at(*off, &mut bytes).map_err(|e| e.to_string())?;
        if got < size {
            bytes.truncate(got);
        }
        *off += size as u64;
    }
    Ok(Some(AccessUnit { bytes, pts_ns, es }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_sources::handle::MediaHandle;

    fn sample_aus() -> Vec<AccessUnit> {
        (0..4)
            .map(|i| AccessUnit {
                bytes: vec![i as u8; 8],
                pts_ns: i * 40_000_000,
                es: 1,
            })
            .collect()
    }

    #[test]
    fn fixture_demux_counts_aus() {
        let path = std::env::temp_dir().join(format!("harbor-au-{}", std::process::id()));
        let want = sample_aus();
        write_fixture(&path, &want).unwrap();
        let mut h = MediaHandle::open_path(&path).unwrap();
        let got = demux_all(&mut h).unwrap();
        assert_eq!(got.len(), 4);
        assert_eq!(got[0].pts_ns, 0);
        assert_eq!(got[3].es, 1);
        assert_eq!(got[2].bytes[0], 2);
    }
}
