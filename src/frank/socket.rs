use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    time::timeout,
};

use super::error::FrankError;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);

/// write a command, read "ok"
pub async fn cmd_transaction(stream: &mut UnixStream, cmd: u8) -> Result<(), FrankError> {
    write_cmd_for_no_payload(stream, cmd).await?;
    read_ok(stream).await
}

/// write a command then i16, read "ok"
pub async fn i16_transaction(stream: &mut UnixStream, cmd: u8, num: i16) -> Result<(), FrankError> {
    write_cmd_for_payload(stream, cmd).await?;
    let mut buf = itoa::Buffer::new();
    stream.write(buf.format(num).as_bytes()).await?;
    write_req_end(stream).await?;
    read_ok(stream).await
}

/// write a command then u16, read "ok"
pub async fn u16_transaction(stream: &mut UnixStream, cmd: u8, num: u16) -> Result<(), FrankError> {
    write_cmd_for_payload(stream, cmd).await?;
    let mut buf = itoa::Buffer::new();
    stream.write(buf.format(num).as_bytes()).await?;
    write_req_end(stream).await?;
    read_ok(stream).await
}

/// write a command then cbor (in hex format, as bytes), read "ok"
pub async fn cbor_transaction(
    stream: &mut UnixStream,
    cmd: u8,
    buf: &[u8],
) -> Result<(), FrankError> {
    write_cmd_for_payload(stream, cmd).await?;
    stream.write(buf).await?;
    write_req_end(stream).await?;
    read_ok(stream).await
}

/// write `COMMAND\n\n`
pub async fn write_cmd_for_no_payload(stream: &mut UnixStream, cmd: u8) -> Result<(), FrankError> {
    stream.writable().await?;

    let mut buf = itoa::Buffer::new();
    stream.write(buf.format(cmd).as_bytes()).await?;
    write_req_end(stream).await
}

/// write `COMMAND\n`
async fn write_cmd_for_payload(stream: &mut UnixStream, cmd: u8) -> Result<(), FrankError> {
    stream.writable().await?;

    let mut buf = itoa::Buffer::new();
    stream.write(buf.format(cmd).as_bytes()).await?;
    stream.write_u8(b'\n').await?;
    Ok(())
}

/// write `\n\n`
pub async fn write_req_end(stream: &mut UnixStream) -> Result<(), FrankError> {
    stream.write_u8(b'\n').await?;
    stream.write_u8(b'\n').await?;
    Ok(())
}

/// fastpath commmon "ok" response
pub async fn read_ok(stream: &mut UnixStream) -> Result<(), FrankError> {
    let mut buf = [0u8; 4];
    match timeout(RESPONSE_TIMEOUT, stream.read_exact(&mut buf)).await {
        Ok(Ok(_)) if &buf == b"ok\n\n" => return Ok(()),
        Ok(Ok(_)) => {
            let mut buf = buf.to_vec();
            let res = read_response(stream, &mut buf).await?;
            Err(FrankError::ExpectedOk(
                res.trim_end_matches('\n').to_string(),
            ))
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err(FrankError::Timeout),
    }
}

/// reads a response from Frank in the format `RESPONSE\n\n`
pub async fn read_response(
    stream: &mut UnixStream,
    buf: &mut Vec<u8>,
) -> Result<String, FrankError> {
    timeout(RESPONSE_TIMEOUT, async {
        stream.readable().await?;
        let mut temp_buf = [0u8; 50];

        loop {
            let n = stream.read(&mut temp_buf).await?;
            if n == 0 {
                return Err(FrankError::UnexpectedEndOfStream);
            }

            buf.extend_from_slice(&temp_buf[..n]);

            if let Some(pos) = find_req_end(&buf) {
                return Ok(String::from_utf8_lossy(&buf[..pos]).into_owned());
            }
        }
    })
    .await
    .map_err(|_| FrankError::Timeout)?
}

fn find_req_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(2).position(|window| window == b"\n\n")
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixStream,
    };

    use crate::frank::state::FrankSettings;

    use super::*;

    #[tokio::test]
    async fn test_cmd_transaction() {
        let (mut client, mut server) = UnixStream::pair().unwrap();

        let cmd = 11u8;
        let expected = format!("{}\n\n", cmd);

        let server_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; expected.len()];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, expected.as_bytes());
            server.write_all(b"ok\n\n").await.unwrap();
        });

        cmd_transaction(&mut client, cmd).await.unwrap();
        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_i16_transaction() {
        let (mut client, mut server) = UnixStream::pair().unwrap();

        let cmd = 11u8;
        let payload = -10i16;
        let expected = format!("{}\n{}\n\n", cmd, payload);

        let server_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; expected.len()];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, expected.as_bytes());
            server.write_all(b"ok\n\n").await.unwrap();
        });

        i16_transaction(&mut client, cmd, payload).await.unwrap();
        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_u16_transaction() {
        let (mut client, mut server) = UnixStream::pair().unwrap();

        let cmd = 11u8;
        let payload = 367u16;
        let expected = format!("{}\n{}\n\n", cmd, payload);

        let server_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; expected.len()];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, expected.as_bytes());
            server.write_all(b"ok\n\n").await.unwrap();
        });

        u16_transaction(&mut client, cmd, payload).await.unwrap();
        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_cbor_transaction() {
        let (mut client, mut server) = UnixStream::pair().unwrap();

        let cmd = 11u8;
        let payload = FrankSettings {
            version: 1,
            gain_left: 400,
            gain_right: 400,
            led_brightness_perc: 100,
        }
        .to_cbor()
        .unwrap();

        let expected = format!("{}\n{}\n\n", cmd, "a461760162676c190190626772190190626c621864");

        let server_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; expected.len()];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, expected.as_bytes());
            server.write_all(b"ok\n\n").await.unwrap();
        });

        cbor_transaction(&mut client, cmd, &payload).await.unwrap();
        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_read_cbor() {
        let (mut client, mut server) = UnixStream::pair().unwrap();

        let expected = r#"tgHeatLevelR = 100
tgHeatLevelL = 100
heatTimeL = 0
heatLevelL = -100
heatTimeR = 0
heatLevelR = -100
sensorLabel = "20600-0001-F00-0001089C"
waterLevel = true
priming = false
settings = "BF61760162676C190190626772190190626C621864FF""#;


        let client_handle = tokio::spawn(async move {
            let mut buf = Vec::with_capacity(272);
            let actual = read_response(&mut client, &mut buf).await.unwrap();
            assert_eq!(expected, actual);
        });

        let msg = expected.to_string() + "\n\n";
        server.write(msg.as_bytes()).await.unwrap();

        client_handle.await.unwrap();
    }
}
