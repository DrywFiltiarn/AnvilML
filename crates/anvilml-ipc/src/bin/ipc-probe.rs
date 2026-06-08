use anvilml_ipc::framing::{read_frame, write_frame};
use anvilml_ipc::WorkerEvent;
use anvilml_ipc::WorkerMessage;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, mut rx) = tokio::io::duplex(4096);

    // Write a Ping frame via the framing layer.
    write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 }).await?;

    // Read back the frame and verify.
    let result = read_frame(&mut rx, 64).await?;

    match result {
        WorkerEvent::Ping { seq: 7 } => println!("OK seq=7"),
        other => {
            eprintln!("mismatch: {:?}", other);
            std::process::exit(1)
        }
    }

    Ok(())
}
