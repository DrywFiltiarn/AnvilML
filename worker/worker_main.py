"""AnvilML Python worker entry point.

Implements a blocking message loop over the ZeroMQ IPC transport
defined in ``worker.ipc``.  Supports mock mode (no torch
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
import queue
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
    cancel_flags: dict[str, threading.Event],
    msg_queue: queue.Queue,
) -> None:
    """Background thread that reads IPC messages from the socket.

    Owns the socket read side.  ``CancelJob`` messages create or set the
    cancel event (creating it if necessary so that a CancelJob arriving
    before Execute is still honoured); all other messages are placed on
    ``msg_queue`` for the main loop to process.  This ensures messages
    are never lost whether the main loop is in the message loop or
    blocked inside ``run_graph``.
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
            # Create the event if it doesn't exist yet (CancelJob may
            # arrive before the Execute handler creates it).
            if job_id not in cancel_flags:
                cancel_flags[job_id] = threading.Event()
            cancel_flags[job_id].set()
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

    # Connect to the ZeroMQ DEALER socket provided by the Rust supervisor.
    ipc.connect(int(os.environ["ANVILML_IPC_PORT"]))

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
    device_str: str = "cpu"
    cancel_flags: dict[str, threading.Event] = {}
    msg_queue: queue.Queue = queue.Queue()

    # Start the message reader thread so CancelJob messages can be
    # received while ``run_graph`` is running.
    reader = threading.Thread(
        target=_message_reader_thread,
        args=(cancel_flags, msg_queue),
        daemon=True,
        name="anvilml-msg-reader",
    )
    reader.start()

    while True:
        msg = msg_queue.get()
        _type: str = msg.get("_type", "")

        if _type == "InitializeHardware" and not ready_sent:
            hw_props = _probe_hardware()
            device_str = msg.get("device_str", "cpu")
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

            # Derive device_str from device_index when not set by Init.
            if device_str == "cpu" and not _mock:
                device_str = f"cuda:{device_index}"

            # Retrieve (or create) the cancel event for this job.
            # The reader thread may have already created it if
            # CancelJob arrived before Execute.
            if job_id not in cancel_flags:
                cancel_flags[job_id] = threading.Event()
            cancel_event = cancel_flags[job_id]

            # Build emit_fn closure around ipc.write_frame.
            def emit_fn(frame: dict) -> None:
                ipc.write_frame(frame)

            # Import run_graph and PipelineCache here so NODE_REGISTRY is
            # available and the cache is fresh per execution context.
            from worker.executor import run_graph  # noqa: E402
            from worker.pipeline_cache import PipelineCache  # noqa: E402

            pipeline_cache = PipelineCache()

            result = run_graph(
                graph,
                settings,
                device_str,
                cancel_event,
                emit_fn,
                pipeline_cache=pipeline_cache,
                job_id=job_id,
            )

            cancel_flags.pop(job_id, None)
            continue

        if _type == "Shutdown":
            ipc.write_frame({"_type": "Dying", "reason": "shutdown"})
            _shutdown_event.set()
            reader.join(timeout=2)
            t.join(timeout=2)
            sys.exit(0)


if __name__ == "__main__":
    main()
