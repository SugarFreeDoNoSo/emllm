"""Supervisa fases y recursos de un comando POSIX; conserva evidencia incremental."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import selectors
import signal
import subprocess
import sys
import time

from native_profile import DEFAULT_PROFILE, PHASES, load_profile, validate_profile

EVENT_PREFIX = b"EML_NATIVE_PHASE "
MAX_EVENT_BYTES = 4096


def emit_phase(event, phase):
    """El hijo emite eventos fuera de su propio intervalo cronometrado y hace flush."""
    if event not in ("start", "end") or phase not in PHASES:
        raise ValueError("invalid native phase event")
    print(EVENT_PREFIX.decode() + json.dumps({"schema_version": 1, "event": event, "phase": phase}),
          flush=True)


def sample_rss(pgid):
    """Suma RSS del grupo nuevo, en bytes; ps entrega KiB en Darwin/Linux.

    Puede contar páginas compartidas varias veces. No incluye al supervisor ni
    al proceso ps. No observa descendientes que abandonen el grupo de proceso.
    """
    if sys.platform not in ("darwin", "linux"):
        return {"bytes": None, "reason": "unsupported RSS platform"}
    try:
        result = subprocess.run(["ps", "-axo", "pid=,pgid=,rss="],
                                capture_output=True, text=True, check=True, timeout=1)
        members = []
        for line in result.stdout.splitlines():
            pid, group, kib = map(int, line.split())
            if group == pgid:
                if kib < 0:
                    raise ValueError("negative RSS")
                members.append({"pid": pid, "bytes": kib * 1024})
        if not members:
            return {"bytes": None, "reason": "process group not observed"}
        return {"bytes": sum(p["bytes"] for p in members), "members": members}
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        return {"bytes": None, "reason": f"ps unavailable: {error}"}


def wait4_peak_bytes(usage):
    if usage is None or sys.platform not in ("darwin", "linux"):
        return None
    # Darwin ru_maxrss es bytes; Linux es KiB. No es la suma simultánea del grupo.
    return int(usage.ru_maxrss) * (1 if sys.platform == "darwin" else 1024)


def _write_json(path, value):
    temporary = path.with_suffix(path.suffix + ".partial")
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")
    temporary.replace(path)


def supervise(command, output_dir, profile, *, interval=0.1, grace=0.25, cwd=None, rss_reader=sample_rss):
    """Lanza un comando sin shell, con sesión/grupo nuevos y perfil inmutable.

    Timeout por fase y también para arranque/huecos sin fase. El hijo debe emitir
    start/end secuenciales y cerrar todas las fases antes de salir con código 0.
    Las duraciones de eventos son de recepción, no tiempos de composición.
    """
    if os.name != "posix" or not hasattr(os, "wait4"):
        raise ValueError("supervision requires POSIX wait4")
    if not command or isinstance(command, (str, bytes)):
        raise ValueError("command must be a nonempty argument list")
    for name, value in [("interval", interval), ("grace", grace)]:
        if isinstance(value, bool) or not 0 < value <= 60 or not math.isfinite(value):
            raise ValueError(f"invalid {name}")
    profile = json.loads(json.dumps(validate_profile(profile), allow_nan=False))
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=False)
    profile_json = json.dumps(profile, sort_keys=True, separators=(",", ":")).encode()
    report = {
        "schema_version": 1, "status": "running", "command": list(map(str, command)),
        "cwd": str(Path(cwd or Path.cwd()).resolve()), "profile": profile,
        "profile_sha256": hashlib.sha256(profile_json).hexdigest(),
        "platform": platform.platform(), "python": platform.python_version(),
        "sampling_interval_seconds": interval, "termination_grace_seconds": grace,
        "rss_scope": "sampled sum of RSS in the new process group; shared pages may be counted repeatedly",
        "rss_method": "ps -axo pid=,pgid=,rss=; KiB * 1024 on Darwin/Linux",
        "wait4_scope": "OS wait4 usage for command, including waited descendants per OS; not simultaneous group sum",
        "timestamp_scope": "parent monotonic receipt; not the child's benchmark timer",
        "limit_kind": "observational watchdog, not a hard OS memory/time cap",
        "last_phase": None, "failure": None, "phases": [],
    }
    _write_json(output_dir / "report.json", report)
    deadline_seconds = profile["supervision"]["phase_timeout_seconds"]
    rss_limit = profile["supervision"]["max_rss_bytes"]
    started = time.monotonic()
    boundary = started
    next_sample = started
    active = None
    samples = 0
    peak = None
    unavailable = set()
    usage = None
    proc = None
    termination_at = None
    kill_sent = False
    exited_at = None
    buffer = b""
    skipping_line = False

    def fail(kind, message, **details):
        if report["failure"] is None:
            report["failure"] = {"kind": kind, "phase": report["last_phase"],
                                 "message": message, **details}

    def kill_group(sig):
        nonlocal usage, exited_at
        try:
            os.killpg(proc.pid, sig)
        except ProcessLookupError:
            pass
        except PermissionError:
            # Darwin puede devolver EPERM si el único miembro ya es un zombie.
            # Solo se acepta esa carrera después de confirmar/recolectar su salida.
            if proc.returncode is None:
                pid, status, observed_usage = os.wait4(proc.pid, os.WNOHANG)
                if pid:
                    proc.returncode = os.waitstatus_to_exitcode(status)
                    usage = observed_usage
                    exited_at = time.monotonic()
                    return
            raise

    with (output_dir / "stdout.log").open("wb") as stdout, \
            (output_dir / "stderr.log").open("wb") as stderr, \
            (output_dir / "events.jsonl").open("w", buffering=1) as events, \
            (output_dir / "rss.jsonl").open("w", buffering=1) as rss_log, \
            selectors.DefaultSelector() as selector:

        def event_line(line):
            nonlocal boundary, active
            if not line.startswith(EVENT_PREFIX):
                return
            now = time.monotonic()
            try:
                value = json.loads(line[len(EVENT_PREFIX):])
                if (not isinstance(value, dict) or set(value) != {"schema_version", "event", "phase"}
                        or type(value["schema_version"]) is not int or value["schema_version"] != 1
                        or value["event"] not in ("start", "end") or value["phase"] not in PHASES):
                    raise ValueError("invalid fields")
                if now - boundary > deadline_seconds:
                    fail("timeout", "phase or unlabelled interval exceeded before event receipt",
                         observed_seconds=now - boundary, limit_seconds=deadline_seconds)
                if value["event"] == "start":
                    if active is not None:
                        raise ValueError("nested or unclosed phase")
                    active = {"phase": value["phase"], "start_seconds": now - started,
                              "end_seconds": None, "sampled_peak_rss_bytes": None, "rss_samples": 0}
                    report["phases"].append(active)
                    report["last_phase"] = value["phase"]
                else:
                    if active is None or active["phase"] != value["phase"]:
                        raise ValueError("unmatched phase end")
                    active["end_seconds"] = now - started
                    active = None
                boundary = now
                events.write(json.dumps({"received_seconds": now - started, **value}) + "\n")
            except (ValueError, TypeError, UnicodeError) as error:
                fail("protocol", f"invalid phase event: {error}")

        try:
            proc = subprocess.Popen(command, cwd=cwd, stdin=subprocess.DEVNULL,
                                    stdout=subprocess.PIPE, stderr=stderr, start_new_session=True)
            report["pid"] = proc.pid
            os.set_blocking(proc.stdout.fileno(), False)
            selector.register(proc.stdout, selectors.EVENT_READ)
            while proc.returncode is None or selector.get_map():
                now = time.monotonic()
                if proc.returncode is None:
                    pid, status, observed_usage = os.wait4(proc.pid, os.WNOHANG)
                    if pid:
                        proc.returncode = os.waitstatus_to_exitcode(status)
                        usage = observed_usage
                        exited_at = now
                if proc.returncode is None and now >= next_sample:
                    observation = rss_reader(proc.pid)
                    rss_log.write(json.dumps({"received_seconds": now - started,
                                              "phase": active["phase"] if active else None, **observation}) + "\n")
                    value = observation["bytes"]
                    if value is None:
                        unavailable.add(observation["reason"])
                    else:
                        samples += 1
                        peak = max(peak or 0, value)
                        if active is not None:
                            active["rss_samples"] += 1
                            active["sampled_peak_rss_bytes"] = max(active["sampled_peak_rss_bytes"] or 0, value)
                        if value > rss_limit:
                            fail("rss-limit", "observed process-group RSS exceeded", observed_bytes=value, limit_bytes=rss_limit)
                    next_sample = time.monotonic() + interval
                # Drena stdout continuamente; el hijo no se bloquea por un pipe lleno.
                for key, _ in selector.select(timeout=min(interval, 0.05)):
                    chunk = os.read(key.fd, 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        if buffer and not skipping_line:
                            event_line(buffer)
                        buffer = b""
                        continue
                    stdout.write(chunk)
                    stdout.flush()
                    buffer += chunk
                    while b"\n" in buffer:
                        line, buffer = buffer.split(b"\n", 1)
                        if not skipping_line:
                            if line.startswith(EVENT_PREFIX) and len(line) > MAX_EVENT_BYTES:
                                fail("protocol", "phase event exceeds 4096 bytes")
                            else:
                                event_line(line)
                        skipping_line = False
                    if len(buffer) > MAX_EVENT_BYTES:
                        if not skipping_line and buffer.startswith(EVENT_PREFIX):
                            fail("protocol", "phase event exceeds 4096 bytes")
                        buffer = b""
                        skipping_line = True
                now = time.monotonic()
                if proc.returncode is None and now - boundary > deadline_seconds:
                    fail("timeout", "phase or unlabelled interval exceeded",
                         observed_seconds=now - boundary, limit_seconds=deadline_seconds)
                if exited_at is not None and selector.get_map() and now - exited_at > grace:
                    fail("unfinished-descendants", "stdout still held after command exit")
                    if not kill_sent:
                        kill_group(signal.SIGKILL)
                        kill_sent = True
                    for key in list(selector.get_map().values()):
                        selector.unregister(key.fileobj)
                if report["failure"] is not None and proc.returncode is None:
                    if termination_at is None:
                        kill_group(signal.SIGTERM)
                        termination_at = now
                    elif not kill_sent and now - termination_at >= grace:
                        kill_group(signal.SIGKILL)
                        kill_sent = True
        except KeyboardInterrupt:
            fail("interrupted", "supervisor interrupted")
        except OSError as error:
            fail("supervisor", str(error))
        finally:
            if proc is not None:
                # También elimina descendientes que sobrevivieran sin retener stdout.
                if not kill_sent:
                    kill_group(signal.SIGKILL)
                if proc.returncode is None:
                    _, status, usage = os.wait4(proc.pid, 0)
                    proc.returncode = os.waitstatus_to_exitcode(status)
                if proc.stdout is not None:
                    proc.stdout.close()

    elapsed = time.monotonic() - started
    final_peak = wait4_peak_bytes(usage)
    if final_peak is not None and final_peak > rss_limit:
        fail("rss-limit", "wait4 peak exceeded after exit", observed_bytes=final_peak, limit_bytes=rss_limit)
    returncode = None if proc is None else proc.returncode
    if returncode != 0:
        fail("command-exit", "command exited unsuccessfully", returncode=returncode)
    if active is not None:
        fail("protocol", "command exited with an open phase")
    if not report["phases"]:
        fail("protocol", "command emitted no phases")
    significant_unavailable = unavailable - {"process group not observed"}
    rss_verified = samples > 0 and not significant_unavailable and final_peak is not None
    report.update({
        "status": "error" if report["failure"] else ("ok" if rss_verified else "unverified"),
        "returncode": returncode, "elapsed_seconds": elapsed,
        "sampled_peak_rss_bytes": peak, "rss_samples": samples,
        "rss_unavailable_reasons": sorted(unavailable), "rss_budget_observed": rss_verified,
        "wait4_peak_rss_bytes": final_peak,
        "wait4_unavailable_reason": None if final_peak is not None else "unsupported platform or no wait4 usage",
        "cpu_user_seconds": None if usage is None else usage.ru_utime,
        "cpu_system_seconds": None if usage is None else usage.ru_stime,
    })
    _write_json(output_dir / "report.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", type=Path, default=DEFAULT_PROFILE)
    parser.add_argument("--output-dir", type=Path, required=True, help="Directorio nuevo; nunca sobrescribe evidencia")
    parser.add_argument("--interval", type=float, default=0.1)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    try:
        report = supervise(command, args.output_dir, load_profile(args.profile), interval=args.interval)
    except (ValueError, OSError) as error:
        parser.exit(2, f"native supervision: {error}\n")
    print(json.dumps({key: report[key] for key in ("status", "last_phase", "failure", "elapsed_seconds",
                                                  "sampled_peak_rss_bytes", "wait4_peak_rss_bytes")}, indent=2))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
