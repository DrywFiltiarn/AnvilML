use anvilml_ipc::framing::read_frame;
use anvilml_ipc::WorkerEvent;
use serde_json::json;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, mut rx) = tokio::io::duplex(4096);

    // Write a Pong event frame in flat-dict format (the response the Python worker sends).
    // The `_type` key is the variant discriminator expected by read_frame's deserializer.
    let pong = json!({ "_type": "Pong", "seq": 7u64 });
    let payload = rmp_serde::to_vec_named(&pong)?;
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
