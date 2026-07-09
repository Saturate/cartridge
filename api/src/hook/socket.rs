use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub fn send(path: &str, data: &[u8]) -> Result<(), String> {
    let stream = match UnixStream::connect(path) {
        Ok(s) => s,
        Err(e) => {
            std::thread::sleep(Duration::from_millis(10));
            UnixStream::connect(path).map_err(|_| format!("connect failed: {e}"))?
        }
    };

    stream
        .set_write_timeout(Some(Duration::from_millis(100)))
        .ok();

    let mut writer = std::io::BufWriter::new(stream);
    let len = (data.len() as u32).to_be_bytes();
    writer.write_all(&len).map_err(|e| format!("write len: {e}"))?;
    writer.write_all(data).map_err(|e| format!("write data: {e}"))?;
    writer.flush().map_err(|e| format!("flush: {e}"))?;

    Ok(())
}
