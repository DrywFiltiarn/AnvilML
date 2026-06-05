use anvilml_ipc::framing::read_frame;
use anvilml_ipc::WorkerEvent;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, mut rx) = tokio::io::duplex(4096);

    // Write a Pong event frame (the response the Python worker would send).
    // This proves the framing layer correctly serializes and deserializes frames.
    let event = WorkerEvent::Pong { seq: 7 };
    let payload = rmp_serde::to_vec_named(&event)?;
    let len = payload.len() as u32;
    tx.write_all(&len.to_be_bytes()).await?;
    tx.write_all(&payload).await?;

    // Read back the frame and verify.
    let result = read_frame(&mut rx, 64).await?;

    match result {
        WorkerEvent::Pong { seq: 7 } => println!("OK seq=7"),
        other => {
            eprintln!("mismatch: {:?}", other);
            std::process::exit(1)
        }
    }

    Ok(())
}
