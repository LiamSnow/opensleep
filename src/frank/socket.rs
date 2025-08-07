use std::time::Duration;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    time::timeout,
};

use super::error::FrankError;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

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
    // stream.write(format!("{}\n\n", cmd).as_bytes()).await?;
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

/// read response, errors if its not "ok"
pub async fn read_ok(stream: &mut UnixStream) -> Result<(), FrankError> {
    let res = read_response(stream, 4, 3).await?;
    match res.as_str() {
        "ok" => Ok(()),
        _ => Err(FrankError::ExpectedOk(res)),
    }
}

/// reads a response from Frank in the format `RESPONSE\n\n`
pub async fn read_response(stream: &mut UnixStream, exp_size: usize, max_exp_line: usize) -> Result<String, FrankError> {
    stream.readable().await?;
    let mut reader = BufReader::new(stream);
    timeout(RESPONSE_TIMEOUT, async {
        let mut result = String::with_capacity(exp_size);
        let mut prev_ended = false;
        loop {
            let mut line = String::with_capacity(max_exp_line);
            reader.read_line(&mut line).await?;

            if line == "\n" && prev_ended {
                break;
            } else {
                result.push_str(&line);
            }

            prev_ended = line.contains('\n');
        }
        result.pop();
        Ok(result)
    })
    .await
    .map_err(|_| FrankError::Timeout)?
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
    async fn test_write_cmd_for_no_payload() {
        let (mut client, mut server) = UnixStream::pair().unwrap();

        let cmd = 14u8;
        let expected = format!("{}\n\n", cmd);

        let server_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; expected.len()];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, expected.as_bytes());
        });

        write_cmd_for_no_payload(&mut client, cmd).await.unwrap();
        server_handle.await.unwrap();
    }

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

        let expected = format!(
            "{}\n{}\n\n",
            cmd, "a461760162676c190190626772190190626c621864"
        );

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
            let actual = read_response(&mut client, 260, 60).await.unwrap();
            assert_eq!(expected, actual);
        });

        let msg = expected.to_string() + "\n\n";
        server.write(msg.as_bytes()).await.unwrap();

        client_handle.await.unwrap();
    }
}
