"""Entry point for the Python worker process.

This module provides the real-mode and mock-mode startup sequences for the
AnvilML Python worker. The real-mode path (torch import, real hardware probe)
is the default; mock mode is the explicit alternate that never imports torch.

Both paths follow the same startup sequence per ``ANVILML_DESIGN.md §14.2``:
IPC connect → probe capabilities → import nodes → send Ready event → dispatch loop.
"""

import logging
import os
import sys
import traceback

logger = logging.getLogger(__name__)


def _import_nodes() -> list[dict]:
    """Import node modules from ``worker/nodes/`` and return the registered node types.

    Triggers the ``worker.nodes`` auto-import mechanism (which scans ``nodes/`` for
    ``.py`` files and registers them via ``@register`` side-effects), then builds
    a list of type-descriptor dicts from ``NODE_REGISTRY``. Each dict contains
    ``type_name``, ``display_name``, ``category``, ``description``, ``inputs``,
    and ``outputs`` — matching the Rust ``NodeTypeDescriptor`` struct fields.

    This function is called during worker startup (both real and mock modes) so that
    the ``Ready`` event can carry the node type list even when it is empty.

    Importing ``worker.nodes`` inside this function (not at module level) follows the
    established pattern — ``worker.ipc`` is also imported inside functions like
    ``_real_startup_sequence()`` and ``_dispatch_loop()`` — to avoid transitive torch
    dependencies during test collection.

    Returns:
        List of dicts, one per registered node type. Each dict has keys
        ``type_name``, ``display_name``, ``category``, ``description``,
        ``inputs``, and ``outputs``. Returns ``[]`` when no node files exist.
    """
    # Import worker.nodes to trigger the auto-import loop in __init__.py.
    # This runs pkgutil.iter_modules() over nodes/ and imports any .py files
    # that define node classes via @register — populating NODE_REGISTRY.
    import worker.nodes  # noqa: F401 (side-effect: auto-imports node modules)

    # Access NODE_REGISTRY from its canonical location (worker.nodes.base).
    # The __init__.py skip-list excludes "base" so it is not auto-imported by
    # the loop — we must explicitly import it to read the registry.
    from worker.nodes import base

    registry = base.NODE_REGISTRY

    result: list[dict] = []
    for type_name, node_cls in registry.items():
        # Build a descriptor dict from the node class's class attributes.
        # Each node class must define NODE_TYPE, DISPLAY_NAME, CATEGORY,
        # DESCRIPTION, INPUT_SLOTS, and OUTPUT_SLOTS (enforced by @register).
        inputs = [
            {"name": spec.name, "slot_type": spec.slot_type, "optional": spec.optional}
            for spec in node_cls.INPUT_SLOTS
        ]
        outputs = [
            {"name": spec.name, "slot_type": spec.slot_type, "optional": spec.optional}
            for spec in node_cls.OUTPUT_SLOTS
        ]

        result.append({
            "type_name": node_cls.NODE_TYPE,
            "display_name": node_cls.DISPLAY_NAME,
            "category": node_cls.CATEGORY,
            "description": node_cls.DESCRIPTION,
            "inputs": inputs,
            "outputs": outputs,
        })

    return result

def _dispatch_loop(device: str = "cpu", caps: dict | None = None, mock: bool = False) -> None:
    """Receive messages from the supervisor in a loop, answering keepalive
    Pings, executing jobs on a background thread, handling cancellation
    requests, and exiting cleanly on Shutdown; all other message types are
    logged and skipped.

    `Ping`, `Shutdown`, and `CancelJob` are handled directly on the dispatch
    thread. `Execute` spawns a background thread that calls ``execute_graph()``
    with a job-scoped ``NodeContext`` factory, keeping the dispatch loop
    responsive to ``Ping``, ``CancelJob``, and ``Shutdown`` while the job
    executes.

    `Ping`: the Rust-side `KeepaliveWatchdog` (`P8-C2`/`P8-E5`) has been
    unconditionally active since Phase 8, sending a `Ping` immediately
    after `Ready` and on every `watchdog_ping_interval` after that, and
    declaring the worker dead if no matching `Pong` arrives within
    `watchdog_pong_timeout`. Without a handler, every real worker that
    reaches `Ready` is unconditionally killed and endlessly respawned.

    `Shutdown`: `ManagedWorker::graceful_shutdown_child()` sends
    `WorkerMessage::Shutdown` over IPC and waits (bounded by
    `graceful_shutdown_timeout`) for this process to exit on its own,
    before falling back to force-killing it. Without a handler here, that
    wait always times out — this process would never know it was asked to
    exit gracefully at all, only ever actually stopping via the timeout's
    force-kill fallback, or, faster still and unhandled entirely, via
    Windows propagating a console Ctrl+C directly to this process (it
    isn't spawned into its own process group) — which raises
    `KeyboardInterrupt` here, not caught by `except Exception` below since
    `KeyboardInterrupt` inherits from `BaseException`, producing an
    unhandled traceback on every single shutdown instead of a clean exit.
    Both paths are handled below so shutdown is clean regardless of which
    one actually wins the race in any given run.

    The loop runs indefinitely until the process is terminated by the
    supervisor or an external signal.

    Raises:
        RuntimeError: If ipc.connect() has not been called before entering
            the loop.
    """
    # Import ipc and threading here (not at module level) — dispatch_loop
    # may be called in tests that don't go through the startup sequence.
    # threading is needed for cancel_flag type annotations and the
    # CancelJob branch.
    import threading

    import worker.ipc as ipc

    logger.info("dispatch_loop: starting")
    # Track the currently-executing job so CancelJob messages can target it.
    # These persist across loop iterations while a job is executing; they
    # are reset only after the job completes (success, failure, or
    # cancellation) or when the loop exits.
    current_job_id: str | None = None
    current_cancel_flag: threading.Event | None = None

    while True:
        try:
            msg = ipc.recv_message()
        except KeyboardInterrupt:
            # Windows propagates a console Ctrl+C directly to this
            # process (it isn't spawned into its own process group), so
            # this can arrive here even when the supervisor also sends
            # (or is about to send) a proper Shutdown message over IPC —
            # see this function's own doc comment for the full
            # explanation. Treat it identically to a clean Shutdown: log
            # at INFO, not ERROR, and exit the loop without a traceback.
            logger.info("dispatch_loop: received KeyboardInterrupt, exiting cleanly")
            break
        except Exception as exc:
            # Log recv failure and continue — a broken socket means the
            # supervisor is gone; the worker should exit gracefully.
            # Without this try/except, the worker would crash on supervisor
            # shutdown instead of exiting cleanly.
            logger.error("dispatch_loop: recv failed, exiting: error=%s", exc)
            # If a background thread was spawned and the result has not
            # yet been sent (current_job_id still set), join and send it.
            # If current_job_id is None, the result was already sent in
            # the main loop when the thread completed.
            if "thread" in locals() and current_job_id is not None:
                thread.join()
                if result.get("cancelled"):
                    ipc.send_event({"_type": "Cancelled", "job_id": job_id})
                    logger.info("dispatch_loop: job cancelled job_id=%s", job_id)
                elif result.get("success"):
                    elapsed_ms = int((time.monotonic() - start) * 1000)
                    ipc.send_event(
                        {"_type": "Completed", "job_id": job_id, "elapsed_ms": elapsed_ms}
                    )
                    logger.info(
                        "dispatch_loop: job completed job_id=%s elapsed_ms=%d",
                        job_id,
                        elapsed_ms,
                    )
                else:
                    ipc.send_event({
                        "_type": "Failed",
                        "job_id": job_id,
                        "error": result["error"],
                        "traceback": result["traceback"],
                    })
                    logger.error(
                        "dispatch_loop: execute failed job_id=%s error=%s",
                        job_id,
                        result["error"],
                    )
                current_job_id = None
                current_cancel_flag = None
            break

        msg_type = msg.get("_type", "<unknown>")
        logger.debug("dispatch_loop: received message type=%s", msg_type)

        # If a background thread has completed, join it and send the
        # result event before processing any further messages. This
        # ensures the dispatch loop remains responsive to CancelJob
        # messages while the thread is running, but still sends the
        # terminal event (Completed/Failed/Cancelled) when done.
        if "thread" in locals() and not thread.is_alive():
            thread.join()
            # Check whether the background thread succeeded, failed, or
            # was cancelled. result is always populated because
            # run_execute writes it before returning.
            if result.get("cancelled"):
                ipc.send_event({"_type": "Cancelled", "job_id": job_id})
                logger.info("dispatch_loop: job cancelled job_id=%s", job_id)
            elif result.get("success"):
                elapsed_ms = int((time.monotonic() - start) * 1000)
                ipc.send_event(
                    {"_type": "Completed", "job_id": job_id, "elapsed_ms": elapsed_ms}
                )
                logger.info(
                    "dispatch_loop: job completed job_id=%s elapsed_ms=%d",
                    job_id,
                    elapsed_ms,
                )
            else:
                # Execution failed — send a Failed event with error details.
                # The error and traceback fields must match the Rust
                # WorkerEvent::Failed struct for msgpack deserialization.
                ipc.send_event({
                    "_type": "Failed",
                    "job_id": job_id,
                    "error": result["error"],
                    "traceback": result["traceback"],
                })
                logger.error(
                    "dispatch_loop: execute failed job_id=%s error=%s",
                    job_id,
                    result["error"],
                )
            # Reset tracking for the next job.
            current_job_id = None
            current_cancel_flag = None
            # Do NOT `continue` here. `ipc.recv_message()` is a fully
            # blocking call — the only reason this iteration is running
            # at all is that `msg` just arrived and woke it up. If a
            # background job thread happens to finish in that same
            # instant (the common case: the very message that wakes us
            # is the next keepalive Ping, landing right on the job's
            # completion boundary), `continue` would jump straight back
            # into `ipc.recv_message()` and discard `msg` completely —
            # it is never dispatched, "the next iteration" never
            # reprocesses it. For a dropped Ping specifically, that
            # means no Pong is ever sent for that seq, and
            # `KeepaliveWatchdog` (Rust side) declares this worker dead
            # `watchdog_pong_timeout` later and force-respawns it, even
            # though the job completed successfully. Falling through
            # instead lets `msg` reach its handler below in this same
            # iteration.
            pass

        if msg_type == "Ping":
            # Echo the sequence number back as a Pong — see this function's
            # own doc comment for why this is handled here rather than
            # deferred. `seq` must round-trip exactly (matched by the
            # watchdog against the ping it sent) — no other transformation.
            seq = msg["seq"]
            ipc.send_event({"_type": "Pong", "seq": seq})
            logger.debug("dispatch_loop: replied Pong seq=%s", seq)
        elif msg_type == "Shutdown":
            # Exit the loop cleanly — see this function's own doc comment
            # for why this is handled here rather than deferred.
            # graceful_shutdown_child() (Rust side) is waiting on this
            # process's own exit, bounded by graceful_shutdown_timeout;
            # responding promptly here is what makes that wait actually
            # succeed instead of always falling through to force-kill.
            logger.info("dispatch_loop: received Shutdown, exiting cleanly")
            break
        elif msg_type == "CancelJob":
            # Compare the incoming cancel request against the currently-
            # executing job. Only cancel the job that is actively running —
            # this is cooperative cancellation; we never interrupt a node
            # mid-execute. The cancel_flag is set so that execute_graph()
            # observes it before the next node's execute() call.
            cancel_job_id = msg["job_id"]
            if current_job_id == cancel_job_id and current_cancel_flag is not None:
                logger.info("dispatch_loop: cancelling job_id=%s", cancel_job_id)
                current_cancel_flag.set()
            else:
                # The cancel was for a job that already completed, or a
                # stale message. This is normal — a race between job
                # completion and the cancel message arrival. Log at DEBUG,
                # not error.
                logger.debug(
                    "dispatch_loop: CancelJob for non-current job_id=%s, ignoring",
                    cancel_job_id,
                )
        elif msg_type == "Execute":
            # Build a ctx_factory for this job — creates a NodeContext with
            # a per-job cancel_flag so CancelJob can signal cancellation to
            # the background execution thread.
            import time

            # Import execute_graph inside the handler (not at module level)
            # to avoid transitive torch dependencies during test collection.
            from worker.executor import execute_graph  # noqa: PLC0415

            job_id = msg["job_id"]
            graph = msg["graph"]
            logger.info("dispatch_loop: executing job_id=%s", job_id)

            # Track this job so CancelJob messages can target it.
            current_job_id = job_id
            # Create the cancel flag before the background thread so the
            # dispatch loop can set it from a CancelJob message.
            cancel_flag = threading.Event()
            current_cancel_flag = cancel_flag

            # Capture start time before the background thread begins.
            start = time.monotonic()

            # Shared result container — the background thread writes either
            # {"success": True}, {"cancelled": True}, or
            # {"success": False, "error": str, "traceback": str} into this
            # dict.  No lock is needed because thread.join() establishes a
            # happens-before guarantee: the main thread reads only after the
            # background thread has finished writing.
            result: dict = {}

            def run_execute() -> None:
                """Background thread target — runs execute_graph on a job-scoped context.

                On success writes {"success": True} to *result*.
                On cancellation writes {"cancelled": True} to *result*.
                On any exception writes {"success": False, "error", "traceback"}
                so the main thread can send a Failed event after join().
                """
                from worker.nodes.base import NodeContext

                ctx_factory = lambda: NodeContext(
                    job_id=job_id,
                    device=device,
                    caps=caps,
                    cancel_flag=cancel_flag,
                    emit=ipc.send_event,
                    pipeline_cache=None,
                    mock=mock,
                )
                try:
                    result_data = execute_graph(graph, ctx_factory)
                    if result_data.get("cancelled"):
                        result["cancelled"] = True
                    else:
                        result["success"] = True
                except Exception as exc:
                    # Capture the exception info so the main thread can
                    # send a Failed event with the error details.
                    result["success"] = False
                    result["error"] = str(exc)
                    result["traceback"] = traceback.format_exc()

            # Spawn a background thread so the dispatch loop remains
            # responsive to CancelJob and Ping messages while the job
            # executes. The thread is started here; the join is deferred
            # to the next loop iteration so that the dispatch loop can
            # process CancelJob messages before the thread finishes.
            thread = threading.Thread(target=run_execute, daemon=True)
            thread.start()



def _real_startup_sequence() -> None:
    """Run the real-mode startup sequence: IPC connect → torch import → device select → probe → Ready → loop.

    This implements the real-mode startup path per ``ANVILML_DESIGN.md §14.2``.
    It reads environment variables injected by the Rust supervisor, connects
    to the IPC ROUTER socket, imports torch, selects the target device, and
    runs the real torch-level capability probe. After probing, it sends a
    ``Ready`` event with ``capabilities_source="pytorch"`` and enters the
    message dispatch loop.

    The torch import happens inside this function (not at module level) so
    that importing ``worker_main`` does not transitively pull in torch —
    mock-mode tests and CI jobs can import this module without torch installed.

    Args:
        None — all configuration comes from environment variables:
            ANVILML_IPC_PORT, ANVILML_WORKER_ID,
            ANVILML_DEVICE_TYPE, ANVILML_DEVICE_INDEX.

    Returns:
        None — enters the dispatch loop and blocks indefinitely.

    Raises:
        KeyError: If any required environment variable is not set.
        RuntimeError: If IPC connection fails.
    """
    # Read IPC connection parameters from environment — these are injected
    # by the Rust supervisor's WorkerEnv at process launch time.
    port = int(os.environ["ANVILML_IPC_PORT"])
    worker_id = os.environ["ANVILML_WORKER_ID"]
    device_type = os.environ["ANVILML_DEVICE_TYPE"]
    device_index = int(os.environ["ANVILML_DEVICE_INDEX"])

    # Connect to the Rust supervisor's ROUTER socket before anything else;
    # all subsequent IPC operations (send_event, recv_message) require this.
    import worker.ipc as ipc

    ipc.connect(port, worker_id)

    # Import torch inside the function body — this ensures the import
    # only happens when real-mode startup actually runs, not at module
    # import time. Mock-mode tests can import worker_main without torch.
    import torch  # noqa: F401 (used for torch.cuda.set_device below)

    # CPU devices have no per-device selection; torch uses "cpu" implicitly.
    # For CUDA/ROCm, we must call set_device to select the target GPU index.
    if device_type != "cpu":
        torch.cuda.set_device(device_index)

    # Run the real torch-level capability probe — this actually constructs
    # tiny layers at each dtype and runs forward passes to determine support.
    import worker.capability as capability

    caps = capability.probe_capabilities(device_type, device_index)

    # Log the probe result at DEBUG level — operators need this for
    # diagnosing precision-capability mismatches on target hardware.
    logger.debug(
        "real_startup: device_type=%s, caps.fp32=%s, caps.fp16=%s, caps.bf16=%s",
        device_type,
        caps["fp32"],
        caps["fp16"],
        caps["bf16"],
    )

    # Import node types — currently empty (Phase 10 scope), but the Ready
    # event must carry the list so the supervisor knows the worker supports
    # the node dispatch pipeline (even if the list is empty).
    node_types = _import_nodes()

    # Build and send the Ready event — this tells the supervisor the worker
    # is operational and what capabilities/nodes it supports.
    # capabilities_source="pytorch" in this branch (real mode);
    # "mock" in the mock-mode branch (ANVILML_WORKER_MOCK=1).
    #
    # All fields must match the Rust `WorkerEvent::Ready` struct for
    # msgpack deserialization to succeed on the supervisor side.
    # device_name: for CPU, synthesize a name from torch's CPU info;
    #   for CUDA/ROCm, use torch.cuda.get_device_name().
    # vram_total_mib / vram_free_mib: for CPU, report 0 (no GPU VRAM);
    #   for CUDA/ROCm, query via torch.cuda.mem_get_info().
    # torch_version: from torch.__version__.
    if device_type == "cpu":
        device_name = "CPU"
        vram_total_mib = 0
        vram_free_mib = 0
    else:
        try:
            device_name = torch.cuda.get_device_name(device_index)
        except Exception:
            device_name = f"{device_type}:{device_index}"
        try:
            total, free = torch.cuda.mem_get_info(device_index)
            vram_total_mib = total // (1024 * 1024)
            vram_free_mib = free // (1024 * 1024)
        except Exception:
            vram_total_mib = 0
            vram_free_mib = 0

    ready_event = {
        "_type": "Ready",
        "worker_id": worker_id,
        "device_index": device_index,
        "device_name": device_name,
        "device_type": device_type,
        "vram_total_mib": vram_total_mib,
        "vram_free_mib": vram_free_mib,
        "torch_version": torch.__version__,
        "fp16": caps["fp16"],
        "bf16": caps["bf16"],
        "fp8": caps["fp8"],
        "flash_attention": caps["flash_attention"],
        "capabilities_source": "pytorch",
        "node_types": node_types,
    }
    ipc.send_event(ready_event)

    logger.info(
        "ready: capabilities_source=%s, node_types_count=%d",
        ready_event["capabilities_source"],
        len(node_types),
    )

    # Enter the message dispatch loop — blocks until the process is terminated.
    device = "cpu" if device_type == "cpu" else f"{device_type}:{device_index}"
    _dispatch_loop(device=device, caps=caps, mock=False)


def _mock_startup_sequence() -> None:
    """Run the mock-mode startup sequence: IPC connect → mock probe → Ready.

    This is the mock-mode equivalent of ``_real_startup_sequence()``.
    It uses ``_mock_probe_capabilities()`` instead of the real torch probe,
    and sends ``capabilities_source="mock"`` in the Ready event.

    The mock branch never imports ``torch`` — all capability values are
    synthetic. IPC connection, node import, and dispatch loop are identical
    to the real-mode path.

    Returns:
        None — enters the dispatch loop and blocks.
    """
    port = int(os.environ["ANVILML_IPC_PORT"])
    worker_id = os.environ["ANVILML_WORKER_ID"]
    device_type = os.environ["ANVILML_DEVICE_TYPE"]
    device_index = int(os.environ["ANVILML_DEVICE_INDEX"])

    import worker.ipc as ipc

    ipc.connect(port, worker_id)

    caps = _mock_probe_capabilities()

    logger.debug(
        "mock_startup: device_type=%s, caps.fp32=%s, caps.fp16=%s, caps.bf16=%s",
        device_type,
        caps["fp32"],
        caps["fp16"],
        caps["bf16"],
    )

    node_types = _import_nodes()

    # Build the Ready event with all fields expected by the Rust
    # `WorkerEvent::Ready` struct. Mock-mode uses synthetic values
    # for device_name, vram, and torch_version since torch is not
    # imported in this branch.
    # device_name: synthetic GPU name for mock mode.
    # vram_total_mib / vram_free_mib: synthetic VRAM values.
    # torch_version: synthetic version string.
    ready_event = {
        "_type": "Ready",
        "worker_id": worker_id,
        "device_index": device_index,
        "device_name": "Mock GPU",
        "device_type": device_type,
        "vram_total_mib": 1024,
        "vram_free_mib": 900,
        "torch_version": "0.0.0-mock",
        "fp16": caps["fp16"],
        "bf16": caps["bf16"],
        "fp8": caps["fp8"],
        "flash_attention": caps["flash_attention"],
        "capabilities_source": "mock",
        "node_types": node_types,
    }
    ipc.send_event(ready_event)

    logger.info(
        "ready: capabilities_source=%s, node_types_count=%d",
        ready_event["capabilities_source"],
        len(node_types),
    )

    _dispatch_loop(device="cpu", caps=caps, mock=True)


def _mock_probe_capabilities() -> dict:
    """Return fixed synthetic capability values matching ``InferenceCaps`` fields.

    This is the mock-mode equivalent of ``capability.probe_capabilities()``.
    It returns a dict with six boolean keys matching the ``InferenceCaps``
    struct field names: ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``,
    ``flash_attention``.

    The function never imports ``torch`` — it is a pure Python function
    returning a static dict literal, enabling mock-mode CI jobs to run
    without any GPU driver or torch installation.

    The values represent what a GPU-capable device would report:

    * ``fp32``, ``fp16``, ``bf16`` are ``True`` — universally supported on
      modern GPUs.
    * ``fp8`` is ``True`` — supported on GPU hardware (Ampere+ / CDNA+).
      While the real probe returns ``fp8=False`` on CPU (because
      ``torch.float8_e4m3fn`` raises on CPU), the mock is a synthetic
      baseline representing GPU capability, not a CPU simulation.
    * ``fp4`` is ``False`` — Torch 2.x has no native ``torch.float4`` dtype,
      so ``fp4`` is universally unsupported on all current torch builds.
    * ``flash_attention`` is ``True`` — on modern torch builds,
      ``scaled_dot_product_attention`` is always available (at minimum via
      the math fallback), so this is correctly ``True`` for the synthetic
      baseline.

    Returns:
        Dict with keys ``fp32``, ``fp16``, ``bf16``, ``fp8``, ``fp4``,
        ``flash_attention``, each mapping to a ``bool``.
    """
    # All capabilities except fp4 are True — this represents a GPU-capable
    # device baseline. fp4 is False because Torch 2.x has no native float4
    # dtype; no torch build supports it.
    return {
        "fp32": True,
        "fp16": True,
        "bf16": True,
        "fp8": True,
        "fp4": False,
        "flash_attention": True,
    }


if __name__ == "__main__":
    # Configure basic logging — the supervisor may set ANVILML_LOG_LEVEL,
    # but we always enable at least INFO-level output for diagnostics.
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )

    if os.environ.get("ANVILML_WORKER_MOCK") == "1":
        logger.info("worker: starting in mock mode")
        _mock_startup_sequence()
    else:
        logger.info("worker: starting in real mode")
        _real_startup_sequence()
