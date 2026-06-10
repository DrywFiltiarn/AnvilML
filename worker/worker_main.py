"""AnvilML Python worker entry point.

Implements a blocking socket message loop over the IPC framing
protocol defined in ``worker.ipc``.  Supports mock mode (no torch
dependency) via ``ANVILML_WORKER_MOCK=1``.

Message protocol
----------------
Rust  -> Python (WorkerMessage):
    InitializeHardware{device_str}
    Ping{seq}
    MemoryQuery
    Execute{job_id, graph, settings, device_index}
    CancelJob{job_id}
    Shutdown

Python -> Rust (WorkerEvent):
    Ready{worker_id, device_index, vram_total_mib, vram_free_mib, arch, fp16, bf16, flash_attention}
    Pong{seq}
    MemoryReport{vram_used_mib, ram_used_mib}
    Progress{job_id, node_index, node_total, node_type}
    Completed{job_id, elapsed_ms}
    Cancelled{job_id}
    Dying{reason}

A background daemon thread emits ``MemoryReport`` every 10 seconds.
"""

import os
import sys
import time

# Ensure the parent directory (repo root) is on sys.path so that
# ``worker.ipc`` can be imported regardless of how this script is
# invoked (e.g. ``python worker/worker_main.py`` vs
# ``python -m worker.worker_main``).
_repo_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _repo_root not in sys.path:
    sys.path.insert(0, _repo_root)

import argparse
import base64
import queue
import random
import threading

# ── Thread environment setup (§14.1 of ANVILML_DESIGN.md) ─────────────────────

_thread_count = int(os.environ.get("ANVILML_NUM_THREADS", "1"))
_interop_count = int(os.environ.get("ANVILML_NUM_INTEROP_THREADS", "1"))

os.environ["OMP_NUM_THREADS"] = str(_thread_count)
os.environ["MKL_NUM_THREADS"] = str(_thread_count)
os.environ["OPENBLAS_NUM_THREADS"] = str(_thread_count)
os.environ["VECLIB_MAXIMUM_THREADS"] = str(_thread_count)

# ── Conditional torch import ───────────────────────────────────────────────────

_mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"

if _mock:
    # Stub values matching the Rust-side mock-hardware defaults.
    _VRAM_TOTAL_MIB: int = 8192
    _VRAM_FREE_MIB: int = 8192
    _ARCH: str = "gfx1100"
    _FP16: bool = True
    _BF16: bool = True
    _FLASH_ATTENTION: bool = False

    torch = None  # type: ignore[name-defined]
else:
    import torch  # noqa: E402

    torch.set_num_threads(_thread_count)
    torch.set_num_interop_threads(_interop_count)


# ── Hardware property resolution ───────────────────────────────────────────────


def _probe_hardware():
    """Return a dict of hardware properties (mock or real)."""
    if _mock:
        return {
            "vram_total_mib": _VRAM_TOTAL_MIB,
            "vram_free_mib": _VRAM_FREE_MIB,
            "arch": _ARCH,
            "fp16": _FP16,
            "bf16": _BF16,
            "flash_attention": _FLASH_ATTENTION,
        }

    # Real hardware path — probe torch CUDA/ROCm.
    if not torch.cuda.is_available():
        # Fallback: CPU device has no VRAM; report zeros.
        return {
            "vram_total_mib": 0,
            "vram_free_mib": 0,
            "arch": "cpu",
            "fp16": False,
            "bf16": torch.cuda.is_bf16_supported() if hasattr(torch.cuda, "is_bf16_supported") else False,
            "flash_attention": False,
        }

    props = torch.cuda.get_device_properties(0)
    total, free = torch.cuda.mem_get_info()
    # Convert bytes -> MiB.
    vram_total_mib = total // (1024 * 1024)
    vram_free_mib = free // (1024 * 1024)

    arch = getattr(props, "name", "unknown")

    return {
        "vram_total_mib": vram_total_mib,
        "vram_free_mib": vram_free_mib,
        "arch": arch,
        "fp16": bool(props.major >= 7),  # Pascal+ supports fp16.
        "bf16": bool(getattr(props, "major", 0) >= 8),  # Ampere+ supports bf16.
        "flash_attention": False,  # Requires explicit torch compile flag.
    }


# ── Mock executor ──────────────────────────────────────────────────────────────


def _generate_black_png() -> bytes:
    """Create a 64x64 black RGB PNG and return raw PNG bytes."""
    from io import BytesIO
    from PIL import Image

    img = Image.new("RGB", (64, 64), (0, 0, 0))
    buf = BytesIO()
    img.save(buf, format="PNG")
    return buf.getvalue()


def _execute_mock(
    job_id: str | int,
    graph: dict,
    settings: dict,
    device_index: int,
    cancel_flag: dict[str, bool] | None = None,
) -> None:
    """Execute a graph in mock mode: emit Progress per node, then Completed.

    Iterates ``graph['nodes']`` in the order given (no topological sort —
    the DAG is validated by the server).  For each node a ``Progress``
    event is emitted; for ``SaveImage`` nodes an ``ImageReady`` event is
    also emitted.  After all nodes a single ``Completed`` event is emitted
    with the total elapsed time in milliseconds.

    If ``cancel_flag[job_id]`` is set before a node is processed, the
    function emits ``Cancelled{job_id}`` instead of ``Completed`` and
    returns immediately.

    When the ``ANVILML_MOCK_NODE_DELAY_MS`` environment variable is set
    to a positive integer, the executor sleeps that many milliseconds
    between nodes (after each ``Progress`` event), making cancellation
    observable during integration tests.
    """
    import worker.ipc as ipc  # noqa: E402

    if cancel_flag is None:
        cancel_flag = {}

    delay_ms = int(os.environ.get("ANVILML_MOCK_NODE_DELAY_MS", "0"))

    nodes = graph.get("nodes", [])
    start_time = time.monotonic()

    for i, node in enumerate(nodes):
        if cancel_flag.get(job_id):
            ipc.write_frame({
                "_type": "Cancelled",
                "job_id": job_id,
            })
            return

        node_type = node.get("type", "unknown")
        ipc.write_frame({
            "_type": "Progress",
            "job_id": job_id,
            "node_index": i,
            "node_total": len(nodes),
            "node_type": node_type,
        })

        if node_type == "SaveImage":
            inputs = node.get("inputs", {})

            # Resolve prompt — default empty string.
            prompt = inputs.get("prompt", "")

            # Resolve seed — from node inputs, fallback to settings, then -1.
            seed = inputs.get("seed", settings.get("seed", -1))
            if seed == -1:
                seed = random.randint(0, 2**63 - 1)

            # Resolve steps — from node inputs, fallback to settings, then 1.
            steps = inputs.get("steps", settings.get("steps", 1))

            # Generate black PNG and base64-encode.
            png_bytes = _generate_black_png()
            image_b64 = base64.b64encode(png_bytes).decode("ascii")

            ipc.write_frame({
                "_type": "ImageReady",
                "job_id": job_id,
                "image_b64": image_b64,
                "width": 64,
                "height": 64,
                "format": "png",
                "seed": seed,
                "steps": steps,
                "prompt": prompt,
            })

        if delay_ms > 0 and i < len(nodes) - 1:
            time.sleep(delay_ms / 1000.0)

    elapsed_ms = int((time.monotonic() - start_time) * 1000)
    ipc.write_frame({
        "_type": "Completed",
        "job_id": job_id,
        "elapsed_ms": elapsed_ms,
    })


# ── Background threads ─────────────────────────────────────────────────────────

_shutdown_event = threading.Event()


def _memory_report_thread(worker_id: str, device_index: int) -> None:
    """Emit ``MemoryReport`` frames every 10 seconds until shutdown."""
    while not _shutdown_event.wait(timeout=10):
        if _mock:
            vram_used_mib = 0
        else:
            try:
                total, free = torch.cuda.mem_get_info()
                vram_used_mib = (total - free) // (1024 * 1024)
            except Exception:
                vram_used_mib = 0

        report = {
            "_type": "MemoryReport",
            "vram_used_mib": vram_used_mib,
            "ram_used_mib": 0,
        }
        try:
            import worker.ipc as ipc

            ipc.write_frame(report)
        except Exception:
            pass  # Ignore write errors during shutdown.


def _message_reader_thread(
    cancel_flag: dict[str, bool],
    msg_queue: queue.Queue,
) -> None:
    """Background thread that reads IPC messages from the socket.

    Owns the socket read side.  ``CancelJob`` messages set the cancel
    flag directly; all other messages are placed on ``msg_queue`` for
    the main loop to process.  This ensures messages are never lost
    whether the main loop is in the message loop or blocked inside
    ``_execute_mock``.
    """
    import worker.ipc as ipc  # noqa: E402

    while not _shutdown_event.is_set():
        try:
            msg = ipc.read_frame()
        except Exception:
            break  # Connection lost — exit reader thread.

        _type: str = msg.get("_type", "")

        if _type == "CancelJob":
            job_id = msg.get("job_id", "")
            cancel_flag[job_id] = True
        elif _type == "Shutdown":
            _shutdown_event.set()
            msg_queue.put(msg)
            break
        else:
            msg_queue.put(msg)


# ── Main entry point ───────────────────────────────────────────────────────────


def main() -> None:
    parser = argparse.ArgumentParser(description="AnvilML Python worker")
    parser.add_argument("--worker-id", required=True, help="Logical worker identifier")
    parser.add_argument(
        "--device-index", type=int, required=True, help="GPU device index"
    )
    args = parser.parse_args()

    import worker.ipc as ipc  # noqa: E402 (after argparse so it can parse)

    # Connect to the IPC socket provided by the Rust supervisor.
    # Falls back to stdin/stdout if ANVILML_IPC_SOCKET is unset
    # (e.g. during testing).
    socket_path = os.environ.get("ANVILML_IPC_SOCKET")
    if socket_path is not None:
        ipc.connect(socket_path)

    # Start the background memory-report thread.
    t = threading.Thread(
        target=_memory_report_thread,
        args=(args.worker_id, args.device_index),
        daemon=True,
        name="anvilml-memory-report",
    )
    t.start()

    ready_sent = False
    hw_props: dict | None = None
    cancel_flag: dict[str, bool] = {}
    msg_queue: queue.Queue = queue.Queue()

    # Start the message reader thread so CancelJob messages can be
    # received while ``_execute_mock`` is running.
    reader = threading.Thread(
        target=_message_reader_thread,
        args=(cancel_flag, msg_queue),
        daemon=True,
        name="anvilml-msg-reader",
    )
    reader.start()

    while True:
        msg = msg_queue.get()
        _type: str = msg.get("_type", "")

        if _type == "InitializeHardware" and not ready_sent:
            hw_props = _probe_hardware()
            ready_event = {
                "_type": "Ready",
                "worker_id": args.worker_id,
                "device_index": args.device_index,
                "vram_total_mib": hw_props["vram_total_mib"],
                "vram_free_mib": hw_props["vram_free_mib"],
                "arch": hw_props["arch"],
                "fp16": hw_props["fp16"],
                "bf16": hw_props["bf16"],
                "flash_attention": hw_props["flash_attention"],
            }
            ipc.write_frame(ready_event)
            ready_sent = True
            continue

        if _type == "Ping":
            seq = msg.get("seq", 0)
            ipc.write_frame({"_type": "Pong", "seq": seq})
            continue

        if _type == "MemoryQuery":
            ipc.write_frame({
                "_type": "MemoryReport",
                "vram_used_mib": 0,
                "ram_used_mib": 0,
            })
            continue

        if _type == "Execute":
            job_id = msg.get("job_id", "")
            graph = msg.get("graph", {})
            settings = msg.get("settings", {})
            device_index = msg.get("device_index", args.device_index)
            _execute_mock(job_id, graph, settings, device_index, cancel_flag)
            cancel_flag.pop(job_id, None)
            continue

        if _type == "Shutdown":
            ipc.write_frame({"_type": "Dying", "reason": "shutdown"})
            _shutdown_event.set()
            reader.join(timeout=2)
            t.join(timeout=2)
            sys.exit(0)


if __name__ == "__main__":
    main()
